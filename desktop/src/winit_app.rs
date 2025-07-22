use std::process::exit;

use winit::application::ApplicationHandler;
use winit::event::*;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::WindowId;

use crate::cef::{Context, Initialized};
use crate::render::{FrameBuffer, FrameBufferHandle, GraphicsState};

pub struct WinitApp {
	pub cef_context: Context<Initialized>,
	// Persistent initalized state when the window is created
	pub graphics_state: Option<GraphicsState>,

	// Shared between winit and cef, and stores the state for the window size and ui frame buffer.
	// Automatically kept in sync with the width/height in graphics state surface config
	pub frame_buffer: FrameBufferHandle,

	// Cached node graph output texture. And its position relative to the full ui
	pub viewport_top_left: u32,
	pub viewport_top_right: u32,
	pub viewport_texture: Option<wgpu::Texture>,
	pub viewport_bind_group: Option<wgpu::BindGroup>,

	// Cached UI texture and bindgroup for the CEF overlay
	pub ui_texture: Option<wgpu::Texture>,
	pub ui_bind_group: Option<wgpu::BindGroup>,
}

impl WinitApp {
	pub fn new(cef_context: Context<Initialized>, frame_buffer: FrameBufferHandle) -> Self {
		Self {
			cef_context,
			graphics_state: None,
			frame_buffer,
			viewport_top_left: 0,
			viewport_top_right: 0,
			viewport_texture: None,
			viewport_bind_group: None,
			ui_texture: None,
			ui_bind_group: None,
		}
	}

	// The single entrypoint for window resizing. It updates the frame buffer, surface config, cached UI overlay texture, and clears the viewport texture
	pub fn resize(&mut self, width: u32, height: u32) {
		if let Some(graphics_state) = &mut self.graphics_state {
			// Updates the surface config
			graphics_state.resize_surface(width, height);
			// Creates the cached ui texture, reconfigures the surface, and updates the frame buffer
			self.ui_texture = Some(graphics_state.device.create_texture(&wgpu::TextureDescriptor {
				label: Some("CEF Texture"),
				size: wgpu::Extent3d {
					width,
					height,
					depth_or_array_layers: 1,
				},
				mip_level_count: 1,
				sample_count: 1,
				dimension: wgpu::TextureDimension::D2,
				format: wgpu::TextureFormat::Bgra8UnormSrgb,
				usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
				view_formats: &[],
			}));

			self.ui_bind_group = Some(graphics_state.device.create_bind_group(&wgpu::BindGroupDescriptor {
				layout: &graphics_state.render_pipeline.get_bind_group_layout(0),
				entries: &[
					wgpu::BindGroupEntry {
						binding: 0,
						resource: wgpu::BindingResource::TextureView(self.ui_texture.as_ref().unwrap()),
					},
					wgpu::BindGroupEntry {
						binding: 1,
						resource: wgpu::BindingResource::Sampler(&graphics_state.sampler),
					},
				],
				label: Some("texture_bind_group"),
			}));

			// Invalidate the viewport texture, since we need to wait until cef calls on paint so we can get the new viewport size, which always changes when the window changes
			self.viewport_bind_group = None;
			self.viewport_texture = None;
		}

		// Keep the frame buffer in sync
		self.frame_buffer.lock().unwrap().resize(width, height);

		if let Some(browser) = &self.cef_context.browser {
			browser.host().unwrap().was_resized();
		}
	}
	// Composites the cached ui overlay texture onto the cached node graph texture. This should be called when the DOM or node graph gets evaluated.
	pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
		let Some(graphics_state) = &mut self.graphics_state else {
			println!("Graphics state not initialized in render function");
			return Ok(());
		};

		let output = graphics_state.surface.get_current_texture()?;
		let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

		let mut encoder = graphics_state.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Render Encoder") });

		{
			let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: Some("Render Pass"),
				color_attachments: &[Some(wgpu::RenderPassColorAttachment {
					view: &view,
					resolve_target: None,
					ops: wgpu::Operations {
						load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.1, g: 0.2, b: 0.3, a: 1.0 }),
						store: wgpu::StoreOp::Store,
					},
				})],
				depth_stencil_attachment: None,
				occlusion_query_set: None,
				timestamp_writes: None,
			});

			render_pass.set_pipeline(&graphics_state.render_pipeline);

			if let Some(node_graph_bind_group) = &self.viewport_bind_group {
				render_pass.set_bind_group(0, node_graph_bind_group, &[]);
				render_pass.draw(0..6, 0..1);
			}
			if let Some(ui_bind_group) = &self.ui_bind_group {
				render_pass.set_bind_group(0, ui_bind_group, &[]);
				render_pass.draw(0..6, 0..1);
			} else {
				println!("No bind group available for ui overlay");
			}
		}

		graphics_state.queue.submit(std::iter::once(encoder.finish()));
		output.present();

		Ok(())
	}

	// Loads the framebuffer data from CEF into the texture if it changed
	pub fn try_load_frame_buffer(&mut self) -> Result<(), String> {
		// Load the data from the shared frame buffer to the texture
		if let Some((new_buffer, width, height)) = self.frame_buffer.inner.lock().unwrap().take_buffer() {
			let Some(cached_ui_texture) = &self.ui_texture else {
				return Err("UI texture must be initialzed before loading framebuffer data".to_string());
			};
			let Some(graphics_state) = &mut self.graphics_state else {
				return Err("graphics state must exist before loading framebuffer data".to_string());
			};
			graphics_state.queue.write_texture(
				wgpu::ImageCopyTexture {
					texture: &cached_ui_texture,
					mip_level: 0,
					origin: wgpu::Origin3d::ZERO,
					aspect: wgpu::TextureAspect::All,
				},
				new_buffer,
				wgpu::ImageDataLayout {
					offset: 0,
					bytes_per_row: Some(4 * width),
					rows_per_image: Some(height),
				},
				wgpu::Extent3d {
					width,
					height,
					depth_or_array_layers: 1,
				},
			);
		}
		Ok(())
	}

	// Load the viewport texture and keep its position in sync with the browser viewport
	pub fn try_load_viewport(&mut self) -> Result<(), String> {
		let Some(graphics_state) = &mut self.graphics_state else {
			println!("Graphics state not initialized in try_load_viewport");
			return Ok(());
		};
		// Only runs if the viewport changes, in which case the cached texture should be recreated
		if let Some((top_left, top_right, width, height)) = self.frame_buffer.inner.lock().unwrap().get_viewport_size() {
			self.viewport_top_left = top_left;
			self.viewport_top_right = top_right;

			self.ui_texture = Some(graphics_state.device.create_texture(&wgpu::TextureDescriptor {
				label: Some("Vello Texture"),
				size: wgpu::Extent3d {
					width,
					height,
					depth_or_array_layers: 1,
				},
				mip_level_count: 1,
				sample_count: 1,
				dimension: wgpu::TextureDimension::D2,
				// Based on Vello requirements https://github.com/linebender/vello/blob/daf940230a24cbb123a458b6de95721af47aef98/vello/src/lib.rs#L460C36-L460C46
				format: wgpu::TextureFormat::Rgba8Unorm,
				usage: wgpu::TextureUsages::STORAGE_BINDING,
				view_formats: &[],
			}));

			self.ui_bind_group = Some(graphics_state.device.create_bind_group(&wgpu::BindGroupDescriptor {
				layout: &graphics_state.render_pipeline.get_bind_group_layout(0),
				entries: &[
					wgpu::BindGroupEntry {
						binding: 0,
						resource: wgpu::BindingResource::TextureView(self.ui_texture.as_ref().unwrap()),
					},
					wgpu::BindGroupEntry {
						binding: 1,
						resource: wgpu::BindingResource::Sampler(&graphics_state.sampler),
					},
				],
				label: Some("texture_bind_group"),
			}));
		}
	}
}

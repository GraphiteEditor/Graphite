//! A window that follows a live document: every change recompiles the graph, and each frame is
//! blitted from the executor's output texture straight onto the window surface.

use crate::{compile_graph, create_executor, editor_api, export};
use document_live::{Event, LiveDocument};
use graph_craft::application_io::{PlatformApplicationIo, PlatformEditorApi};
use graph_craft::document::value::UVec2;
use graphene_std::application_io::{ApplicationIo, ExportFormat, RenderConfig};
use interpreted_executor::dynamic_executor::DynamicExecutor;
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};
use wgpu::util::{TextureBlitter, TextureBlitterBuilder};
use wgpu_executor::{WgpuCurrentSurfaceTexture, WgpuExecutor, WgpuSurface};
use winit::application::ApplicationHandler;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

const POLL_INTERVAL: Duration = Duration::from_millis(50);
const BACKGROUND: wgpu::Color = wgpu::Color { r: 0.015, g: 0.015, b: 0.015, a: 1. };

pub async fn run(live: LiveDocument) -> Result<(), Box<dyn Error>> {
	let (application_io, editor_api) = editor_api(Some(Box::new(live.resource_proxy()))).await;
	let event_loop = EventLoop::new()?;

	let viewer = Viewer {
		live,
		application_io,
		editor_api,
		runtime: tokio::runtime::Handle::current(),
		window: None,
		surface: None,
		blitter: None,
		executor: None,
		dirty: false,
	};
	tokio::task::block_in_place(|| event_loop.run_app(viewer))?;
	Ok(())
}

struct Viewer {
	live: LiveDocument,
	application_io: Arc<PlatformApplicationIo>,
	editor_api: Arc<PlatformEditorApi>,
	runtime: tokio::runtime::Handle,
	window: Option<Arc<dyn Window>>,
	surface: Option<(WgpuSurface, wgpu::SurfaceConfiguration)>,
	blitter: Option<TextureBlitter>,
	executor: Option<DynamicExecutor>,
	dirty: bool,
}

impl Viewer {
	fn wgpu_executor(&self) -> &WgpuExecutor {
		self.application_io.gpu_executor().expect("the CLI is built with a GPU executor")
	}

	fn create_surface(&mut self, window: &Arc<dyn Window>) {
		let context = self.wgpu_executor().context();
		let surface = context.instance.create_surface(window.clone()).expect("create surface");
		let capabilities = surface.get_capabilities(&context.adapter);
		// A non-sRGB 8-bit target passes the renderer's already-encoded bytes through unchanged.
		let preferred = [wgpu::TextureFormat::Bgra8Unorm, wgpu::TextureFormat::Rgba8Unorm];
		let format = preferred.into_iter().find(|format| capabilities.formats.contains(format)).unwrap_or(capabilities.formats[0]);
		let size = window.surface_size();
		let config = wgpu::SurfaceConfiguration {
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
			format,
			width: size.width.max(1),
			height: size.height.max(1),
			present_mode: capabilities.present_modes[0],
			alpha_mode: capabilities.alpha_modes[0],
			view_formats: vec![],
			desired_maximum_frame_latency: 1,
		};
		surface.configure(&context.device, &config);

		self.blitter = Some(TextureBlitterBuilder::new(&context.device, format).blend_state(wgpu::BlendState::ALPHA_BLENDING).build());
		self.surface = Some((surface, config));
	}

	fn resize(&mut self) {
		let Some(window) = &self.window else { return };
		let size = window.surface_size();
		let device = self.wgpu_executor().context().device.clone();
		if let Some((surface, config)) = &mut self.surface
			&& size.width > 0
			&& size.height > 0
		{
			config.width = size.width;
			config.height = size.height;
			surface.configure(&device, config);
		}
	}

	fn recompile(&mut self) {
		let network = match self.runtime.block_on(self.live.network()) {
			Ok(network) => network,
			Err(error) => return log::error!("Failed to build the runtime network: {error}"),
		};
		let proto = match compile_graph(network, self.editor_api.clone(), Some(self.live.document())) {
			Ok(proto) => proto,
			Err(error) => return log::error!("Compile failed: {error}"),
		};
		match create_executor(proto) {
			Ok(executor) => {
				log::info!("Document compiled");
				self.executor = Some(executor);
			}
			Err(error) => log::error!("Executor creation failed: {error}"),
		}
	}

	fn draw(&mut self) -> Result<(), Box<dyn Error>> {
		let (Some(window), Some((surface, config)), Some(blitter)) = (&self.window, &self.surface, &self.blitter) else {
			return Ok(());
		};

		// The document is rendered once it is on hand and compiled; until then the window still presents
		// its background, since a Wayland surface that never commits a buffer is never shown at all.
		let frame = match &self.executor {
			Some(executor) => {
				let mut render_config = RenderConfig {
					export_format: ExportFormat::Raster,
					scale: 1.,
					..Default::default()
				};
				render_config.viewport.resolution = UVec2::new(config.width, config.height);
				Some(self.runtime.block_on(export::render_texture(executor, render_config))?)
			}
			None => None,
		};

		let context = self.wgpu_executor().context();
		let surface_texture = match surface.get_current_texture(&context.queue) {
			WgpuCurrentSurfaceTexture::Success(texture) | WgpuCurrentSurfaceTexture::Suboptimal(texture) => texture,
			WgpuCurrentSurfaceTexture::Occluded => return Ok(()),
			other => return Err(format!("surface unavailable: {other:?}").into()),
		};
		let target = surface_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());

		let mut encoder = context.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("live view") });
		encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
			label: Some("live view clear"),
			color_attachments: &[Some(wgpu::RenderPassColorAttachment {
				view: &target,
				resolve_target: None,
				depth_slice: None,
				ops: wgpu::Operations {
					load: wgpu::LoadOp::Clear(BACKGROUND),
					store: wgpu::StoreOp::Store,
				},
			})],
			depth_stencil_attachment: None,
			occlusion_query_set: None,
			timestamp_writes: None,
			multiview_mask: None,
		});
		if let Some(frame) = &frame {
			let source = frame.create_view(&wgpu::TextureViewDescriptor::default());
			blitter.copy(&context.device, &mut encoder, &source, &target);
		}

		surface_texture.queue.submit([encoder.finish()]);
		window.pre_present_notify();
		surface_texture.present();
		Ok(())
	}
}

impl ApplicationHandler for Viewer {
	fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
		let attributes = WindowAttributes::default()
			.with_title(format!("Graphite live view {}", self.live.token()))
			.with_surface_size(winit::dpi::LogicalSize::new(800, 600));
		let window: Arc<dyn Window> = event_loop.create_window(attributes).expect("create window").into();
		self.create_surface(&window);
		window.request_redraw();
		self.window = Some(window);
		event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + POLL_INTERVAL));
	}

	fn new_events(&mut self, event_loop: &dyn ActiveEventLoop, _cause: StartCause) {
		for event in self.runtime.block_on(self.live.poll()) {
			if matches!(event, Event::Synced | Event::Changed | Event::ResourceReceived { .. }) {
				self.dirty = true;
			}
			if let Event::Synced = event {
				log::info!("Synced with the session");
			}
		}
		if self.dirty && self.live.is_ready() {
			self.dirty = false;
			self.recompile();
			if let Some(window) = &self.window {
				window.request_redraw();
			}
		}
		event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + POLL_INTERVAL));
	}

	fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
		match event {
			WindowEvent::CloseRequested => {
				self.live.leave();
				event_loop.exit();
			}
			WindowEvent::SurfaceResized(_) => {
				self.resize();
				if let Some(window) = &self.window {
					window.request_redraw();
				}
			}
			WindowEvent::RedrawRequested => {
				if let Err(error) = self.draw() {
					log::error!("Draw failed: {error}");
				}
			}
			_ => {}
		}
	}
}

//! A window that follows a live document, recompiling the graph on every change and blitting the rendered texture onto the surface.

use document_live::{Event, LiveDocument};
use graph_craft::application_io::resource::ResourceHash;
use graph_craft::application_io::{PlatformApplicationIo, PlatformEditorApi};
use graph_craft::document::value::UVec2;
use graphene_cli::{TokioSpawner, compile_graph, create_executor, editor_api, export};
use graphene_std::application_io::{ApplicationIo, ExportFormat, RenderConfig};
use graphene_std::runtime::AsyncRuntime;
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
	if application_io.gpu_executor().is_none() {
		return Err("watching a session needs a usable GPU".into());
	}
	let event_loop = EventLoop::new()?;
	let spawner = Arc::new(TokioSpawner::default());

	let viewer = Viewer {
		live,
		application_io,
		editor_api,
		runtime: tokio::runtime::Handle::current(),
		spawner: spawner.clone(),
		graph_runtime: AsyncRuntime::new(spawner, Arc::new(std::time::Instant::now())),
		window: None,
		surface: None,
		blitter: None,
		executor: None,
		dirty: false,
		waiting: Vec::new(),
	};
	tokio::task::block_in_place(|| event_loop.run_app(viewer))?;
	Ok(())
}

struct Viewer {
	live: LiveDocument,
	application_io: Arc<PlatformApplicationIo>,
	editor_api: Arc<PlatformEditorApi>,
	runtime: tokio::runtime::Handle,
	spawner: Arc<TokioSpawner>,
	graph_runtime: AsyncRuntime,
	window: Option<Arc<dyn Window>>,
	surface: Option<(WgpuSurface, wgpu::SurfaceConfiguration)>,
	blitter: Option<TextureBlitter>,
	executor: Option<DynamicExecutor>,
	dirty: bool,
	/// The missing resources last logged.
	waiting: Vec<ResourceHash>,
}

impl Viewer {
	fn wgpu_executor(&self) -> &WgpuExecutor {
		self.application_io.gpu_executor().expect("`run` checks for a GPU executor")
	}

	fn create_surface(&mut self, window: &Arc<dyn Window>) {
		let context = self.wgpu_executor().context();
		let surface = context.instance.create_surface(window.clone()).expect("create surface");
		let capabilities = surface.get_capabilities(&context.adapter);
		// The rendered bytes are already sRGB encoded, so a non-sRGB target passes them through unchanged
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
			color_space: wgpu::SurfaceColorSpace::Auto,
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

	fn log_missing_resources(&mut self) {
		let missing = self.live.missing_resources();
		if missing == self.waiting {
			return;
		}
		if !missing.is_empty() {
			let hashes = missing.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ");
			log::info!("Waiting for {} resources no peer has sent yet: {hashes}", missing.len());
		}
		self.waiting = missing;
	}

	fn recompile(&mut self) {
		let network = match self.runtime.block_on(self.live.network()) {
			Ok(network) => network,
			Err(error) => return log::error!("Failed to build the runtime network: {error}"),
		};
		let proto = match compile_graph(network, self.editor_api.clone(), Some(self.live.document()), self.graph_runtime.handle()) {
			Ok(proto) => proto,
			Err(error) => return log::error!("Compile failed: {error}"),
		};
		match create_executor(proto) {
			Ok(executor) => {
				self.graph_runtime.retain_sources(executor.sources());
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

		// Present the background even without a frame, since Wayland never shows a surface that has not committed a buffer
		let frame = match &self.executor {
			Some(executor) => {
				let mut render_config = RenderConfig {
					export_format: ExportFormat::Raster,
					scale: 1.,
					..Default::default()
				};
				render_config.viewport.resolution = UVec2::new(config.width, config.height);
				Some(self.runtime.block_on(export::render_texture(executor, render_config, &self.graph_runtime, &self.spawner))?)
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
			self.dirty |= match event {
				Event::Synced => {
					log::info!("Synced with the session");
					true
				}
				Event::ResourceReceived { hash, bytes } => {
					log::info!("Received resource {hash} ({} bytes)", bytes.len());
					true
				}
				Event::Changed => true,
				_ => false,
			};
		}
		if self.dirty && self.live.is_synced() {
			self.log_missing_resources();
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

use std::fmt::Debug;
use std::process::exit;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use winit::application::ApplicationHandler;
use winit::event::*;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowId};

mod cef;
use cef::Setup;

mod render;
use render::{FrameBuffer, GraphicsState};

pub(crate) enum CustomEvent {
	UiUpdate,
	Resized,
	DoBrowserWork,
}

pub(crate) struct WindowState {
	width: Option<usize>,
	height: Option<usize>,
	ui_fb: Option<FrameBuffer>,
	preview_fb: Option<FrameBuffer>,
	graphics_state: Option<GraphicsState>,
	event_loop_proxy: Option<EventLoopProxy<CustomEvent>>,
}

impl WindowState {
	fn new() -> Self {
		Self {
			width: None,
			height: None,
			ui_fb: None,
			preview_fb: None,
			graphics_state: None,
			event_loop_proxy: None,
		}
	}

	fn handle(self) -> WindowStateHandle {
		WindowStateHandle { inner: Arc::new(Mutex::new(self)) }
	}
}

impl Debug for WindowState {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("WindowState")
			.field("width", &self.width.is_some())
			.field("height", &self.height.is_some())
			.field("ui_fb", &self.ui_fb.is_some())
			.field("preview_fb", &self.preview_fb.is_some())
			.field("graphics_state", &self.graphics_state.is_some())
			.finish()
	}
}

pub(crate) struct WindowStateHandle {
	inner: Arc<Mutex<WindowState>>,
}

impl WindowStateHandle {
	fn with<'a, P>(&self, p: P) -> Result<(), PoisonError<MutexGuard<'a, WindowState>>>
	where
		P: FnOnce(&mut WindowState),
	{
		match self.inner.lock() {
			Ok(mut guard) => Ok(p(&mut guard)),
			Err(_) => todo!("not error handling yet"),
		}
	}
}

impl Clone for WindowStateHandle {
	fn clone(&self) -> Self {
		Self { inner: self.inner.clone() }
	}
}

#[derive(Clone)]
struct CefEventHandler {
	window_state: WindowStateHandle,
}

impl CefEventHandler {
	fn new(window_state: WindowStateHandle) -> Self {
		Self { window_state }
	}
}

impl cef::EventHandler for CefEventHandler {
	fn view(&self) -> cef::View {
		let mut w = 1;
		let mut h = 1;

		self.window_state
			.with(|s| match s {
				WindowState {
					width: Some(width),
					height: Some(height),
					..
				} => {
					w = *width;
					h = *height;
				}
				_ => {}
			})
			.unwrap();

		cef::View::new(w, h)
	}

	fn draw(&self, buffer: Vec<u8>, width: usize, height: usize) -> bool {
		let fb = FrameBuffer::new(buffer, width, height)
			.map_err(|e| {
				panic!("Failed to create FrameBuffer: {}", e);
			})
			.unwrap();

		let mut correct_size = true;
		self.window_state
			.with(|s| {
				if let Some(event_loop_proxy) = &s.event_loop_proxy {
					let _ = event_loop_proxy.send_event(CustomEvent::UiUpdate);
					let _ = event_loop_proxy.send_event(CustomEvent::DoBrowserWork);
				}
				if width != s.width.unwrap_or(1) || height != s.height.unwrap_or(1) {
					correct_size = false;
				} else {
					s.ui_fb = Some(fb);
				}
			})
			.unwrap();

		correct_size
	}
}

struct WinitApp {
	window_state: WindowStateHandle,
	cef_context: cef::Context<cef::Initialized>,
	window: Option<Arc<Window>>,
}

impl WinitApp {
	fn new(window_state: WindowStateHandle, cef_context: cef::Context<cef::Initialized>) -> Self {
		Self {
			window_state,
			cef_context,
			window: None,
		}
	}
}

impl ApplicationHandler<CustomEvent> for WinitApp {
	fn resumed(&mut self, event_loop: &ActiveEventLoop) {
		self.window_state
			.with(|s| match s {
				WindowState { width: Some(w), height: Some(h), .. } => {
					let window = Arc::new(
						event_loop
							.create_window(
								Window::default_attributes()
									.with_title("CEF Offscreen Rendering")
									.with_inner_size(winit::dpi::LogicalSize::new(*w as u32, *h as u32)),
							)
							.unwrap(),
					);
					let graphics_state = pollster::block_on(GraphicsState::new(window.clone()));

					self.window = Some(window.clone());
					s.graphics_state = Some(graphics_state);

					let _ = thread::spawn(move || loop {
						thread::sleep(Duration::from_millis(100));
						window.request_redraw();
					});

					println!("Winit window created and ready");
				}
				_ => {}
			})
			.unwrap();
	}

	fn user_event(&mut self, _: &ActiveEventLoop, event: CustomEvent) {
		match event {
			CustomEvent::DoBrowserWork => {
				self.cef_context.work();
			}
			CustomEvent::UiUpdate | CustomEvent::Resized => {
				if let Some(window) = &self.window {
					window.request_redraw();
				}
			}
		}
	}

	fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
		self.cef_context.handle_window_event(&event);

		match event {
			WindowEvent::CloseRequested => {
				println!("The close button was pressed; stopping");
				event_loop.exit();
			}
			WindowEvent::Resized(physical_size) => {
				self.window_state
					.with(|s| {
						let width = physical_size.width as usize;
						let height = physical_size.height as usize;
						s.width = Some(width);
						s.height = Some(height);
						if let Some(elp) = &s.event_loop_proxy {
							let _ = elp.send_event(CustomEvent::Resized);
						}
						if let Some(event_loop_proxy) = &s.event_loop_proxy {
							let _ = event_loop_proxy.send_event(CustomEvent::DoBrowserWork);
						}
						if let Some(graphics_state) = &mut s.graphics_state {
							graphics_state.resize(width, height);
						}
					})
					.unwrap();
			}

			WindowEvent::RedrawRequested => {
				self.cef_context.work();

				self.window_state
					.with(|s| match s {
						WindowState {
							width: Some(width),
							height: Some(height),
							graphics_state: Some(graphics_state),
							ui_fb,
							..
						} => {
							if let Some(fb) = &*ui_fb {
								graphics_state.update_texture(fb);
								if fb.width() != *width && fb.height() != *height {
									graphics_state.resize(*width, *height);
								}
							} else {
								if let Some(window) = &self.window {
									window.request_redraw();
								}
							}

							match graphics_state.render() {
								Ok(_) => {}
								Err(wgpu::SurfaceError::Lost) => {
									graphics_state.resize(*width, *height);
								}
								Err(wgpu::SurfaceError::OutOfMemory) => {
									event_loop.exit();
								}
								Err(e) => eprintln!("{:?}", e),
							}
						}
						_ => {}
					})
					.unwrap();
			}
			_ => {}
		}

		self.window_state
			.with(|s| {
				if let Some(event_loop_proxy) = &s.event_loop_proxy {
					let _ = event_loop_proxy.send_event(CustomEvent::DoBrowserWork);
				}
			})
			.unwrap();
	}
}

fn main() {
	let cef_context = match cef::Context::<Setup>::new() {
		Ok(c) => c,
		Err(cef::SetupError::Subprocess) => exit(0),
		Err(cef::SetupError::SubprocessFailed(t)) => {
			println!("Subprocess of type {t} failed");
			exit(1);
		}
	};

	let window_state = WindowState::new().handle();

	window_state
		.with(|s| {
			s.width = Some(1200);
			s.height = Some(800);
		})
		.unwrap();

	let event_loop = EventLoop::<CustomEvent>::with_user_event().build().unwrap();
	event_loop.set_control_flow(ControlFlow::Wait);

	window_state.with(|s| s.event_loop_proxy = Some(event_loop.create_proxy())).unwrap();

	let cef_context = match cef_context.init(CefEventHandler::new(window_state.clone())) {
		Ok(c) => c,
		Err(cef::InitError::InitializationFailed) => {
			println!("Cef initialization failed");
			exit(1);
		}
	};

	println!("Cef initialized successfully");

	let mut winit_app = WinitApp::new(window_state, cef_context);

	event_loop.run_app(&mut winit_app).unwrap();

	winit_app.cef_context.shutdown();
}

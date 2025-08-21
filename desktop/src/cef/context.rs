use std::cell::RefCell;
use std::mem::ManuallyDrop;

use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_task_t, CEF_API_VERSION_LAST, cef_base_ref_counted_t, cef_resultcode_t, cef_thread_id_t};
use cef::{App, BrowserSettings, Client, DictionaryValue, ImplBrowser, ImplBrowserHost, ImplCommandLine, RenderHandler, RequestContext, WindowInfo, browser_host_create_browser_sync, initialize};
use cef::{Browser, CefString, ImplTask, Settings, Task, ThreadId, WrapTask, api_hash, args::Args, execute_process, post_task};
use thiserror::Error;
use winit::event::WindowEvent;

use crate::cef::dirs::{cef_cache_dir, cef_data_dir};

use super::input::InputState;
use super::ipc::{MessageType, SendMessage};
use super::scheme_handler::{FRONTEND_DOMAIN, GRAPHITE_SCHEME};
use super::{CefEventHandler, input};

use super::internal::{BrowserProcessAppImpl, BrowserProcessClientImpl, RenderHandlerImpl, RenderProcessAppImpl};

pub(crate) struct Setup {}
pub(crate) struct Initialized {}
pub(crate) trait ContextState {}
impl ContextState for Setup {}
impl ContextState for Initialized {}

pub(crate) struct Context<S: ContextState> {
	args: Args,
	pub(super) browser: Option<Browser>,
	pub(super) input_state: InputState,
	marker: std::marker::PhantomData<S>,
}

impl Context<Setup> {
	pub(crate) fn new() -> Result<Context<Setup>, SetupError> {
		#[cfg(target_os = "macos")]
		let _loader = {
			let loader = library_loader::LibraryLoader::new(&std::env::current_exe().unwrap(), false);
			assert!(loader.load());
			loader
		};
		let _ = api_hash(CEF_API_VERSION_LAST, 0);

		let args = Args::new();
		let cmd = args.as_cmd_line().unwrap();
		let switch = CefString::from("type");
		let is_browser_process = cmd.has_switch(Some(&switch)) != 1;

		if !is_browser_process {
			let process_type = CefString::from(&cmd.switch_value(Some(&switch)));
			let mut app = RenderProcessAppImpl::app();
			let ret = execute_process(Some(args.as_main_args()), Some(&mut app), std::ptr::null_mut());
			if ret >= 0 {
				return Err(SetupError::SubprocessFailed(process_type.to_string()));
			} else {
				return Err(SetupError::Subprocess);
			}
		}

		Ok(Context {
			args,
			browser: None,
			input_state: InputState::default(),
			marker: std::marker::PhantomData::<Setup>,
		})
	}

	pub(crate) fn init(self, event_handler: impl CefEventHandler) -> Result<Context<Initialized>, InitError> {
		let settings = Settings {
			windowless_rendering_enabled: 1,
			#[cfg(target_os = "macos")]
			multi_threaded_message_loop: 0,
			#[cfg(target_os = "macos")]
			external_message_pump: 1,
			#[cfg(not(target_os = "macos"))]
			multi_threaded_message_loop: 1,
			root_cache_path: cef_data_dir().to_str().map(CefString::from).unwrap(),
			cache_path: cef_cache_dir().to_str().map(CefString::from).unwrap(),
			..Default::default()
		};

		// Attention! Wrapping this in an extra App is necessary, otherwise the program still compiles but segfaults
		let mut cef_app = App::new(BrowserProcessAppImpl::new(event_handler.clone()));

		let result = initialize(Some(self.args.as_main_args()), Some(&settings), Some(&mut cef_app), std::ptr::null_mut());
		if result != 1 {
			let cef_exit_code = cef::get_exit_code() as u32;
			if cef_exit_code == cef_resultcode_t::CEF_RESULT_CODE_NORMAL_EXIT_PROCESS_NOTIFIED as u32 {
				return Err(InitError::AlreadyRunning);
			}
			return Err(InitError::InitializationFailed(cef_exit_code));
		}

		let render_handler = RenderHandler::new(RenderHandlerImpl::new(event_handler.clone()));
		let mut client = Client::new(BrowserProcessClientImpl::new(render_handler, event_handler.clone()));

		let url = CefString::from(format!("{GRAPHITE_SCHEME}://{FRONTEND_DOMAIN}/").as_str());
		// let url = CefString::from("chrome://gpu");

		let window_info = WindowInfo {
			windowless_rendering_enabled: 1,
			#[cfg(feature = "accelerated_paint")]
			shared_texture_enabled: if crate::cef::platform::should_enable_hardware_acceleration() { 1 } else { 0 },
			..Default::default()
		};

		let settings = BrowserSettings {
			windowless_frame_rate: crate::consts::CEF_WINDOWLESS_FRAME_RATE,
			background_color: 0x0,
			..Default::default()
		};

		let browser = browser_host_create_browser_sync(
			Some(&window_info),
			Some(&mut client),
			Some(&url),
			Some(&settings),
			Option::<&mut DictionaryValue>::None,
			Option::<&mut RequestContext>::None,
		);

		Ok(Context {
			args: self.args.clone(),
			browser,
			input_state: self.input_state.clone(),
			marker: std::marker::PhantomData::<Initialized>,
		})
	}
}

impl CefContext for Context<Initialized> {
	fn work(&mut self) {
		// cef::do_message_loop_work();
	}

	fn handle_window_event(&mut self, event: WindowEvent) -> Option<WindowEvent> {
		input::handle_window_event(self, event)
	}

	fn notify_of_resize(&self) {
		if let Some(browser) = &self.browser {
			browser.host().unwrap().was_resized();
		}
	}

	fn send_web_message(&self, message: Vec<u8>) {
		self.send_message(MessageType::SendToJS, &message);
	}
}

enum ContextMessage {
	Work,
	WindowEvent(WindowEvent),
	Resize,
	WebMessage(Vec<u8>),
}

// New proxy that uses closure tasks instead of channels
pub struct CefContextSendProxy;

impl CefContext for CefContextSendProxy {
	fn work(&mut self) {
		// CEF handles its own message loop in multi-threaded mode
	}

	fn handle_window_event(&mut self, event: WindowEvent) -> Option<WindowEvent> {
		let _event_clone = event.clone();
		post_closure_task(ThreadId::from(cef_thread_id_t::TID_UI), move || {
			BROWSER.with(|b| {
				if let Some(_browser) = b.borrow().as_ref() {
					// Forward window event to CEF input handling on UI thread
					// TODO: Implement input handling directly here
				}
			});
		});
		Some(event)
	}

	fn notify_of_resize(&self) {
		post_closure_task(ThreadId::from(cef_thread_id_t::TID_UI), || {
			BROWSER.with(|b| {
				if let Some(browser) = b.borrow().as_ref() {
					if let Some(host) = browser.host() {
						host.was_resized();
					}
				}
			});
		});
	}

	fn send_web_message(&self, message: Vec<u8>) {
		post_closure_task(ThreadId::from(cef_thread_id_t::TID_UI), move || {
			BROWSER.with(|b| {
				if let Some(browser) = b.borrow().as_ref() {
					// Inline the send_message functionality
					use super::ipc::{MessageType, SendMessage};
					if let Some(frame) = browser.main_frame() {
						let message_bytes = &message;
						frame.send_message(MessageType::SendToJS, message_bytes);
					}
				}
			});
		});
	}
}

impl CefContext for std::sync::mpsc::Sender<ContextMessage> {
	fn work(&mut self) {
		let _ = self.send(ContextMessage::Work);
	}

	fn handle_window_event(&mut self, event: WindowEvent) -> Option<WindowEvent> {
		let _ = self.send(ContextMessage::WindowEvent(event.clone()));
		Some(event)
	}

	fn notify_of_resize(&self) {
		let _ = self.send(ContextMessage::Resize);
	}

	fn send_web_message(&self, message: Vec<u8>) {
		let _ = self.send(ContextMessage::WebMessage(message));
	}
}

pub(crate) fn cef_context(context: Context<Setup>, event_handler: impl CefEventHandler + Send + 'static) -> Box<dyn CefContext> {
	#[cfg(target_os = "macos")]
	{
		// On macOS, use the old synchronous approach
		let context = context.init(event_handler).unwrap();
		Box::new(context)
	}

	#[cfg(not(target_os = "macos"))]
	{
		// On other platforms, use the new multi-threaded approach
		// Initialize CEF first
		let _context = context.init(event_handler.clone()).unwrap();

		// Post browser creation task to CEF's UI thread
		post_closure_task(ThreadId::from(cef_thread_id_t::TID_UI), move || {
			// Create browser on CEF's UI thread
			let render_handler = RenderHandler::new(super::internal::RenderHandlerImpl::new(event_handler.clone()));
			let mut client = Client::new(super::internal::BrowserProcessClientImpl::new(render_handler, event_handler));

			let url = CefString::from(format!("{}://{}/", super::scheme_handler::GRAPHITE_SCHEME, super::scheme_handler::FRONTEND_DOMAIN).as_str());

			let window_info = WindowInfo {
				windowless_rendering_enabled: 1,
				#[cfg(feature = "accelerated_paint")]
				shared_texture_enabled: if super::platform::should_enable_hardware_acceleration() { 1 } else { 0 },
				..Default::default()
			};

			let settings = BrowserSettings {
				windowless_frame_rate: 120,
				background_color: 0x0,
				..Default::default()
			};

			let browser = browser_host_create_browser_sync(
				Some(&window_info),
				Some(&mut client),
				Some(&url),
				Some(&settings),
				Option::<&mut DictionaryValue>::None,
				Option::<&mut RequestContext>::None,
			);

			// Store browser in thread-local storage
			BROWSER.with(|b| {
				*b.borrow_mut() = browser;
			});
		});

		Box::new(CefContextSendProxy)
	}
}

// Keep the old channel-based implementation for reference/fallback
pub(crate) fn cef_context_channel_based(context: Context<Setup>, event_handler: impl CefEventHandler + Send + 'static) -> impl CefContext {
	let (tx, rx) = std::sync::mpsc::channel();
	let manually_drop_context = ManuallyDrop::new(context);
	let args = manually_drop_context.args.clone();

	let arg_bytes: [u8; 64] = unsafe { std::mem::transmute(args) };
	std::thread::spawn(move || {
		let args = unsafe { std::mem::transmute(arg_bytes) };
		let context = Context {
			args,
			browser: None,
			input_state: Default::default(),
			marker: Default::default(),
		};
		let mut context = context.init(event_handler).unwrap();
		loop {
			let msg = rx.recv().unwrap();
			match msg {
				ContextMessage::Work => context.work(),
				ContextMessage::WindowEvent(window_event) => {
					context.handle_window_event(window_event);
				}
				ContextMessage::Resize => context.notify_of_resize(),
				ContextMessage::WebMessage(message) => context.send_web_message(message),
			};
		}
	});
	tx
}

pub(crate) trait CefContext {
	fn work(&mut self);

	fn handle_window_event(&mut self, event: WindowEvent) -> Option<WindowEvent>;

	fn notify_of_resize(&self);

	fn send_web_message(&self, message: Vec<u8>);
}

// Thread-local browser storage for UI thread
thread_local! {
	static BROWSER: RefCell<Option<Browser>> = RefCell::new(None);
}

// Closure-based task wrapper following CEF patterns
pub struct ClosureTask<F> {
	object: *mut RcImpl<_cef_task_t, Self>,
	closure: RefCell<Option<F>>,
}

impl<F: FnOnce() + Send + 'static> ClosureTask<F> {
	pub fn new(closure: F) -> Self {
		Self {
			object: std::ptr::null_mut(),
			closure: RefCell::new(Some(closure)),
		}
	}
}

impl<F: FnOnce() + Send + 'static> ImplTask for ClosureTask<F> {
	fn execute(&self) {
		if let Some(closure) = self.closure.borrow_mut().take() {
			closure();
		}
	}

	fn get_raw(&self) -> *mut _cef_task_t {
		self.object.cast()
	}
}

impl<F: FnOnce() + Send + 'static> Clone for ClosureTask<F> {
	fn clone(&self) -> Self {
		unsafe {
			if !self.object.is_null() {
				let rc_impl = &mut *self.object;
				rc_impl.interface.add_ref();
			}
		}
		Self {
			object: self.object,
			closure: RefCell::new(None), // Closure can only be executed once
		}
	}
}

impl<F: FnOnce() + Send + 'static> Rc for ClosureTask<F> {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}

impl<F: FnOnce() + Send + 'static> WrapTask for ClosureTask<F> {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_task_t, Self>) {
		self.object = object;
	}
}

// Convenience function for posting closure tasks
pub fn post_closure_task<F>(thread_id: ThreadId, closure: F)
where
	F: FnOnce() + Send + 'static,
{
	let closure_task = ClosureTask::new(closure);
	let mut task = Task::new(closure_task);
	post_task(thread_id, Some(&mut task));
}

impl<S: ContextState> Drop for Context<S> {
	fn drop(&mut self) {
		if self.browser.is_some() {
			cef::shutdown();
		}
	}
}

#[derive(Error, Debug)]
pub(crate) enum SetupError {
	#[error("this is the sub process should exit immediately")]
	Subprocess,
	#[error("subprocess returned non zero exit code")]
	SubprocessFailed(String),
}

#[derive(Error, Debug)]
pub(crate) enum InitError {
	#[error("initialization failed")]
	InitializationFailed(u32),
	#[error("Another instance is already running")]
	AlreadyRunning,
}

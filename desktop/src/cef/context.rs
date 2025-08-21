use cef::sys::{CEF_API_VERSION_LAST, cef_resultcode_t, cef_thread_id_t};
use cef::{App, BrowserSettings, Client, DictionaryValue, ImplCommandLine, RenderHandler, RequestContext, WindowInfo, browser_host_create_browser_sync, initialize};
use cef::{Browser, CefString, Settings, ThreadId, api_hash, args::Args, execute_process};
use context_impl::BROWSER;
use winit::event::WindowEvent;

use crate::cef::dirs::{cef_cache_dir, cef_data_dir};

use super::CefEventHandler;
use super::input::InputState;
use super::scheme_handler::{FRONTEND_DOMAIN, GRAPHITE_SCHEME};

use super::internal::{BrowserProcessAppImpl, BrowserProcessClientImpl, RenderHandlerImpl, RenderProcessAppImpl};

pub(crate) struct Setup {}
pub(crate) struct Initialized {}
pub(crate) trait ContextState {}
impl ContextState for Setup {}
impl ContextState for Initialized {}

mod cef_task;
pub(super) mod context_impl;

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

pub(crate) trait CefContext {
	fn work(&mut self);

	fn handle_window_event(&mut self, event: WindowEvent) -> Option<WindowEvent>;

	fn notify_of_resize(&self);

	fn send_web_message(&self, message: Vec<u8>);
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
		cef_task::post_closure_task(ThreadId::from(cef_thread_id_t::TID_UI), move || {
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
				*b.borrow_mut() = Some((browser.expect("failed to initialize browser"), InputState::default()));
			});
		});

		Box::new(context_impl::CefContextSendProxy)
	}
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum SetupError {
	#[error("this is the sub process should exit immediately")]
	Subprocess,
	#[error("subprocess returned non zero exit code")]
	SubprocessFailed(String),
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum InitError {
	#[error("initialization failed")]
	InitializationFailed(u32),
	#[error("Another instance is already running")]
	AlreadyRunning,
}

use cef::sys::{CEF_API_VERSION_LAST, cef_resultcode_t};
use cef::{App, BrowserSettings, Client, DictionaryValue, ImplBrowser, ImplBrowserHost, ImplCommandLine, RenderHandler, RequestContext, WindowInfo, browser_host_create_browser_sync, initialize};
use cef::{Browser, CefString, Settings, api_hash, args::Args, execute_process};
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
	pub(crate) browser: Option<Browser>,
	pub(crate) input_state: InputState,
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
			multi_threaded_message_loop: 0,
			external_message_pump: 1,
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

		let window_info = WindowInfo {
			windowless_rendering_enabled: 1,
			..Default::default()
		};

		let settings = BrowserSettings {
			windowless_frame_rate: 60,
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

impl Context<Initialized> {
	pub(crate) fn work(&mut self) {
		cef::do_message_loop_work();
	}

	pub(crate) fn handle_window_event(&mut self, event: WindowEvent) -> Option<WindowEvent> {
		input::handle_window_event(self, event)
	}

	pub(crate) fn notify_of_resize(&self) {
		if let Some(browser) = &self.browser {
			browser.host().unwrap().was_resized();
		}
	}

	pub(crate) fn send_web_message(&self, message: &[u8]) {
		self.send_message(MessageType::SendToJS, message);
	}
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

use cef::sys::CEF_API_VERSION_LAST;
use cef::{api_hash, args::Args, execute_process, Browser, CefString, Settings};
use cef::{browser_host_create_browser_sync, initialize, BrowserSettings, DictionaryValue, ImplCommandLine, RequestContext, WindowInfo};
use thiserror::Error;
use winit::event::WindowEvent;

use super::input::{handle_window_event, InputState};
use super::EventHandler;

use super::internal::{AppImpl, ClientImpl, NonBrowserAppImpl, RenderHandlerImpl};

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
		cmd.append_switch(Some(&CefString::from("disable-gpu")));
		cmd.append_switch(Some(&CefString::from("disable-gpu-compositing")));
		let is_browser_process = cmd.has_switch(Some(&switch)) != 1;
		if !is_browser_process {
			let process_type = CefString::from(&cmd.switch_value(Some(&switch)));
			let mut app = NonBrowserAppImpl::new();
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

	pub(crate) fn init(self, event_handler: impl EventHandler) -> Result<Context<Initialized>, InitError> {
		let mut settings = Settings::default();
		settings.windowless_rendering_enabled = 1;
		settings.multi_threaded_message_loop = 0;
		settings.external_message_pump = 1;

		let mut cef_app = AppImpl::new(event_handler.clone());

		let res = initialize(Some(self.args.as_main_args()), Some(&settings), Some(&mut cef_app), std::ptr::null_mut());
		if res != 1 {
			return Err(InitError::InitializationFailed);
		}

		let render_handler = RenderHandlerImpl::new(event_handler.clone());
		let mut client = ClientImpl::new(render_handler);

		let url = CefString::from("graphite://frontend/");

		let mut window_info = WindowInfo::default();
		window_info.windowless_rendering_enabled = 1;

		let mut settings = BrowserSettings::default();
		settings.windowless_frame_rate = 60;
		settings.background_color = 0x0;

		let browser = browser_host_create_browser_sync(
			Some(&window_info),
			Some(&mut client),
			Some(&url),
			Some(&settings),
			Option::<&mut DictionaryValue>::None,
			Option::<&mut RequestContext>::None,
		);

		Ok(Context {
			args: self.args,
			browser,
			input_state: self.input_state,
			marker: std::marker::PhantomData::<Initialized>,
		})
	}
}

impl Context<Initialized> {
	pub(crate) fn work(&mut self) {
		cef::do_message_loop_work();
	}

	pub(crate) fn handle_window_event(&mut self, event: &WindowEvent) {
		handle_window_event(self, event);
	}

	pub(crate) fn shutdown(self) {
		cef::shutdown();
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
	InitializationFailed,
}

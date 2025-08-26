use cef::args::Args;
use cef::sys::{CEF_API_VERSION_LAST, cef_resultcode_t};
use cef::{
	App, BrowserSettings, CefString, Client, DictionaryValue, ImplCommandLine, RenderHandler, RequestContext, Settings, WindowInfo, api_hash, browser_host_create_browser_sync, execute_process,
};

use super::CefContext;
use super::singlethreaded::SingleThreadedCefContext;
use crate::cef::CefEventHandler;
use crate::cef::consts::{RESOURCE_DOMAIN, RESOURCE_SCHEME};
use crate::cef::dirs::{cef_cache_dir, cef_data_dir};
use crate::cef::input::InputState;
use crate::cef::internal::{BrowserProcessAppImpl, BrowserProcessClientImpl, RenderHandlerImpl, RenderProcessAppImpl};

pub(crate) struct CefContextBuilder<H: CefEventHandler> {
	pub(crate) args: Args,
	pub(crate) is_sub_process: bool,
	_marker: std::marker::PhantomData<H>,
}

unsafe impl<H: CefEventHandler> Send for CefContextBuilder<H> {}

impl<H: CefEventHandler> CefContextBuilder<H> {
	pub(crate) fn new() -> Self {
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
		let is_sub_process = cmd.has_switch(Some(&switch)) == 1;

		Self {
			args,
			is_sub_process,
			_marker: std::marker::PhantomData,
		}
	}

	pub(crate) fn is_sub_process(&self) -> bool {
		self.is_sub_process
	}

	pub(crate) fn execute_sub_process(&self) -> SetupError {
		let cmd = self.args.as_cmd_line().unwrap();
		let switch = CefString::from("type");
		let process_type = CefString::from(&cmd.switch_value(Some(&switch)));
		let mut app = RenderProcessAppImpl::<H>::app();
		let ret = execute_process(Some(self.args.as_main_args()), Some(&mut app), std::ptr::null_mut());
		if ret >= 0 {
			SetupError::SubprocessFailed(process_type.to_string())
		} else {
			SetupError::Subprocess
		}
	}

	#[cfg(target_os = "macos")]
	pub(crate) fn initialize(self, event_handler: H) -> Result<impl CefContext, InitError> {
		let settings = Settings {
			windowless_rendering_enabled: 1,
			multi_threaded_message_loop: 0,
			external_message_pump: 1,
			root_cache_path: cef_data_dir().to_str().map(CefString::from).unwrap(),
			cache_path: cef_cache_dir().to_str().map(CefString::from).unwrap(),
			..Default::default()
		};

		self.initialize_inner(&event_handler, settings)?;

		create_browser(event_handler)
	}

	#[cfg(not(target_os = "macos"))]
	pub(crate) fn initialize(self, event_handler: H) -> Result<impl CefContext, InitError> {
		let settings = Settings {
			windowless_rendering_enabled: 1,
			multi_threaded_message_loop: 1,
			root_cache_path: cef_data_dir().to_str().map(CefString::from).unwrap(),
			cache_path: cef_cache_dir().to_str().map(CefString::from).unwrap(),
			..Default::default()
		};

		self.initialize_inner(&event_handler, settings)?;

		super::multithreaded::run_on_ui_thread(move || match create_browser(event_handler) {
			Ok(context) => {
				super::multithreaded::CONTEXT.with(|b| {
					*b.borrow_mut() = Some(context);
				});
			}
			Err(e) => {
				tracing::error!("Failed to initialize CEF context: {:?}", e);
				std::process::exit(1);
			}
		});

		Ok(super::multithreaded::MultiThreadedCefContextProxy)
	}

	fn initialize_inner(self, event_handler: &H, settings: Settings) -> Result<(), InitError> {
		let mut cef_app = App::new(BrowserProcessAppImpl::new(event_handler.clone()));
		let result = cef::initialize(Some(self.args.as_main_args()), Some(&settings), Some(&mut cef_app), std::ptr::null_mut());
		// Attention! Wrapping this in an extra App is necessary, otherwise the program still compiles but segfaults

		if result != 1 {
			let cef_exit_code = cef::get_exit_code() as u32;
			if cef_exit_code == cef_resultcode_t::CEF_RESULT_CODE_NORMAL_EXIT_PROCESS_NOTIFIED as u32 {
				return Err(InitError::AlreadyRunning);
			}
			return Err(InitError::InitializationFailed(cef_exit_code));
		}
		Ok(())
	}
}

fn create_browser<H: CefEventHandler>(event_handler: H) -> Result<SingleThreadedCefContext, InitError> {
	let render_handler = RenderHandler::new(RenderHandlerImpl::new(event_handler.clone()));
	let mut client = Client::new(BrowserProcessClientImpl::new(render_handler, event_handler.clone()));

	let url = CefString::from(format!("{RESOURCE_SCHEME}://{RESOURCE_DOMAIN}/").as_str());

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

	if let Some(browser) = browser {
		Ok(SingleThreadedCefContext {
			browser,
			input_state: InputState::default(),
		})
	} else {
		tracing::error!("Failed to create browser");
		Err(InitError::BrowserCreationFailed)
	}
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum SetupError {
	#[error("This is the sub process should exit immediately")]
	Subprocess,
	#[error("Subprocess returned non zero exit code")]
	SubprocessFailed(String),
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum InitError {
	#[error("Initialization failed")]
	InitializationFailed(u32),
	#[error("Browser creation failed")]
	BrowserCreationFailed,
	#[error("Another instance is already running")]
	AlreadyRunning,
}

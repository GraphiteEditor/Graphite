use std::path::{Path, PathBuf};

use cef::args::Args;
use cef::sys::{CEF_API_VERSION_LAST, cef_resultcode_t};
use cef::{
	App, BrowserSettings, CefString, Client, DictionaryValue, ImplCommandLine, ImplRequestContext, RenderHandler, RequestContextSettings, SchemeHandlerFactory, Settings, WindowInfo, api_hash,
	browser_host_create_browser_sync, execute_process,
};

use super::CefContext;
use super::singlethreaded::SingleThreadedCefContext;
use crate::cef::CefEventHandler;
use crate::cef::consts::{RESOURCE_DOMAIN, RESOURCE_SCHEME};
use crate::cef::dirs::create_instance_dir;
use crate::cef::input::InputState;
use crate::cef::internal::{BrowserProcessAppImpl, BrowserProcessClientImpl, RenderHandlerImpl, RenderProcessAppImpl, SchemeHandlerFactoryImpl};

pub(crate) struct CefContextBuilder<H: CefEventHandler> {
	pub(crate) args: Args,
	pub(crate) is_sub_process: bool,
	_marker: std::marker::PhantomData<H>,
}

unsafe impl<H: CefEventHandler> Send for CefContextBuilder<H> {}

impl<H: CefEventHandler> CefContextBuilder<H> {
	pub(crate) fn new() -> Self {
		Self::new_inner(false)
	}

	pub(crate) fn new_helper() -> Self {
		Self::new_inner(true)
	}

	fn new_inner(helper: bool) -> Self {
		#[cfg(target_os = "macos")]
		let _loader = {
			let loader = cef::library_loader::LibraryLoader::new(&std::env::current_exe().unwrap(), helper);
			assert!(loader.load());
			loader
		};
		#[cfg(not(target_os = "macos"))]
		let _ = helper;

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

	fn common_settings(instance_dir: &Path) -> Settings {
		Settings {
			windowless_rendering_enabled: 1,
			root_cache_path: instance_dir.to_str().map(CefString::from).unwrap(),
			cache_path: CefString::from(""),
			disable_signal_handlers: 1,
			..Default::default()
		}
	}

	#[cfg(target_os = "macos")]
	pub(crate) fn initialize(self, event_handler: H, disable_gpu_acceleration: bool) -> Result<impl CefContext, InitError> {
		let instance_dir = create_instance_dir();

		let exe = std::env::current_exe().expect("cannot get current exe path");
		let app_root = exe.parent().and_then(|p| p.parent()).expect("bad path structure").parent().expect("bad path structure");

		let settings = Settings {
			main_bundle_path: CefString::from(app_root.to_str().unwrap()),
			multi_threaded_message_loop: 0,
			external_message_pump: 1,
			no_sandbox: 1, // GPU helper crashes when running with sandbox
			..Self::common_settings(&instance_dir)
		};

		self.initialize_inner(&event_handler, settings)?;

		create_browser(event_handler, instance_dir, disable_gpu_acceleration)
	}

	#[cfg(not(target_os = "macos"))]
	pub(crate) fn initialize(self, event_handler: H, disable_gpu_acceleration: bool) -> Result<impl CefContext, InitError> {
		let instance_dir = create_instance_dir();

		let settings = Settings {
			multi_threaded_message_loop: 1,
			..Self::common_settings(&instance_dir)
		};

		self.initialize_inner(&event_handler, settings)?;

		super::multithreaded::run_on_ui_thread(move || match create_browser(event_handler, instance_dir, disable_gpu_acceleration) {
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
		// Attention! Wrapping this in an extra App is necessary, otherwise the program still compiles but segfaults
		let mut cef_app = App::new(BrowserProcessAppImpl::new(event_handler.clone()));

		let result = cef::initialize(Some(self.args.as_main_args()), Some(&settings), Some(&mut cef_app), std::ptr::null_mut());
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

fn create_browser<H: CefEventHandler>(event_handler: H, instance_dir: PathBuf, disable_gpu_acceleration: bool) -> Result<SingleThreadedCefContext, InitError> {
	let render_handler = RenderHandler::new(RenderHandlerImpl::new(event_handler.clone()));
	let mut client = Client::new(BrowserProcessClientImpl::new(render_handler, event_handler.clone()));

	#[cfg(feature = "accelerated_paint")]
	let use_accelerated_paint = if disable_gpu_acceleration {
		false
	} else {
		crate::cef::platform::should_enable_hardware_acceleration()
	};

	let window_info = WindowInfo {
		windowless_rendering_enabled: 1,
		#[cfg(feature = "accelerated_paint")]
		shared_texture_enabled: use_accelerated_paint as i32,
		..Default::default()
	};

	let settings = BrowserSettings {
		windowless_frame_rate: crate::consts::CEF_WINDOWLESS_FRAME_RATE,
		background_color: 0x0,
		..Default::default()
	};

	let Some(mut incognito_request_context) = cef::request_context_create_context(
		Some(&RequestContextSettings {
			persist_session_cookies: 0,
			cache_path: CefString::from(""),
			..Default::default()
		}),
		Option::<&mut cef::RequestContextHandler>::None,
	) else {
		return Err(InitError::RequestContextCreationFailed);
	};

	let mut scheme_handler_factory = SchemeHandlerFactory::new(SchemeHandlerFactoryImpl::new(event_handler.clone()));
	incognito_request_context.clear_scheme_handler_factories();
	incognito_request_context.register_scheme_handler_factory(Some(&CefString::from(RESOURCE_SCHEME)), Some(&CefString::from(RESOURCE_DOMAIN)), Some(&mut scheme_handler_factory));

	let url = CefString::from(format!("{RESOURCE_SCHEME}://{RESOURCE_DOMAIN}/").as_str());

	let browser = browser_host_create_browser_sync(
		Some(&window_info),
		Some(&mut client),
		Some(&url),
		Some(&settings),
		Option::<&mut DictionaryValue>::None,
		Some(&mut incognito_request_context),
	);

	if let Some(browser) = browser {
		Ok(SingleThreadedCefContext {
			browser,
			input_state: InputState::default(),
			instance_dir,
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
	#[error("Request context creation failed")]
	RequestContextCreationFailed,
	#[error("Another instance is already running")]
	AlreadyRunning,
}

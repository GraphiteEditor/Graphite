use cef::args::Args;
use cef::sys::{CEF_API_VERSION_LAST, cef_log_severity_t};
use cef::{
	App, BrowserSettings, CefString, Client, DictionaryValue, ImplCommandLine, ImplRequestContext, LogSeverity, RequestContextSettings, SchemeHandlerFactory, Settings, WindowInfo, api_hash,
	browser_host_create_browser_sync, execute_process,
};
use std::path::Path;

use super::CefContext;
use super::singlethreaded::SingleThreadedCefContext;
use crate::cef::CefEventHandler;
use crate::cef::consts::{RESOURCE_DOMAIN, RESOURCE_SCHEME};
use crate::cef::input::InputState;
use crate::cef::internal::{BrowserProcessAppImpl, BrowserProcessClientImpl, RenderProcessAppImpl, SchemeHandlerFactoryImpl};
use crate::dirs::TempDir;

pub(crate) struct CefContextBuilder<H: CefEventHandler> {
	pub(crate) args: Args,
	pub(crate) is_sub_process: bool,
	_marker: std::marker::PhantomData<H>,
}

unsafe impl<H: CefEventHandler> Send for CefContextBuilder<H> {}

impl<H: CefEventHandler> CefContextBuilder<H> {
	pub(crate) fn new() -> Self {
		Self::new_impl(false)
	}
	pub(crate) fn new_helper() -> Self {
		Self::new_impl(true)
	}

	fn new_impl(helper: bool) -> Self {
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
		let is_sub_process = args.as_cmd_line().unwrap().has_switch(Some(&"type".into())) == 1;
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
		let process_type = CefString::from(&cmd.switch_value(Some(&"type".into())));
		let mut app = RenderProcessAppImpl::<H>::app();
		let ret = execute_process(Some(self.args.as_main_args()), Some(&mut app), std::ptr::null_mut());
		if ret >= 0 {
			SetupError::SubprocessFailed(process_type.to_string())
		} else {
			SetupError::Subprocess
		}
	}

	#[cfg(target_os = "macos")]
	pub(crate) fn create(self, event_handler: H, disable_gpu_acceleration: bool) -> Result<impl CefContext, InitError> {
		let instance_dir = TempDir::new().expect("Failed to create temporary directory for CEF instance");
		let accelerated_paint = accelerated_paint(disable_gpu_acceleration);
		self.build_inner(&event_handler, instance_dir.as_ref(), accelerated_paint)?;
		create_browser(event_handler, instance_dir, accelerated_paint)
	}

	#[cfg(not(target_os = "macos"))]
	pub(crate) fn create(self, event_handler: H, disable_gpu_acceleration: bool) -> Result<impl CefContext, InitError> {
		let instance_dir = TempDir::new().expect("Failed to create temporary directory for CEF instance");
		let accelerated_paint = accelerated_paint(disable_gpu_acceleration);
		self.build_inner(&event_handler, instance_dir.as_ref(), accelerated_paint)?;
		super::multithreaded::run_on_ui_thread(move || match create_browser(event_handler, instance_dir, accelerated_paint) {
			Ok(context) => super::multithreaded::CONTEXT.with(|b| *b.borrow_mut() = Some(context)),
			Err(e) => panic!("Failed to initialize CEF context: {:?}", e),
		});
		Ok(super::multithreaded::MultiThreadedCefContextProxy)
	}

	fn build_inner(self, event_handler: &H, instance_dir: &Path, accelerated_paint: bool) -> Result<(), InitError> {
		let mut cef_app = App::new(BrowserProcessAppImpl::new(event_handler.duplicate(), accelerated_paint));
		let result = cef::initialize(Some(self.args.as_main_args()), Some(&platform_settings(instance_dir)), Some(&mut cef_app), std::ptr::null_mut());
		if result != 1 {
			return Err(InitError::InitializationFailureCode(cef::get_exit_code() as u32));
		}
		Ok(())
	}
}

fn accelerated_paint(disable_gpu_acceleration: bool) -> bool {
	#[cfg(feature = "accelerated_paint")]
	{
		!disable_gpu_acceleration && crate::cef::platform::should_enable_hardware_acceleration()
	}
	#[cfg(not(feature = "accelerated_paint"))]
	{
		let _ = disable_gpu_acceleration;
		false
	}
}

fn platform_settings(instance_dir: &Path) -> Settings {
	let log_severity = LogSeverity::from(match std::env::var("GRAPHITE_BROWSER_LOG").as_deref() {
		Ok("debug") => cef_log_severity_t::LOGSEVERITY_VERBOSE,
		Ok("info") => cef_log_severity_t::LOGSEVERITY_INFO,
		Ok("warn") => cef_log_severity_t::LOGSEVERITY_WARNING,
		Ok("error") => cef_log_severity_t::LOGSEVERITY_ERROR,
		Ok("none") => cef_log_severity_t::LOGSEVERITY_DISABLE,
		_ => cef_log_severity_t::LOGSEVERITY_FATAL,
	});

	let base = Settings {
		windowless_rendering_enabled: 1,
		root_cache_path: instance_dir.to_str().map(CefString::from).unwrap(),
		cache_path: "".into(),
		disable_signal_handlers: 1,
		log_severity,
		..Default::default()
	};

	#[cfg(target_os = "macos")]
	{
		let exe = std::env::current_exe().expect("cannot get current exe path");
		let app_root = exe.parent().and_then(|p| p.parent()).expect("bad path structure").parent().expect("bad path structure");
		return Settings {
			main_bundle_path: app_root.to_str().map(CefString::from).unwrap(),
			multi_threaded_message_loop: 0,
			external_message_pump: 1,
			no_sandbox: 1, // GPU helper crashes when running with sandbox
			..base
		};
	}

	#[cfg(not(target_os = "macos"))]
	Settings {
		multi_threaded_message_loop: 1,
		#[cfg(target_os = "linux")]
		no_sandbox: 1,
		..base
	}
}

fn create_browser<H: CefEventHandler>(event_handler: H, instance_dir: TempDir, accelerated_paint: bool) -> Result<SingleThreadedCefContext, InitError> {
	let mut client = Client::new(BrowserProcessClientImpl::new(&event_handler));

	let window_info = WindowInfo {
		windowless_rendering_enabled: 1,
		#[cfg(feature = "accelerated_paint")]
		shared_texture_enabled: accelerated_paint as i32,
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
			cache_path: "".into(),
			..Default::default()
		}),
		Option::<&mut cef::RequestContextHandler>::None,
	) else {
		return Err(InitError::RequestContextCreationFailed);
	};

	let mut scheme_handler_factory = SchemeHandlerFactory::new(SchemeHandlerFactoryImpl::new(event_handler.duplicate()));
	incognito_request_context.clear_scheme_handler_factories();
	incognito_request_context.register_scheme_handler_factory(Some(&RESOURCE_SCHEME.into()), Some(&RESOURCE_DOMAIN.into()), Some(&mut scheme_handler_factory));

	let url = format!("{RESOURCE_SCHEME}://{RESOURCE_DOMAIN}/");
	browser_host_create_browser_sync(
		Some(&window_info),
		Some(&mut client),
		Some(&url.as_str().into()),
		Some(&settings),
		Option::<&mut DictionaryValue>::None,
		Some(&mut incognito_request_context),
	)
	.map(|browser| SingleThreadedCefContext {
		event_handler: Box::new(event_handler),
		browser,
		input_state: InputState::default(),
		_instance_dir: instance_dir,
	})
	.ok_or_else(|| {
		tracing::error!("Failed to create browser");
		InitError::BrowserCreationFailed
	})
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum SetupError {
	#[error("This is the sub process should exit immediately")]
	Subprocess,
	#[error("Subprocess returned non zero exit code: {0}")]
	SubprocessFailed(String),
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum InitError {
	#[error("Initialization failed with code: {0}")]
	InitializationFailureCode(u32),
	#[error("Browser creation failed")]
	BrowserCreationFailed,
	#[error("Request context creation failed")]
	RequestContextCreationFailed,
}

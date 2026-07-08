use cef::args::Args;
use cef::sys::{CEF_API_VERSION_LAST, cef_log_severity_t, cef_thread_id_t};
use cef::{
	App, Browser, BrowserSettings, CefString, Client, DictionaryValue, ImplBrowser, ImplBrowserHost, ImplCommandLine, ImplRequestContext, LogSeverity, RequestContextSettings, SchemeHandlerFactory,
	Settings, Task, ThreadId, WindowInfo, api_hash, browser_host_create_browser_sync, execute_process, post_task,
};
use std::cell::RefCell;
use std::marker::PhantomData;
use std::path::Path;
use std::sync::mpsc::Sender;

use crate::consts::{RESOURCE_DOMAIN, RESOURCE_SCHEME, WINDOWLESS_FRAME_RATE};
use crate::delegate::BrowserDelegate;
use crate::dirs::TempDir;
use crate::frames::FrameStreamer;
use crate::input::{self, InputEvent};
use crate::internal::task::ClosureTask;
use crate::internal::{BrowserProcessAppImpl, BrowserProcessClientImpl, RenderProcessAppImpl, SchemeHandlerFactoryImpl};
use crate::ipc::{MessageType, SendMessage};
use crate::view::ViewInfoUpdate;

thread_local! {
	static CONTEXT: RefCell<Option<BrowserContext>> = const { RefCell::new(None) };
}

pub(crate) struct CefContext {
	_not_send: PhantomData<*const ()>, // impl !Send for CefContext
}

impl CefContext {
	pub(crate) fn create(delegate: BrowserDelegate, frames: FrameStreamer, view_info_sender: Sender<ViewInfoUpdate>, accelerated_paint: bool) -> Result<Self, InitError> {
		let args = bootstrap(false);
		#[cfg(target_os = "macos")]
		crate::platform::mac::install_application();

		let instance_dir = TempDir::new().map_err(|e| InitError::InstanceDirectoryCreationFailed(e.to_string()))?;
		initialize(&args, instance_dir.as_ref(), accelerated_paint)?;

		let (created_tx, created_rx) = std::sync::mpsc::channel();
		let install_browser = move || {
			let result = create_browser(delegate, frames, view_info_sender, instance_dir, accelerated_paint).map(|context| CONTEXT.with(|b| *b.borrow_mut() = Some(context)));
			let _ = created_tx.send(result);
		};
		#[cfg(target_os = "macos")]
		install_browser();
		#[cfg(not(target_os = "macos"))]
		run_on_ui_thread(install_browser);

		created_rx.recv().unwrap_or(Err(InitError::BrowserCreationFailed))?;
		Ok(Self { _not_send: PhantomData })
	}

	#[cfg(not(target_os = "macos"))]
	pub(crate) fn run<R: Send + 'static>(self, control: impl FnOnce(CefContextHandle) -> R + Send + 'static) -> R {
		let result = control(CefContextHandle);
		let (dropped_sender, dropped_receiver) = std::sync::mpsc::channel();
		run_on_ui_thread(move || {
			drop(CONTEXT.take());
			let _ = dropped_sender.send(());
		});
		let _ = dropped_receiver.recv();
		cef::shutdown();
		result
	}

	#[cfg(target_os = "macos")]
	pub(crate) fn run<R: Send + 'static>(self, control: impl FnOnce(CefContextHandle) -> R + Send + 'static) -> R {
		let (result_sender, result_receiver) = std::sync::mpsc::channel();
		let control_thread = std::thread::Builder::new()
			.name("cef-host-control".to_string())
			.spawn(move || {
				let result = control(CefContextHandle);
				with_context(|context| {
					context.browser.host().unwrap().close_browser(1);
				});
				run_on_ui_thread(cef::quit_message_loop);
				let _ = result_sender.send(result);
			})
			.expect("Failed to spawn the CEF control thread");
		cef::run_message_loop();
		drop(CONTEXT.take());
		cef::shutdown();
		let _ = control_thread.join();
		result_receiver.recv().expect("The CEF control thread ended without a result")
	}
}

pub(crate) fn execute_helper_process() -> std::process::ExitCode {
	let args = bootstrap(true);
	assert_eq!(args.as_cmd_line().unwrap().has_switch(Some(&"type".into())), 1, "Not a CEF helper process");
	let mut app = RenderProcessAppImpl::app();
	let code = execute_process(Some(args.as_main_args()), Some(&mut app), std::ptr::null_mut());
	std::process::ExitCode::from(code as u8)
}

fn bootstrap(helper: bool) -> Args {
	#[cfg(target_os = "macos")]
	let _loader = {
		let loader = cef::library_loader::LibraryLoader::new(&std::env::current_exe().unwrap(), helper);
		assert!(loader.load());
		loader
	};
	#[cfg(not(target_os = "macos"))]
	let _ = helper;

	let _ = api_hash(CEF_API_VERSION_LAST, 0);
	Args::new()
}

fn initialize(args: &Args, instance_dir: &Path, accelerated_paint: bool) -> Result<(), InitError> {
	let mut app = App::new(BrowserProcessAppImpl::new(accelerated_paint));
	if cef::initialize(Some(args.as_main_args()), Some(&platform_settings(instance_dir)), Some(&mut app), std::ptr::null_mut()) != 1 {
		return Err(InitError::InitializationFailureCode(cef::get_exit_code() as u32));
	}
	Ok(())
}

fn platform_settings(instance_dir: &Path) -> Settings {
	let log_severity = match std::env::var("GRAPHITE_BROWSER_LOG").unwrap_or_default().to_lowercase().as_str() {
		"debug" => cef_log_severity_t::LOGSEVERITY_VERBOSE,
		"info" => cef_log_severity_t::LOGSEVERITY_INFO,
		"warn" => cef_log_severity_t::LOGSEVERITY_WARNING,
		"error" => cef_log_severity_t::LOGSEVERITY_ERROR,
		"none" => cef_log_severity_t::LOGSEVERITY_DISABLE,
		_ => cef_log_severity_t::LOGSEVERITY_FATAL,
	};

	let base = Settings {
		windowless_rendering_enabled: 1,
		root_cache_path: instance_dir.to_str().map(CefString::from).unwrap(),
		cache_path: "".into(),
		disable_signal_handlers: 1,
		log_severity: LogSeverity::from(log_severity),
		..Default::default()
	};

	#[cfg(target_os = "macos")]
	{
		let exe = std::env::current_exe().expect("cannot get current exe path");
		let app_root = exe.parent().and_then(|p| p.parent()).expect("bad path structure").parent().expect("bad path structure");
		Settings {
			main_bundle_path: app_root.to_str().map(CefString::from).unwrap(),
			multi_threaded_message_loop: 0,
			external_message_pump: 0,
			no_sandbox: 1, // GPU helper crashes when running with sandbox
			..base
		}
	}

	#[cfg(not(target_os = "macos"))]
	Settings {
		multi_threaded_message_loop: 1,
		#[cfg(target_os = "linux")]
		no_sandbox: 1,
		..base
	}
}

fn create_browser(delegate: BrowserDelegate, frames: FrameStreamer, view_info_sender: Sender<ViewInfoUpdate>, instance_dir: TempDir, accelerated_paint: bool) -> Result<BrowserContext, InitError> {
	#[cfg(not(feature = "accelerated_paint"))]
	let _ = accelerated_paint;
	let mut client = Client::new(BrowserProcessClientImpl::new(&delegate, frames));

	let window_info = WindowInfo {
		windowless_rendering_enabled: 1,
		#[cfg(feature = "accelerated_paint")]
		shared_texture_enabled: accelerated_paint as i32,
		..Default::default()
	};

	let settings = BrowserSettings {
		windowless_frame_rate: WINDOWLESS_FRAME_RATE,
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

	let mut scheme_handler_factory = SchemeHandlerFactory::new(SchemeHandlerFactoryImpl::new(delegate.clone()));
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
	.map(|browser| BrowserContext {
		delegate,
		browser,
		view_info_sender,
		_instance_dir: instance_dir,
	})
	.ok_or_else(|| {
		tracing::error!("Failed to create browser");
		InitError::BrowserCreationFailed
	})
}

#[derive(thiserror::Error, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) enum InitError {
	#[error("Failed to create the instance directory: {0}")]
	InstanceDirectoryCreationFailed(String),
	#[error("Initialization failed with code: {0}")]
	InitializationFailureCode(u32),
	#[error("Browser creation failed")]
	BrowserCreationFailed,
	#[error("Request context creation failed")]
	RequestContextCreationFailed,
}

#[derive(Clone)]
pub(crate) struct CefContextHandle;

impl CefContextHandle {
	pub(crate) fn apply_input(&self, events: Vec<InputEvent>) {
		with_context(move |context| {
			for event in &events {
				input::apply(&context.browser, event);
			}
		});
	}

	pub(crate) fn update_view_info(&self, update: ViewInfoUpdate) {
		with_context(move |context| context.update_view_info(update));
	}

	pub(crate) fn refresh_view_info(&self) {
		with_context(|context| context.refresh_view_info());
	}

	pub(crate) fn send_web_message(&self, message: Vec<u8>) {
		with_context(move |context| context.send_web_message(message));
	}
}

struct BrowserContext {
	delegate: BrowserDelegate,
	browser: Browser,
	view_info_sender: Sender<ViewInfoUpdate>,
	_instance_dir: TempDir,
}

impl BrowserContext {
	fn update_view_info(&self, update: ViewInfoUpdate) {
		let _ = self.view_info_sender.send(update);
	}

	fn refresh_view_info(&self) {
		let view_info = self.delegate.view_info();
		let host = self.browser.host().unwrap();
		host.set_zoom_level(view_info.zoom());
		host.was_resized();

		// Fix for CEF not updating the view after resize
		// TODO: remove once https://github.com/chromiumembedded/cef/issues/3822 is fixed
		host.invalidate(cef::PaintElementType::default());
	}

	fn send_web_message(&self, message: Vec<u8>) {
		self.send_message(MessageType::SendToJS, &message);
	}
}

impl Drop for BrowserContext {
	fn drop(&mut self) {
		tracing::debug!("Shutting down CEF");
		self.browser.host().unwrap().close_browser(1);
	}
}

impl SendMessage for BrowserContext {
	fn send_message(&self, message_type: MessageType, message: &[u8]) {
		let Some(frame) = self.browser.main_frame() else {
			tracing::error!("Main frame is not available, cannot send message");
			return;
		};
		frame.send_message(message_type, message);
	}
}

fn run_on_ui_thread<F>(closure: F)
where
	F: FnOnce() + Send + 'static,
{
	let closure_task = ClosureTask::new(closure);
	let mut task = Task::new(closure_task);
	post_task(ThreadId::from(cef_thread_id_t::TID_UI), Some(&mut task));
}

fn with_context<F>(closure: F)
where
	F: FnOnce(&mut BrowserContext) + Send + 'static,
{
	run_on_ui_thread(move || {
		CONTEXT.with(|b| {
			if let Some(context) = b.borrow_mut().as_mut() {
				closure(context);
			}
		});
	});
}

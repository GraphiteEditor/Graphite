use clap::Parser;
use std::ffi::OsStr;
use std::io::Write;
use std::process::exit;
use tracing_subscriber::EnvFilter;
use winit::event_loop::EventLoop;

pub(crate) mod consts;

mod app;
mod cef;
mod cli;
mod dirs;
mod event;
mod persist;
mod render;
mod window;

mod gpu_context;

pub(crate) use graphite_desktop_wrapper as wrapper;

use app::App;
use cef::CefHandler;
use cli::Cli;
use event::CreateAppEventSchedulerEventLoopExt;

use crate::consts::APP_LOCK_FILE_NAME;

pub fn start() {
	tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env()).init();

	let cef_context_builder = cef::CefContextBuilder::<CefHandler>::new();

	if cef_context_builder.is_sub_process() {
		// We are in a CEF subprocess
		// This will block until the CEF subprocess quits
		let error = cef_context_builder.execute_sub_process();
		tracing::warn!("Cef subprocess failed with error: {error}");
		return;
	}

	let cli = Cli::parse();
	let mut startup_settings = persist::StartupSettings::load_from_disk();
	let compatibility_mode = cli.disable_ui_acceleration || startup_settings.ui_compatibility_mode;

	let Ok(lock_file) = std::fs::OpenOptions::new()
		.read(true)
		.write(true)
		.create(true)
		.truncate(true)
		.open(dirs::app_data_dir().join(APP_LOCK_FILE_NAME))
	else {
		tracing::error!("Failed to open lock file, Exiting.");
		exit(1);
	};
	let mut lock = fd_lock::RwLock::new(lock_file);
	let lock = match lock.try_write() {
		Ok(mut guard) => {
			tracing::info!("Acquired application lock");
			let _ = guard.set_len(0);
			let _ = write!(guard, "{}", std::process::id());
			let _ = guard.sync_all();
			guard
		}
		Err(_) => {
			tracing::error!("Another instance is already running, Exiting.");
			exit(1);
		}
	};

	// Must be called before event loop initialization or native window integrations will break
	App::init();

	let wgpu_context = futures::executor::block_on(gpu_context::create_wgpu_context());

	let event_loop = EventLoop::new().unwrap();
	let (app_event_sender, app_event_receiver) = std::sync::mpsc::channel();
	let app_event_scheduler = event_loop.create_app_event_scheduler(app_event_sender);

	let (cef_view_info_sender, cef_view_info_receiver) = std::sync::mpsc::channel();

	if compatibility_mode {
		println!("UI acceleration is disabled");
	}

	let cef_handler = cef::CefHandler::new(wgpu_context.clone(), app_event_scheduler.clone(), cef_view_info_receiver);
	let cef_context = match cef_context_builder.initialize(cef_handler, compatibility_mode) {
		Ok(context) => {
			tracing::info!("CEF initialized successfully");
			context
		}
		Err(cef::InitError::AlreadyRunning) => {
			tracing::error!("Another instance is already running, Exiting.");
			exit(1);
		}
		Err(cef::InitError::InitializationFailed(code)) => {
			tracing::error!("Cef initialization failed with code: {code}");
			exit(1);
		}
		Err(cef::InitError::BrowserCreationFailed) => {
			tracing::error!("Failed to create CEF browser");
			exit(1);
		}
		Err(cef::InitError::RequestContextCreationFailed) => {
			tracing::error!("Failed to create CEF request context");
			exit(1);
		}
	};

	let app = App::new(
		Box::new(cef_context),
		cef_view_info_sender,
		wgpu_context,
		app_event_receiver,
		app_event_scheduler,
		cli,
		compatibility_mode,
	);

	let exit_reason = app.run(event_loop);

	// Explicitly drop the instance lock
	drop(lock);

	match exit_reason {
		app::ExitReason::UiAccelerationFailure => {
			tracing::error!("Restarting application without UI acceleration");
			startup_settings.ui_compatibility_mode = true;
			startup_settings.save_to_disk();
			if let Err(error) = restart_application(true) {
				tracing::error!("Failed to restart application: {error}");
			}
		}
		app::ExitReason::RelaunchWithUiAcceleration => {
			tracing::info!("Restarting application with UI acceleration");
			startup_settings.ui_compatibility_mode = false;
			startup_settings.save_to_disk();
			if let Err(error) = restart_application(false) {
				tracing::error!("Failed to restart application: {error}");
			}
		}
		app::ExitReason::Shutdown => {}
	}

	// Workaround for a Windows-specific exception that occurs when `app` is dropped.
	// The issue causes the window to hang for a few seconds before closing.
	// Appears to be related to CEF object destruction order.
	// Calling `exit` bypasses rust teardown and lets Windows perform process cleanup.
	// TODO: Identify and fix the underlying CEF shutdown issue so this workaround can be removed.
	#[cfg(target_os = "windows")]
	exit(0);
}

fn restart_application(disable_ui_acceleration: bool) -> std::io::Result<()> {
	let current_exe = std::env::current_exe()?;
	let args = restart_arguments(disable_ui_acceleration);
	std::process::Command::new(current_exe).args(args).spawn().map(|_| ())
}

fn restart_arguments(disable_ui_acceleration: bool) -> Vec<std::ffi::OsString> {
	let disable_flag = OsStr::new("--disable-ui-acceleration");
	let mut args = std::env::args_os().skip(1).filter(|arg| arg != disable_flag).collect::<Vec<_>>();
	if disable_ui_acceleration {
		args.push(disable_flag.into());
	}
	args
}

pub fn start_helper() {
	let cef_context_builder = cef::CefContextBuilder::<CefHandler>::new_helper();
	assert!(cef_context_builder.is_sub_process());
	cef_context_builder.execute_sub_process();
}

use crate::app::App;
use crate::cli::Cli;
use crate::consts::APP_LOCK_FILE_NAME;
use crate::event::{AppEvent, CreateAppEventSchedulerEventLoopExt};
use clap::Parser;
use graphite_desktop_ui::{Acceleration, UiConfig, UiContext, UiEvent, UiSetupResult};
use std::io::Write;
use std::process::ExitCode;
use tracing_subscriber::EnvFilter;
use winit::event_loop::EventLoop;

pub(crate) use graphite_desktop_wrapper as wrapper;

mod app;
mod cli;
mod dirs;
mod event;
mod gpu_context;
mod persist;
mod preferences;
mod render;
mod socket;
mod window;

pub(crate) mod consts;

pub fn start() -> ExitCode {
	tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env()).init();

	let ui_context = match UiContext::setup() {
		UiSetupResult::Ready(context) => context,
		UiSetupResult::Helper(code) => return code,
		UiSetupResult::Failed => {
			eprintln!("Failed to set up the UI runtime");
			return ExitCode::FAILURE;
		}
	};

	let cli = Cli::parse();

	let Ok(lock_file) = std::fs::OpenOptions::new()
		.read(true)
		.write(true)
		.create(true)
		.truncate(true)
		.open(dirs::app_data_dir().join(APP_LOCK_FILE_NAME))
	else {
		panic!("Failed to open lock file.")
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
			if !cli.files.is_empty()
				&& let Err(error) = socket::send(socket::Message::OpenFiles(cli.files))
			{
				tracing::error!("Failed to send socket message to running instance: {}", error);
				return ExitCode::FAILURE;
			}
			return ExitCode::SUCCESS;
		}
	};

	dirs::clear_dir(&graphite_desktop_ui::temp_dir_root());

	// TODO: Eventually remove this cleanup code for the old "browser" CEF directory
	dirs::delete_old_cef_browser_directory();

	let mut prefs = preferences::read();

	// Must be called before event loop initialization or native window integrations will break
	App::init();

	let wgpu_context = futures::executor::block_on(gpu_context::create_wgpu_context());

	let event_loop = EventLoop::new().unwrap();
	let (app_event_sender, app_event_receiver) = std::sync::mpsc::channel();
	let app_event_scheduler = event_loop.create_app_event_scheduler(app_event_sender);

	let _socket_handle = socket::start(app_event_scheduler.clone());

	if cli.disable_ui_acceleration {
		prefs.disable_ui_acceleration = true;
	}
	if prefs.disable_ui_acceleration {
		println!("UI acceleration is disabled");
	}

	let acceleration = if prefs.disable_ui_acceleration { Acceleration::Disabled } else { Acceleration::Auto };
	let ui_context = match ui_context.start(UiConfig { acceleration }) {
		Ok(context) => context,
		Err(error) => {
			tracing::error!("Failed to start the UI runtime: {error}");
			return ExitCode::FAILURE;
		}
	};
	let ui = match ui_context.instance(&wgpu_context.device, &wgpu_context.queue) {
		Ok(ui) => ui,
		Err(error) => {
			tracing::error!("Failed to start the UI: {error}");
			return ExitCode::FAILURE;
		}
	};
	tracing::info!("UI runtime started successfully");

	{
		let ui = ui.clone();
		let scheduler = app_event_scheduler.clone();
		let spawned = std::thread::Builder::new().name("ui-events".to_string()).spawn(move || {
			while let Some(event) = ui.recv() {
				match event {
					UiEvent::Ready => scheduler.schedule(AppEvent::WebCommunicationInitialized),
					UiEvent::Frame(texture) => scheduler.schedule(AppEvent::UiUpdate(texture)),
					UiEvent::Cursor(cursor) => scheduler.schedule(AppEvent::CursorChange(cursor)),
					UiEvent::Message(message) => match wrapper::deserialize_editor_message(&message) {
						Some(message) => scheduler.schedule(AppEvent::DesktopWrapperMessage(message)),
						None => tracing::error!("Failed to deserialize web message"),
					},
					UiEvent::InitFailed(error) => {
						tracing::error!("UI initialization failed: {error}");
						scheduler.schedule(AppEvent::UiCrashed);
					}
					UiEvent::Crashed => scheduler.schedule(AppEvent::UiCrashed),
				}
			}
		});
		if let Err(error) = spawned {
			tracing::error!("Failed to spawn the UI event bridge thread: {error}");
			return ExitCode::FAILURE;
		}
	}

	let app = App::new(ui.clone(), wgpu_context, app_event_receiver, app_event_scheduler, prefs, cli.files);

	let exit_reason = app.run(event_loop);

	// ui needs to be shutdown before restarting
	ui.shutdown();

	// If exiting due to a UI acceleration failure, update preferences to disable it for next launch
	if matches!(exit_reason, app::ExitReason::UiAccelerationFailure) {
		tracing::error!("Disabling UI acceleration");
		preferences::modify(|prefs| {
			prefs.disable_ui_acceleration = true;
		});
	}

	// Explicitly drop the instance lock
	drop(lock);

	match exit_reason {
		app::ExitReason::Restart | app::ExitReason::UiAccelerationFailure => {
			tracing::info!("Restarting application");
			let mut command = std::process::Command::new(std::env::current_exe().unwrap());
			#[cfg(target_family = "unix")]
			let _ = std::os::unix::process::CommandExt::exec(&mut command);
			#[cfg(target_family = "unix")]
			tracing::error!("Failed to restart application");
			#[cfg(not(target_family = "unix"))]
			let _ = command.spawn();
		}
		_ => {}
	}

	#[cfg(not(target_os = "windows"))]
	ExitCode::SUCCESS
}

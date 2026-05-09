use interprocess::local_socket::{GenericFilePath, GenericNamespaced, ListenerNonblockingMode, ListenerOptions, Name, prelude::*};
use std::io::{ErrorKind, Read, Write};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::consts::APP_SOCKET_FILE_NAME;
use crate::event::{AppEvent, AppEventScheduler};

// TODO: Needs to be integrated/replaced with the action system.
// TODO: At that point this should just wrap the action, meaning all actions bindable by the user can also be accessed via the socket.
#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) enum Message {
	OpenFiles(Vec<std::path::PathBuf>),
}

fn handle_message(message: Message, app_event_scheduler: &AppEventScheduler) {
	match message {
		Message::OpenFiles(paths) => {
			app_event_scheduler.schedule(AppEvent::OpenFiles(paths));
		}
	}
}

pub(crate) fn send(message: Message) -> std::io::Result<()> {
	let data = ron::ser::to_string(&message).map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
	let mut connection = interprocess::local_socket::Stream::connect(socket_name())?;
	connection.write_all(data.as_bytes())
}

pub(crate) struct SocketHandle {
	thread: Option<thread::JoinHandle<()>>,
	shutdown_sender: mpsc::Sender<()>,
}
impl Drop for SocketHandle {
	fn drop(&mut self) {
		let _ = self.shutdown_sender.send(());
		let _ = self.thread.take().expect("SocketHandle can only be dropped once").join();
	}
}

pub(crate) fn start(app_event_scheduler: AppEventScheduler) -> SocketHandle {
	let (shutdown_sender, shutdown_receiver) = mpsc::channel();

	let thread = thread::Builder::new()
		.name("socket".to_string())
		.spawn(move || run(app_event_scheduler, shutdown_receiver))
		.expect("Failed to spawn socket thread");

	SocketHandle {
		shutdown_sender,
		thread: Some(thread),
	}
}

fn run(app_event_scheduler: AppEventScheduler, shutdown_receiver: mpsc::Receiver<()>) {
	let listener = match ListenerOptions::new()
		.name(socket_name())
		.nonblocking(ListenerNonblockingMode::Accept)
		.try_overwrite(true)
		.max_spin_time(Duration::from_millis(100))
		.create_sync()
	{
		Ok(listener) => listener,
		Err(error) => {
			tracing::error!("Failed to bind socket: {}", error);
			return;
		}
	};

	let max_backoff = Duration::from_millis(100);
	let mut backoff = Duration::ZERO;

	loop {
		if backoff.is_zero() {
			match shutdown_receiver.try_recv() {
				Ok(()) | Err(mpsc::TryRecvError::Disconnected) => break,
				Err(mpsc::TryRecvError::Empty) => {}
			}
			backoff = Duration::from_nanos(1);
		} else {
			match shutdown_receiver.recv_timeout(backoff) {
				Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
				Err(mpsc::RecvTimeoutError::Timeout) => {}
			}
			backoff = (backoff * 2).min(max_backoff);
		}

		match listener.accept() {
			Ok(mut connection) => {
				backoff = Duration::ZERO;

				let app_event_scheduler = app_event_scheduler.clone();
				let spawn_result = thread::Builder::new().name("socket-connection".to_string()).spawn(move || {
					let mut data = String::new();
					if let Err(error) = connection.read_to_string(&mut data) {
						tracing::error!("Failed to read socket message: {}", error);
						return;
					}

					match ron::de::from_str(&data) {
						Ok(message) => handle_message(message, &app_event_scheduler),
						Err(error) => tracing::error!("Failed to deserialize socket message: {}", error),
					}
				});
				if let Err(error) = spawn_result {
					tracing::error!("Failed to spawn socket connection thread: {}", error);
				}
			}
			Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted) => {}
			Err(error) => {
				tracing::error!("Failed to accept socket connection: {}", error);
			}
		}
	}
}

fn socket_name() -> Name<'static> {
	if cfg!(target_os = "windows") {
		let user = std::env::var("USERNAME").unwrap_or_default();
		let name = format!("{user}-{app}-{APP_SOCKET_FILE_NAME}", app = crate::consts::APP_NAME);
		name.to_ns_name::<GenericNamespaced>().expect("valid named pipe name")
	} else {
		crate::dirs::app_data_dir().join(APP_SOCKET_FILE_NAME).to_fs_name::<GenericFilePath>().expect("valid socket path")
	}
}

//! Starting, reaching, and stopping the session host, which owns the browser so separate invocations can act on the same editor one after another.

use graphite_editor_control_protocol::{ClientMessage, Command, CommandResult, HostMessage, PROTOCOL_VERSION};
use interprocess::local_socket::{RecvHalf, SendHalf, Stream, prelude::*};
use serde::{Deserialize, Serialize};
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::paths;
use crate::processes::{POLL_INTERVAL, PROBE_TIMEOUT, read_log, read_record, spawn_detached, terminate};
use crate::session::SessionOptions;
use crate::socket;

const SESSION_START_TIMEOUT: Duration = Duration::from_secs(300);
const SESSION_STOP_TIMEOUT: Duration = Duration::from_secs(10);
const HOST_COPY_ATTEMPTS: u32 = 8;

/// Where the session host can be reached, and the secret every connection to it must open with.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRecord {
	pub process_id: u32,
	/// Names the host's socket, unique to this session so a stale record cannot reach another one.
	pub session_id: String,
	pub url: String,
	pub token: String,
}

enum OpenFailure {
	/// Nothing answered as the recorded session.
	Unreachable,
	/// The session answered but turned the connection away for the given reason.
	Refused(String),
}

pub struct Connection {
	receiver: BufReader<RecvHalf>,
	sender: SendHalf,
	next_id: u64,
}

impl Connection {
	fn open(record: &SessionRecord) -> Result<Self, OpenFailure> {
		let name = socket::name(&record.session_id).map_err(|_| OpenFailure::Unreachable)?;
		let stream = Stream::connect(name).map_err(|_| OpenFailure::Unreachable)?;
		let (receiver, sender) = stream.split();
		let mut connection = Self {
			receiver: BufReader::new(receiver),
			sender,
			next_id: 1,
		};

		let attach = ClientMessage::Attach {
			version: PROTOCOL_VERSION,
			token: record.token.clone(),
		};
		connection.send(&attach).map_err(|_| OpenFailure::Unreachable)?;
		match connection.receive() {
			Ok(Some(HostMessage::Attached { process_id, .. })) if process_id == record.process_id => Ok(connection),
			Ok(Some(HostMessage::Refused { reason })) => Err(OpenFailure::Refused(reason)),
			_ => Err(OpenFailure::Unreachable),
		}
	}

	pub fn perform(&mut self, commands: Vec<Command>, fail_on_console_errors: bool) -> Result<Vec<CommandResult>, String> {
		let id = self.next_id;
		self.next_id += 1;
		self.send(&ClientMessage::Perform { id, commands, fail_on_console_errors })?;

		match self.receive()? {
			Some(HostMessage::Results { id: answered, results }) if answered == id => Ok(results),
			Some(_) => Err("The session sent an unexpected reply".to_string()),
			None => Err(format!(
				"The session closed the connection before answering, and its log may say why: {}",
				paths::session_log_file().display()
			)),
		}
	}

	// Asks the session to end, returning whether it exited in time, which its end of the connection closing shows
	fn shut_down(mut self) -> bool {
		if self.send(&ClientMessage::Shutdown).is_err() {
			return false;
		}

		let (sender, receiver) = mpsc::channel();
		std::thread::spawn(move || {
			while let Ok(Some(_)) = self.receive() {}
			let _ = sender.send(());
		});
		receiver.recv_timeout(SESSION_STOP_TIMEOUT).is_ok()
	}

	fn send(&mut self, message: &ClientMessage) -> Result<(), String> {
		socket::write_message(&mut self.sender, message)
	}

	fn receive(&mut self) -> Result<Option<HostMessage>, String> {
		socket::read_message(&mut self.receiver)
	}
}

pub enum SessionState {
	NotRunning,
	Running(SessionRecord, Connection),
	/// A session answered but turned the connection away, such as one started by a build that speaks another protocol version.
	Refused(SessionRecord, String),
	/// A session's socket exists but the session did not answer in time.
	Unresponsive(SessionRecord),
}

/// Finds the running session, removing a stale record left by one that is gone.
pub fn session_state() -> SessionState {
	let Some(record) = read_record::<SessionRecord>(&paths::session_file()) else {
		return SessionState::NotRunning;
	};

	// A session that has stopped answering cannot hold up every later invocation
	let (sender, receiver) = mpsc::channel();
	let probe_record = record.clone();
	std::thread::spawn(move || sender.send(Connection::open(&probe_record)));

	match receiver.recv_timeout(PROBE_TIMEOUT) {
		Ok(Ok(connection)) => SessionState::Running(record, connection),
		Ok(Err(OpenFailure::Refused(reason))) => SessionState::Refused(record, reason),
		Ok(Err(OpenFailure::Unreachable)) => {
			let _ = std::fs::remove_file(paths::session_file());
			SessionState::NotRunning
		}
		Err(_) => SessionState::Unresponsive(record),
	}
}

pub fn perform(commands: Vec<Command>, fail_on_console_errors: bool) -> Result<Vec<CommandResult>, String> {
	match session_state() {
		SessionState::Running(_, mut connection) => connection.perform(commands, fail_on_console_errors),
		SessionState::Refused(_, reason) => Err(reason),
		SessionState::Unresponsive(_) => Err("The session is not answering, so stop it and start a new one".to_string()),
		SessionState::NotRunning => Err("No session is running, so start one first".to_string()),
	}
}

pub fn start_session(options: &SessionOptions) -> Result<(), String> {
	let _lock = StartLock::take()?;
	if !matches!(session_state(), SessionState::NotRunning) {
		return Err("A session is already running, so stop it first".to_string());
	}

	let mut arguments = vec![
		"host".to_string(),
		"--url".into(),
		options.url.clone(),
		"--width".into(),
		options.size.x.to_string(),
		"--height".into(),
		options.size.y.to_string(),
		"--scale".into(),
		options.scale.to_string(),
	];
	if options.headed {
		arguments.push("--headed".into());
	}
	let mut process = spawn_detached(&host_executable()?, &arguments, &paths::tool_directory(), &paths::session_log_file())?;

	let deadline = Instant::now() + SESSION_START_TIMEOUT;
	while Instant::now() < deadline {
		if read_record::<SessionRecord>(&paths::session_file()).is_some_and(|record| record.process_id == process.id) {
			return Ok(());
		}
		if !process.is_running() {
			return Err(format!("The session failed to start:\n{}", read_log(&paths::session_log_file())));
		}

		std::thread::sleep(POLL_INTERVAL);
	}

	terminate(process.id);
	Err(format!("The editor did not finish loading in time:\n{}", read_log(&paths::session_log_file())))
}

pub fn stop_session() {
	match session_state() {
		SessionState::Running(record, connection) => {
			// The process answered as itself just above, so ending it by its ID is safe if it does not exit in time
			if !connection.shut_down() {
				terminate(record.process_id);
			}
		}
		// Only the session's own process listens on the socket named for it, so ending that process is safe
		SessionState::Refused(record, _) | SessionState::Unresponsive(record) => terminate(record.process_id),
		SessionState::NotRunning => {}
	}

	let _ = std::fs::remove_file(paths::session_file());
}

// On Windows a running program's file cannot be replaced, so the session runs from a copy to leave the build free to update the original
fn host_executable() -> Result<PathBuf, String> {
	let current = std::env::current_exe().map_err(|error| format!("Failed to find this program's file: {error}"))?;
	if !cfg!(windows) {
		return Ok(current);
	}

	let copy = paths::session_directory().join(format!("host{}", std::env::consts::EXE_SUFFIX));
	paths::create_session_directory()?;

	// The previous session's copy can stay locked for a moment after that session exits
	for _ in 0..HOST_COPY_ATTEMPTS {
		if std::fs::copy(&current, &copy).is_ok() {
			return Ok(copy);
		}
		std::thread::sleep(POLL_INTERVAL);
	}

	eprintln!("Failed to copy this program for the session to run from, so rebuilding it will fail until the session stops");
	Ok(current)
}

// A lock file held while starting keeps two `start` commands from both launching a session
struct StartLock;

impl StartLock {
	fn take() -> Result<Self, String> {
		let path = paths::session_lock_file();

		let age = std::fs::metadata(&path).and_then(|metadata| metadata.modified()).ok().and_then(|modified| modified.elapsed().ok());
		if age.is_some_and(|age| age > SESSION_START_TIMEOUT) {
			let _ = std::fs::remove_file(&path);
		}

		paths::create_session_directory()?;
		match std::fs::File::create_new(&path) {
			Ok(_) => Ok(Self),
			Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Err("Another `start` is already in progress".to_string()),
			Err(error) => Err(format!("Failed to create {}: {error}", path.display())),
		}
	}
}

impl Drop for StartLock {
	fn drop(&mut self) {
		let _ = std::fs::remove_file(paths::session_lock_file());
	}
}

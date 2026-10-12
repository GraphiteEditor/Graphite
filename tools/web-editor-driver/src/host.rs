//! The long-lived process that owns the browser, which `start` runs in the background and every later invocation connects to.

use graphite_editor_control_protocol::{ClientMessage, Command, CommandResult, HostMessage, PROTOCOL_VERSION};
use interprocess::local_socket::{ListenerOptions, Stream, prelude::*};
use std::io::BufReader;
use std::sync::{Arc, Mutex, PoisonError};

use crate::browser::Browser;
use crate::client::SessionRecord;
use crate::paths;
use crate::processes::write_record;
use crate::session::{EditorSession, SessionOptions};
use crate::socket;

const TOKEN_BYTES: usize = 32;
const SESSION_ID_BYTES: usize = 8;

struct HostState {
	// Commands are performed one batch at a time so the commands of overlapping invocations cannot interleave
	session: Mutex<EditorSession>,
	browser: Arc<Browser>,
	token: String,
}

fn random_hex(byte_count: usize) -> Result<String, String> {
	let mut bytes = vec![0; byte_count];
	getrandom::fill(&mut bytes).map_err(|error| format!("Failed to generate random bytes: {error}"))?;
	Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub fn run(options: SessionOptions) -> Result<(), String> {
	// Only someone who can read the record file knows the token, which keeps other programs from driving the session
	let token = random_hex(TOKEN_BYTES)?;
	let session_id = random_hex(SESSION_ID_BYTES)?;

	let browser = Arc::new(Browser::start()?);
	let session = EditorSession::launch(browser.clone(), options.clone())?;

	let listener = ListenerOptions::new()
		.name(socket::name(&session_id)?)
		.try_overwrite(true)
		.create_sync()
		.map_err(|error| format!("Failed to listen on the session's socket: {error}"))?;

	let record = SessionRecord {
		process_id: std::process::id(),
		session_id,
		url: options.url,
		token: token.clone(),
	};
	write_record(&paths::session_file(), &record)?;

	let state = Arc::new(HostState {
		session: Mutex::new(session),
		browser,
		token,
	});
	for connection in listener.incoming() {
		match connection {
			Ok(stream) => {
				let state = state.clone();
				std::thread::spawn(move || serve(stream, &state));
			}
			Err(error) => eprintln!("Failed to accept a connection: {error}"),
		}
	}

	Ok(())
}

fn serve(stream: Stream, state: &HostState) {
	let (receiver, mut sender) = stream.split();
	let mut receiver = BufReader::new(receiver);

	let refusal = match socket::read_message(&mut receiver) {
		Ok(Some(ClientMessage::Attach { version, .. })) if version != PROTOCOL_VERSION => Some(format!(
			"The session speaks version {PROTOCOL_VERSION} of the protocol rather than version {version}, so stop it and start a new one"
		)),
		Ok(Some(ClientMessage::Attach { token, .. })) if token != state.token => Some("The token does not match the session's".to_string()),
		Ok(Some(ClientMessage::Attach { .. })) => None,
		_ => Some("A connection must begin by attaching".to_string()),
	};
	if let Some(reason) = refusal {
		let _ = socket::write_message(&mut sender, &HostMessage::Refused { reason });
		return;
	}

	let attached = HostMessage::Attached {
		version: PROTOCOL_VERSION,
		process_id: std::process::id(),
	};
	if socket::write_message(&mut sender, &attached).is_err() {
		return;
	}

	loop {
		let message = match socket::read_message(&mut receiver) {
			Ok(Some(message)) => message,
			Ok(None) => return,
			Err(error) => {
				eprintln!("{error}");
				return;
			}
		};

		let reply = match message {
			ClientMessage::Perform { id, commands, fail_on_console_errors } => {
				let mut session = state.session.lock().unwrap_or_else(PoisonError::into_inner);
				HostMessage::Results {
					id,
					results: perform_all(&mut session, &commands, fail_on_console_errors),
				}
			}
			ClientMessage::Shutdown => {
				let _ = socket::write_message(&mut sender, &HostMessage::ShuttingDown);
				shut_down(state);
			}
			ClientMessage::Attach { .. } => {
				let reason = "The connection is already attached".to_string();
				let _ = socket::write_message(&mut sender, &HostMessage::Refused { reason });
				return;
			}
		};
		if socket::write_message(&mut sender, &reply).is_err() {
			return;
		}
	}
}

fn perform_all(session: &mut EditorSession, commands: &[Command], fail_on_console_errors: bool) -> Vec<CommandResult> {
	let mut results = Vec::new();
	for command in commands {
		let result = session.perform(command, fail_on_console_errors);
		let failed = !result.ok;
		results.push(result);

		// The remaining commands are skipped after a failure, since they were written assuming the earlier ones succeeded
		if failed {
			break;
		}
	}
	results
}

// Exits even if closing the browser fails, so a shutdown always ends the process, which closes the connection that asked for it
fn shut_down(state: &HostState) -> ! {
	let _ = std::fs::remove_file(paths::session_file());
	state.browser.close();
	std::process::exit(0);
}

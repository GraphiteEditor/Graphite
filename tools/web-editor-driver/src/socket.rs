use interprocess::local_socket::{GenericFilePath, GenericNamespaced, Name, prelude::*};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::io::{BufRead, Read, Write};

// Room for any batch of commands or results, while a connection that never ends its line cannot make the reader buffer without end
const MAX_MESSAGE_LENGTH: u64 = 16 * 1024 * 1024;

/// The name of the local socket a session's host listens on, which is unique to the session.
pub fn name(session_id: &str) -> Result<Name<'static>, String> {
	let file_name = format!("graphite-web-editor-driver-{session_id}.sock");
	let name = if GenericNamespaced::is_supported() {
		file_name.to_ns_name::<GenericNamespaced>()
	} else {
		std::env::temp_dir().join(file_name).to_fs_name::<GenericFilePath>()
	};
	name.map_err(|error| format!("Failed to name the session's socket: {error}"))
}

/// Sends a message as one line of JSON.
pub fn write_message(writer: &mut impl Write, message: &impl Serialize) -> Result<(), String> {
	let mut line = serde_json::to_string(message).map_err(|error| format!("Failed to encode a message: {error}"))?;
	line.push('\n');
	writer
		.write_all(line.as_bytes())
		.and_then(|()| writer.flush())
		.map_err(|error| format!("Failed to send a message: {error}"))
}

/// Receives a message sent as one line of JSON, or `None` once the other end has closed the connection.
pub fn read_message<T: DeserializeOwned>(reader: &mut impl BufRead) -> Result<Option<T>, String> {
	let mut line = String::new();
	let length = reader
		.by_ref()
		.take(MAX_MESSAGE_LENGTH)
		.read_line(&mut line)
		.map_err(|error| format!("Failed to receive a message: {error}"))?;
	if length == 0 {
		return Ok(None);
	}
	if !line.ends_with('\n') && length as u64 == MAX_MESSAGE_LENGTH {
		return Err("Received a message longer than 16 MiB".to_string());
	}

	serde_json::from_str(&line).map(Some).map_err(|error| format!("Received a message that could not be read: {error}"))
}

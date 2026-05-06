//! Single-instance file-open handoff for Windows and Linux.
//!
//! When the user double-clicks a `.graphite` file (or drags it onto the executable) while a
//! Graphite instance is already running, the OS spawns a new process. The new process fails to
//! acquire the application lock, then forwards its file paths to the running instance over a
//! local IPC channel and exits. The running instance opens those files in place.
//!
//! Mac handles the same scenario natively via `NSApplicationDelegate::application:openURLs:`
//! and so this module is unused there.

use std::ffi::OsString;
#[cfg(windows)]
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use interprocess::local_socket::traits::{ListenerExt, Stream as StreamTrait};
#[cfg(unix)]
use interprocess::local_socket::{GenericFilePath, ToFsName};
#[cfg(windows)]
use interprocess::local_socket::{GenericNamespaced, ToNsName};
use interprocess::local_socket::{ListenerOptions, Name, Stream};

use crate::dirs;
use crate::event::{AppEvent, AppEventScheduler};

const MAX_PATH_COUNT: u32 = 1024;
const MAX_PATH_BYTES: u32 = 32 * 1024;
const CONNECT_RETRY_ATTEMPTS: u32 = 30;
const CONNECT_RETRY_INTERVAL: Duration = Duration::from_millis(100);

#[cfg(windows)]
fn endpoint_name() -> io::Result<Name<'static>> {
	// Named pipes share a global namespace per machine, so derive a per-user identifier from the user's app data directory (which is itself per-user).
	let mut hasher = DefaultHasher::new();
	dirs::app_data_dir().hash(&mut hasher);
	let pipe_name = format!("graphite-instance-{:016x}", hasher.finish());
	pipe_name.to_ns_name::<GenericNamespaced>().map(|name| name.into_owned())
}

#[cfg(unix)]
fn endpoint_path() -> PathBuf {
	dirs::app_data_dir().join("instance.sock")
}

#[cfg(unix)]
fn endpoint_name() -> io::Result<Name<'static>> {
	endpoint_path().to_fs_name::<GenericFilePath>().map(|name| name.into_owned())
}

/// Bind the IPC endpoint and spawn a listener thread that forwards received paths to the live
/// instance via [`AppEvent::AddLaunchDocuments`]. Called once after the application lock is acquired.
pub(crate) fn start_listener(scheduler: AppEventScheduler) {
	#[cfg(unix)]
	{
		// A stale socket file may remain after a previous unclean exit. Removing it before bind
		// is safe because we hold the application lock, so no other instance can be listening.
		let _ = std::fs::remove_file(endpoint_path());
	}

	let name = match endpoint_name() {
		Ok(name) => name,
		Err(error) => {
			tracing::error!("Failed to construct instance IPC endpoint name: {error}");
			return;
		}
	};

	let listener = match ListenerOptions::new().name(name).create_sync() {
		Ok(listener) => listener,
		Err(error) => {
			tracing::error!("Failed to bind instance IPC listener: {error}");
			return;
		}
	};

	let _ = thread::Builder::new().name("graphite-instance-ipc".into()).spawn(move || {
		for connection in listener.incoming() {
			match connection {
				Ok(mut stream) => match read_paths(&mut stream) {
					Ok(paths) if !paths.is_empty() => {
						tracing::info!("Received {} file path(s) from secondary instance", paths.len());
						scheduler.schedule(AppEvent::AddLaunchDocuments(paths));
					}
					Ok(_) => {}
					Err(error) => tracing::error!("Failed to read paths from secondary instance: {error}"),
				},
				Err(error) => tracing::error!("Instance IPC accept failed: {error}"),
			}
		}
	});
}

/// Connect to the live instance's IPC endpoint and send `paths` to it. Retries briefly to cover
/// the brief timeframe during which the live instance has acquired the lock but has not yet bound
/// its listener. Returns `Ok(())` only if the live instance acknowledged the write.
pub(crate) fn try_send_paths(paths: &[PathBuf]) -> io::Result<()> {
	let mut last_error: Option<io::Error> = None;
	for _ in 0..CONNECT_RETRY_ATTEMPTS {
		let name = endpoint_name()?;
		match Stream::connect(name) {
			Ok(mut stream) => {
				write_paths(&mut stream, paths)?;
				return Ok(());
			}
			Err(error) => {
				last_error = Some(error);
				thread::sleep(CONNECT_RETRY_INTERVAL);
			}
		}
	}
	Err(last_error.unwrap_or_else(|| io::Error::other("Failed to connect to instance IPC endpoint")))
}

/// Best-effort removal of the Unix socket file on shutdown. No-op on Windows since the named pipe is reclaimed when the process exits.
pub(crate) fn cleanup() {
	#[cfg(unix)]
	{
		let _ = std::fs::remove_file(endpoint_path());
	}
}

fn read_paths(stream: &mut Stream) -> io::Result<Vec<PathBuf>> {
	let count = read_u32(stream)?;
	if count > MAX_PATH_COUNT {
		return Err(io::Error::new(io::ErrorKind::InvalidData, "Too many paths in IPC payload"));
	}

	let mut paths = Vec::with_capacity(count as usize);
	for _ in 0..count {
		let length = read_u32(stream)?;
		if length > MAX_PATH_BYTES {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "IPC path exceeds maximum length"));
		}

		let mut buffer = vec![0_u8; length as usize];
		stream.read_exact(&mut buffer)?;

		// Safety: bytes were produced by `OsStr::as_encoded_bytes` on the same OS,
		// which is the documented round-trip contract for `from_encoded_bytes_unchecked`.
		let os_string = unsafe { OsString::from_encoded_bytes_unchecked(buffer) };
		paths.push(PathBuf::from(os_string));
	}
	Ok(paths)
}

fn write_paths(stream: &mut Stream, paths: &[PathBuf]) -> io::Result<()> {
	let count = u32::try_from(paths.len()).map_err(|_| io::Error::other("Too many paths"))?;
	stream.write_all(&count.to_le_bytes())?;

	for path in paths {
		let bytes = path.as_os_str().as_encoded_bytes();
		let length = u32::try_from(bytes.len()).map_err(|_| io::Error::other("Path too long"))?;
		stream.write_all(&length.to_le_bytes())?;
		stream.write_all(bytes)?;
	}

	stream.flush()
}

fn read_u32(stream: &mut Stream) -> io::Result<u32> {
	let mut buffer = [0_u8; 4];
	stream.read_exact(&mut buffer)?;
	Ok(u32::from_le_bytes(buffer))
}

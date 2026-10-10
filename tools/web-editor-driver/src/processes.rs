//! Starting and stopping background processes, and the records they leave so later invocations can find them.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(unix)]
use std::process::Stdio;
use std::time::{Duration, Instant};

use crate::paths;

pub const POLL_INTERVAL: Duration = Duration::from_millis(250);
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const DEV_SERVER_START_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevServerRecord {
	pub port: u16,
	pub process_id: u32,
}

pub fn read_record<T: DeserializeOwned>(path: &Path) -> Option<T> {
	serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

// Writes beside the file, then renames it into place, so a reader never sees it half written
pub fn write_record(path: &Path, record: &impl Serialize) -> Result<(), String> {
	let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
	let contents = serde_json::to_vec(record).map_err(|error| error.to_string())?;

	if let Some(parent) = path.parent() {
		std::fs::create_dir_all(parent).map_err(|error| format!("Failed to create the folder {}: {error}", parent.display()))?;
	}
	std::fs::write(&temporary, contents).map_err(|error| format!("Failed to write {}: {error}", temporary.display()))?;
	std::fs::rename(&temporary, path).map_err(|error| format!("Failed to write {}: {error}", path.display()))
}

pub fn read_log(path: &Path) -> String {
	std::fs::read_to_string(path).unwrap_or_default()
}

/// A process started detached from this one, which keeps running after this one exits.
pub struct DetachedProcess {
	pub id: u32,
	#[cfg(windows)]
	handle: std::os::windows::io::OwnedHandle,
	#[cfg(unix)]
	child: std::process::Child,
}

impl DetachedProcess {
	#[cfg(windows)]
	pub fn is_running(&mut self) -> bool {
		use std::os::windows::io::AsRawHandle;
		use windows_sys::Win32::Foundation::WAIT_TIMEOUT;
		use windows_sys::Win32::System::Threading::WaitForSingleObject;

		// SAFETY: The handle is owned by this struct, so it stays open for the call
		unsafe { WaitForSingleObject(self.handle.as_raw_handle(), 0) == WAIT_TIMEOUT }
	}

	#[cfg(unix)]
	pub fn is_running(&mut self) -> bool {
		matches!(self.child.try_wait(), Ok(None))
	}
}

// Runs a program detached from this process, with its output written to a fresh log file
pub fn spawn_detached(program: &Path, arguments: &[String], working_directory: &Path, log_path: &Path) -> Result<DetachedProcess, String> {
	paths::create_session_directory()?;
	let log = File::create(log_path).map_err(|error| format!("Failed to create {}: {error}", log_path.display()))?;

	spawn_with_log(program, arguments, working_directory, log).map_err(|error| format!("Failed to start {}: {error}", program.display()))
}

// In a process group of its own, so Ctrl+C in the terminal does not reach it
#[cfg(unix)]
fn spawn_with_log(program: &Path, arguments: &[String], working_directory: &Path, log: File) -> std::io::Result<DetachedProcess> {
	use std::os::unix::process::CommandExt;

	let child = Command::new(program)
		.args(arguments)
		.current_dir(working_directory)
		.stdin(Stdio::null())
		.stdout(log.try_clone()?)
		.stderr(log)
		.process_group(0)
		.spawn()?;
	Ok(DetachedProcess { id: child.id(), child })
}

// Passes only the handles it needs, since Windows otherwise hands over stray copies of the terminal's output that would keep a caller reading it waiting.
// It also gets a hidden console that its children share instead of opening windows, and a process group out of reach of Ctrl+C in the terminal.
#[cfg(windows)]
fn spawn_with_log(program: &Path, arguments: &[String], working_directory: &Path, log: File) -> std::io::Result<DetachedProcess> {
	use std::os::windows::ffi::OsStrExt;
	use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
	use windows_sys::Win32::Foundation::{CloseHandle, HANDLE_FLAG_INHERIT, SetHandleInformation};
	use windows_sys::Win32::System::Threading::{
		CREATE_NEW_PROCESS_GROUP, CREATE_NO_WINDOW, CreateProcessW, DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT, InitializeProcThreadAttributeList, LPPROC_THREAD_ATTRIBUTE_LIST,
		PROC_THREAD_ATTRIBUTE_HANDLE_LIST, PROCESS_INFORMATION, STARTF_USESTDHANDLES, STARTUPINFOEXW, STARTUPINFOW, UpdateProcThreadAttribute,
	};

	let input = File::open("NUL")?;
	let handles = [input.as_raw_handle(), log.as_raw_handle()];
	let mut command_line = windows_command_line(program, arguments);
	let directory = working_directory.as_os_str().encode_wide().chain([0]).collect::<Vec<u16>>();

	// SAFETY: Every pointer passed points into a local that outlives the call, and the attribute list is deleted before its buffer is freed
	unsafe {
		for handle in handles {
			if SetHandleInformation(handle, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) == 0 {
				return Err(std::io::Error::last_os_error());
			}
		}

		// The first call only reports the size the list needs, which is allocated as pointer-sized words to keep it aligned
		let mut list_size = 0;
		InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut list_size);
		let mut list_buffer = vec![0_usize; list_size.div_ceil(size_of::<usize>())];
		let list = list_buffer.as_mut_ptr() as LPPROC_THREAD_ATTRIBUTE_LIST;
		if InitializeProcThreadAttributeList(list, 1, 0, &mut list_size) == 0 {
			return Err(std::io::Error::last_os_error());
		}

		let attribute = PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize;
		let created = if UpdateProcThreadAttribute(list, 0, attribute, handles.as_ptr().cast(), size_of_val(&handles), std::ptr::null_mut(), std::ptr::null()) == 0 {
			Err(std::io::Error::last_os_error())
		} else {
			let mut startup_information = STARTUPINFOEXW::default();
			startup_information.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
			startup_information.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
			startup_information.StartupInfo.hStdInput = handles[0];
			startup_information.StartupInfo.hStdOutput = handles[1];
			startup_information.StartupInfo.hStdError = handles[1];
			startup_information.lpAttributeList = list;

			let mut process_information = PROCESS_INFORMATION::default();
			let flags = EXTENDED_STARTUPINFO_PRESENT | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW;
			// The extended structure is read whole, so the pointer covers all of it
			let startup = (&raw const startup_information).cast::<STARTUPINFOW>();
			let (application, environment, security) = (std::ptr::null(), std::ptr::null(), std::ptr::null());
			if CreateProcessW(
				application,
				command_line.as_mut_ptr(),
				security,
				security,
				1,
				flags,
				environment,
				directory.as_ptr(),
				startup,
				&mut process_information,
			) == 0
			{
				Err(std::io::Error::last_os_error())
			} else {
				CloseHandle(process_information.hThread);
				Ok(DetachedProcess {
					id: process_information.dwProcessId,
					handle: OwnedHandle::from_raw_handle(process_information.hProcess),
				})
			}
		};

		DeleteProcThreadAttributeList(list);
		created
	}
}

// Quotes each argument the way the Microsoft C runtime splits a command line, which Rust and Node.js programs both follow
#[cfg(windows)]
fn windows_command_line(program: &Path, arguments: &[String]) -> Vec<u16> {
	use std::os::windows::ffi::OsStrExt;

	let mut line = Vec::new();
	let quote = u16::from(b'"');
	let backslash = u16::from(b'\\');

	for (index, argument) in std::iter::once(program.as_os_str()).chain(arguments.iter().map(std::ffi::OsStr::new)).enumerate() {
		if index > 0 {
			line.push(u16::from(b' '));
		}
		line.push(quote);

		// Backslashes are literal unless they come before a quote, where each must be doubled, including before the closing quote
		let mut backslashes = 0;
		for character in argument.encode_wide() {
			if character == backslash {
				backslashes += 1;
				continue;
			}

			let escaped_backslashes = if character == quote { backslashes * 2 + 1 } else { backslashes };
			line.extend(std::iter::repeat_n(backslash, escaped_backslashes));
			line.push(character);
			backslashes = 0;
		}
		line.extend(std::iter::repeat_n(backslash, backslashes * 2));

		line.push(quote);
	}

	line.push(0);
	line
}

pub fn terminate(process_id: u32) {
	let process_id = process_id.to_string();

	// On Windows only `taskkill` also ends the process's children, which would otherwise keep holding the port or the browser
	let ended = if cfg!(windows) {
		Command::new("taskkill").args(["/pid", &process_id, "/T", "/F"]).output()
	} else {
		Command::new("kill").arg(&process_id).output()
	};
	if let Err(error) = ended {
		eprintln!("Failed to end process {process_id}: {error}");
	}
}

// Whether anything answers HTTP requests on a local port
fn port_answers(port: u16) -> bool {
	let address = SocketAddr::from(([127, 0, 0, 1], port));
	let Ok(mut stream) = TcpStream::connect_timeout(&address, PROBE_TIMEOUT) else { return false };

	let request = format!("GET / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
	if stream.set_read_timeout(Some(PROBE_TIMEOUT)).is_err() || stream.write_all(request.as_bytes()).is_err() {
		return false;
	}

	let mut first_byte = [0];
	matches!(stream.read(&mut first_byte), Ok(1))
}

// The ID of the process listening on a local port, if the operating system's own tools can tell
fn listening_process_id(port: u16) -> Option<u32> {
	let listing = if cfg!(windows) {
		Command::new("netstat").args(["-ano", "-p", "TCP"]).output()
	} else {
		Command::new("lsof").args(["-nP", &format!("-iTCP:{port}"), "-sTCP:LISTEN", "-t"]).output()
	}
	.ok()?;
	let listing = String::from_utf8_lossy(&listing.stdout);

	// On Windows the process ID ends the port's listening line, found by its lack of a foreign address since the state's name is translated
	let line = if cfg!(windows) {
		let local_address_suffix = format!(":{port}");
		listing.lines().find(|line| {
			let columns = line.split_whitespace().collect::<Vec<_>>();
			columns.get(1).is_some_and(|local_address| local_address.ends_with(&local_address_suffix)) && columns.get(2) == Some(&"0.0.0.0:0")
		})?
	} else {
		listing.lines().next()?
	};

	line.split_whitespace().last()?.parse().ok().filter(|process_id| *process_id > 0)
}

// The real Node.js program rather than a shim that runs it as a child, so the process started is the one that listens on the port
fn node_executable() -> Result<PathBuf, String> {
	let output = Command::new("node")
		.args(["--print", "process.execPath"])
		.output()
		.map_err(|error| format!("Failed to run Node.js: {error}"))?;

	let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
	if !output.status.success() || path.is_empty() {
		return Err("Node.js did not say where it is installed".to_string());
	}
	Ok(PathBuf::from(path))
}

/// The dev server that `serve` started, if its port still answers and is still held by the process that was recorded.
pub fn dev_server_record() -> Option<DevServerRecord> {
	let record: DevServerRecord = read_record(&paths::dev_server_file())?;
	(port_answers(record.port) && listening_process_id(record.port) == Some(record.process_id)).then_some(record)
}

pub fn start_dev_server(port: u16) -> Result<DevServerRecord, String> {
	if dev_server_record().is_some() {
		return Err("The dev server is already running, so stop it first".to_string());
	}
	if port_answers(port) {
		return Err(format!("Something already answers on port {port}, so use it without `serve` or choose another --port"));
	}

	let vite = paths::frontend_directory().join("node_modules").join("vite").join("bin").join("vite.js");
	if !vite.exists() {
		return Err("The frontend's packages are not installed, which `cargo run` does on its first launch".to_string());
	}

	let arguments = [
		vite.display().to_string(),
		"--port".into(),
		port.to_string(),
		"--strictPort".into(),
		"--host".into(),
		"127.0.0.1".into(),
	];
	let mut process = spawn_detached(&node_executable()?, &arguments, &paths::frontend_directory(), &paths::dev_server_log_file())?;
	let record = DevServerRecord { port, process_id: process.id };

	let deadline = Instant::now() + DEV_SERVER_START_TIMEOUT;
	while Instant::now() < deadline {
		// Vite exits when it cannot take the port, so an answer only counts while the process started here is still running
		let running = process.is_running();
		if running && port_answers(port) {
			if let Err(error) = write_record(&paths::dev_server_file(), &record) {
				terminate(record.process_id);
				return Err(error);
			}
			return Ok(record);
		}
		if !running {
			return Err(format!("The dev server failed to start:\n{}", read_log(&paths::dev_server_log_file())));
		}

		std::thread::sleep(POLL_INTERVAL);
	}

	terminate(record.process_id);
	Err(format!("The dev server did not respond in time:\n{}", read_log(&paths::dev_server_log_file())))
}

// A record that is no longer live may name a process ID since reused by another program, so it is only removed
pub fn stop_dev_server() {
	if let Some(record) = dev_server_record() {
		terminate(record.process_id);
	}
	let _ = std::fs::remove_file(paths::dev_server_file());
}

#[cfg(all(test, windows))]
mod tests {
	use super::*;

	#[test]
	fn command_line_arguments_are_quoted() {
		let arguments = ["plain", "with space", "quote\"inside", "backslash\\\"quote", "trailing\\", ""].map(String::from);
		let line = windows_command_line(Path::new(r"C:\Program Files\host.exe"), &arguments);

		let expected = r#""C:\Program Files\host.exe" "plain" "with space" "quote\"inside" "backslash\\\"quote" "trailing\\" """#;
		assert_eq!(String::from_utf16(&line[..line.len() - 1]).unwrap(), expected);
		assert_eq!(line.last(), Some(&0));
	}
}

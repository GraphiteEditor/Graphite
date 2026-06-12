use std::path::PathBuf;
use std::process;

pub mod requirements;
pub mod wasm;

pub enum Action {
	Run,
	Build,
	Explore(Option<String>),
}

pub enum Target {
	Web,
	Desktop,
	Cli,
}

pub enum Profile {
	Default,
	Release,
	Debug,
}

pub struct Task {
	pub action: Action,
	pub target: Target,
	pub profile: Profile,
	pub args: Vec<String>,
}

impl Task {
	pub fn parse(args: &[&str]) -> Option<Self> {
		let split = args.iter().position(|a| *a == "--").unwrap_or(args.len());
		let passthru_args = args[split..].iter().skip(1).map(|s| s.to_string()).collect();
		let args = &args[..split];

		let (action, args) = match args.first() {
			Some(&"build") => (Action::Build, &args[1..]),
			Some(&"run") => (Action::Run, &args[1..]),
			Some(&"explore") => (Action::Explore(args.get(1).map(|s| s.to_string())), &[] as &[&str]),
			Some(&"help") => return None,
			_ => (Action::Run, args),
		};

		let (target, args) = match args.first() {
			Some(&"desktop") => (Target::Desktop, &args[1..]),
			Some(&"web") => (Target::Web, &args[1..]),
			Some(&"cli") => (Target::Cli, &args[1..]),
			_ => (Target::Web, args),
		};

		let profile = match args.first() {
			Some(&"release") => Profile::Release,
			Some(&"debug") => Profile::Debug,
			None => Profile::Default,
			_ => return None,
		};

		Some(Task {
			target,
			action,
			profile,
			args: passthru_args,
		})
	}
}

pub fn run(command: &str) -> Result<(), Error> {
	run_from(command, None)
}

/// Installs the frontend's npm packages and branding assets by running the `setup` script from `frontend/package.json`
pub fn run_frontend_setup() -> Result<(), Error> {
	let workspace_dir = std::path::PathBuf::from(env!("CARGO_WORKSPACE_DIR"));
	let frontend_dir = workspace_dir.join("frontend");
	let npm = if cfg!(target_os = "windows") { "npm.cmd" } else { "npm" };
	run_from(&format!("{npm} run setup"), Some(&frontend_dir))
}

/// Runs Vite from the `frontend/` directory by invoking its JS entry point with Node.js directly.
pub fn run_vite_in_frontend_dir(args: &str) -> Result<(), Error> {
	let workspace_dir = std::path::PathBuf::from(env!("CARGO_WORKSPACE_DIR"));
	let frontend_dir = workspace_dir.join("frontend");

	// Calling the script avoids npm's `vite.cmd` batch shim, which `cmd.exe` interrupts with a "Terminate batch job (Y/N)?" prompt on Ctrl+C
	run_from(&format!("node node_modules/vite/bin/vite.js {args}"), Some(&frontend_dir))
}

/// Runs the dev server's process supervisor from the `frontend/` directory, given its program and arguments.
pub fn run_dev_server_in_frontend_dir(program: &str, args: &[&str]) -> Result<(), Error> {
	let workspace_dir = std::path::PathBuf::from(env!("CARGO_WORKSPACE_DIR"));
	let frontend_dir = workspace_dir.join("frontend");

	let mut cmd = process::Command::new(program);
	cmd.args(args);
	cmd.current_dir(&frontend_dir);

	// On Windows, the supervisor is placed in its own process group which doesn't receive the console's Ctrl+C events.
	// Instead, a console handler kills the entire process tree at once. A single Ctrl+C thereby shuts everything down
	// silently and immediately, rather than letting each descendant process race to react to the event with its own
	// error messages and prompts. (Unix terminals already deliver the signal to the whole foreground process group.)
	#[cfg(target_os = "windows")]
	{
		use std::os::windows::process::CommandExt;
		const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
		cmd.creation_flags(CREATE_NEW_PROCESS_GROUP);
	}

	let command_str = format!("{program} {}", args.join(" "));
	let mut child = cmd.spawn().map_err(|e| Error::Io(e, format!("Failed to spawn command '{command_str}'")))?;

	#[cfg(target_os = "windows")]
	ctrl_c_windows::install_handler(child.id());

	let exit_code = child.wait().map_err(|e| Error::Io(e, format!("Failed to wait for command '{command_str}'")))?;

	#[cfg(target_os = "windows")]
	if ctrl_c_windows::interrupted() {
		return Ok(());
	}

	if !exit_code.success() {
		return Err(Error::Command(command_str, exit_code));
	}
	Ok(())
}

/// Console Ctrl+C handling for [`run_dev_server_in_frontend_dir`]: terminates the dev server's process tree and
/// reports the interruption so the exit is treated as a success.
#[cfg(target_os = "windows")]
mod ctrl_c_windows {
	use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

	static INTERRUPTED: AtomicBool = AtomicBool::new(false);
	static CHILD_PID: AtomicU32 = AtomicU32::new(0);

	#[link(name = "kernel32")]
	unsafe extern "system" {
		fn SetConsoleCtrlHandler(handler: Option<unsafe extern "system" fn(u32) -> i32>, add: i32) -> i32;
	}

	/// Windows runs this on a dedicated thread when the console receives Ctrl+C (or Ctrl+Break, or a window close)
	unsafe extern "system" fn ctrl_handler(_ctrl_type: u32) -> i32 {
		INTERRUPTED.store(true, Ordering::SeqCst);

		let pid = CHILD_PID.load(Ordering::SeqCst);
		if pid != 0 {
			let _ = std::process::Command::new("taskkill")
				.args(["/T", "/F", "/PID", &pid.to_string()])
				.stdout(std::process::Stdio::null())
				.stderr(std::process::Stdio::null())
				.status();
		}

		// Report the event as handled so the default handler doesn't also terminate this process
		1
	}

	pub fn install_handler(child_pid: u32) {
		CHILD_PID.store(child_pid, Ordering::SeqCst);
		unsafe { SetConsoleCtrlHandler(Some(ctrl_handler), 1) };
	}

	pub fn interrupted() -> bool {
		INTERRUPTED.load(Ordering::SeqCst)
	}
}

pub fn open_url(url: &str) -> Result<(), Error> {
	#[cfg(target_os = "windows")]
	let mut cmd = process::Command::new("cmd");
	#[cfg(target_os = "windows")]
	cmd.args(["/c", "start", url]);

	#[cfg(target_os = "macos")]
	let mut cmd = process::Command::new("open");
	#[cfg(target_os = "macos")]
	cmd.arg(url);

	#[cfg(not(any(target_os = "windows", target_os = "macos")))]
	let mut cmd = process::Command::new("xdg-open");
	#[cfg(not(any(target_os = "windows", target_os = "macos")))]
	cmd.arg(url);

	let command_str = format!("{:?}", cmd);
	let exit_code = cmd
		.spawn()
		.map_err(|e| Error::Io(e, format!("Failed to spawn command '{command_str}'")))?
		.wait()
		.map_err(|e| Error::Io(e, format!("Failed to wait for command '{command_str}'")))?;
	if !exit_code.success() {
		return Err(Error::Command(command_str, exit_code));
	}
	Ok(())
}

fn run_from(command: &str, dir: Option<&PathBuf>) -> Result<(), Error> {
	let command = command.split_whitespace().collect::<Vec<_>>();
	let mut cmd = process::Command::new(command[0]);
	if command.len() > 1 {
		cmd.args(&command[1..]);
	}
	if let Some(dir) = dir {
		cmd.current_dir(dir);
	}
	let exit_code = cmd
		.spawn()
		.map_err(|e| Error::Io(e, format!("Failed to spawn command '{}'", command.join(" "))))?
		.wait()
		.map_err(|e| Error::Io(e, format!("Failed to wait for command '{}'", command.join(" "))))?;
	if !exit_code.success() {
		return Err(Error::Command(command.join(" "), exit_code));
	}
	Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("{1}: {0}")]
	Io(#[source] std::io::Error, String),

	#[error("Command '{0}' exited with code {1}")]
	Command(String, process::ExitStatus),
}

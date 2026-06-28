use crate::Error;
use duct::{Handle, ReaderHandle};
use std::ffi::OsString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

pub use duct::{Expression, cmd};

pub mod prelude {
	pub use super::{Expression, ExpressionExt, Sequence, TerminalColor, cmd, sequence, sequence_then, supervise, utils};
}

pub trait ExpressionExt {
	fn arg(self, arg: impl Into<OsString>) -> Self;
	fn args<I, S>(self, args: I) -> Self
	where
		I: IntoIterator<Item = S>,
		S: Into<OsString>;
	fn arg_if(self, cond: bool, arg: impl Into<OsString>) -> Self;
	fn args_if<I, S>(self, cond: bool, args: I) -> Self
	where
		I: IntoIterator<Item = S>,
		S: Into<OsString>;
	fn run(self) -> Result<(), Error>;
	fn read(self) -> Result<String, Error>;
	fn output_unchecked(self) -> Result<std::process::Output, Error>;
}

impl ExpressionExt for Expression {
	fn arg(self, arg: impl Into<OsString>) -> Self {
		let arg = arg.into();
		self.before_spawn(move |c| {
			c.arg(&arg);
			Ok(())
		})
	}

	fn args<I, S>(self, args: I) -> Self
	where
		I: IntoIterator<Item = S>,
		S: Into<OsString>,
	{
		let args: Vec<OsString> = args.into_iter().map(Into::into).collect();
		self.before_spawn(move |c| {
			c.args(&args);
			Ok(())
		})
	}

	fn arg_if(self, cond: bool, arg: impl Into<OsString>) -> Self {
		if cond { self.arg(arg) } else { self }
	}

	fn args_if<I, S>(self, cond: bool, args: I) -> Self
	where
		I: IntoIterator<Item = S>,
		S: Into<OsString>,
	{
		if cond { self.args(args) } else { self }
	}

	fn run(self) -> Result<(), Error> {
		Expression::run(&self).map_err(Error::Command)?;
		Ok(())
	}

	fn read(self) -> Result<String, Error> {
		Expression::read(&self).map_err(Error::Command)
	}

	fn output_unchecked(self) -> Result<std::process::Output, Error> {
		let e = self.unchecked().stdout_capture().stderr_capture();
		Expression::run(&e).map_err(Error::Command)
	}
}

pub enum TerminalColor {
	Black,
	Red,
	Green,
	Yellow,
	Blue,
	Magenta,
	Cyan,
	White,
	BrightBlack,
	BrightRed,
	BrightGreen,
	BrightYellow,
	BrightBlue,
	BrightMagenta,
	BrightCyan,
	BrightWhite,
	Reset,
}
impl TerminalColor {
	fn as_str(&self) -> &'static str {
		match self {
			Self::Black => "\x1b[30m",
			Self::Red => "\x1b[31m",
			Self::Green => "\x1b[32m",
			Self::Yellow => "\x1b[33m",
			Self::Blue => "\x1b[34m",
			Self::Magenta => "\x1b[35m",
			Self::Cyan => "\x1b[36m",
			Self::White => "\x1b[37m",
			Self::BrightBlack => "\x1b[90m",
			Self::BrightRed => "\x1b[91m",
			Self::BrightGreen => "\x1b[92m",
			Self::BrightYellow => "\x1b[93m",
			Self::BrightBlue => "\x1b[94m",
			Self::BrightMagenta => "\x1b[95m",
			Self::BrightCyan => "\x1b[96m",
			Self::BrightWhite => "\x1b[97m",
			Self::Reset => "\x1b[0m",
		}
	}
}

pub fn sequence<I: IntoIterator<Item = Expression>>(expressions: I) -> Sequence {
	sequence_then(expressions, Vec::new)
}

/// Like [`sequence`], but once the steps complete successfully, `follow_up` is called and any steps it
/// returns are run as part of the same (still killable) sequence. This lets a build append a clean-and-rebuild
/// when it detects its own corrupt output, without spawning a detached job that could race a fresh build.
pub fn sequence_then<I, F>(expressions: I, follow_up: F) -> Sequence
where
	I: IntoIterator<Item = Expression>,
	F: FnOnce() -> Vec<Expression> + Send + 'static,
{
	let expressions: Vec<Expression> = expressions.into_iter().collect();
	let current: Arc<Mutex<Option<Arc<Handle>>>> = Arc::new(Mutex::new(None));
	let killed = Arc::new(AtomicBool::new(false));

	let worker_current = Arc::clone(&current);
	let worker_killed = Arc::clone(&killed);
	let worker = std::thread::spawn(move || {
		// Runs every step, returning `false` if it was killed or a step failed to start/complete.
		let run_steps = |steps: Vec<Expression>| -> bool {
			for expr in steps {
				if worker_killed.load(Ordering::SeqCst) {
					return false;
				}
				let handle = match expr.start() {
					Ok(h) => Arc::new(h),
					Err(e) => {
						eprintln!("sequence: failed to start step: {e}");
						return false;
					}
				};
				{
					let mut slot = worker_current.lock().unwrap();
					if worker_killed.load(Ordering::SeqCst) {
						let _ = handle.kill();
						return false;
					}
					*slot = Some(Arc::clone(&handle));
				}
				let result = handle.wait().map(|_| ());
				worker_current.lock().unwrap().take();
				if worker_killed.load(Ordering::SeqCst) {
					return false;
				}
				if let Err(e) = result {
					eprintln!("sequence: step failed: {e}");
					return false;
				}
			}
			true
		};

		if run_steps(expressions) {
			let extra = follow_up();
			if !extra.is_empty() {
				run_steps(extra);
			}
		}
	});

	Sequence {
		current,
		killed,
		worker: Some(worker),
	}
}

pub struct Sequence {
	current: Arc<Mutex<Option<Arc<Handle>>>>,
	killed: Arc<AtomicBool>,
	worker: Option<JoinHandle<()>>,
}
impl Sequence {
	pub fn kill(&self) {
		let slot = self.current.lock().unwrap();
		self.killed.store(true, Ordering::SeqCst);
		if let Some(handle) = slot.as_ref() {
			let _ = handle.kill();
		}
	}

	pub fn wait(&mut self) {
		if let Some(w) = self.worker.take() {
			let _ = w.join();
		}
	}
}
impl Drop for Sequence {
	fn drop(&mut self) {
		self.kill();
		self.wait();
	}
}

pub fn supervise<I, S>(children: I) -> Result<(), Error>
where
	I: IntoIterator<Item = (S, TerminalColor, Expression)>,
	S: Into<String>,
{
	use std::io::{BufRead, BufReader, IsTerminal, Write};
	use std::sync::Arc;
	use std::thread;
	use std::time::Duration;

	#[cfg(target_os = "windows")]
	windows_ctrl_c::install();

	let mut handles: Vec<(String, TerminalColor, Arc<ReaderHandle>)> = Vec::new();
	for (label, color, expr) in children {
		#[cfg(target_os = "windows")]
		let expr = expr.before_spawn(|cmd| {
			use std::os::windows::process::CommandExt;
			const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
			cmd.creation_flags(CREATE_NEW_PROCESS_GROUP);
			Ok(())
		});

		let handle = expr.stderr_to_stdout().reader().map_err(Error::Command)?;
		handles.push((label.into(), color, Arc::new(handle)));
	}

	let mut io_threads = Vec::new();
	for (label, color, handle) in handles.iter() {
		let prefix = if std::io::stdout().is_terminal() {
			format!("{color}[{label}]{reset} ", color = color.as_str(), reset = TerminalColor::Reset.as_str())
		} else {
			format!("[{label}] ")
		};
		let handle = handle.clone();
		io_threads.push(thread::spawn(move || {
			let reader = BufReader::new(&*handle);
			for line in reader.lines().map_while(Result::ok) {
				let out = std::io::stdout();
				let mut out = out.lock();
				let _ = writeln!(out, "{prefix}{line}");
			}
		}));
	}

	let mut reason: Option<(String, std::process::ExitStatus)> = None;

	loop {
		#[cfg(target_os = "windows")]
		if windows_ctrl_c::interrupted() {
			break;
		}

		for (label, _color, handle) in handles.iter() {
			if let Ok(Some(output)) = handle.try_wait() {
				reason = Some((label.clone(), output.status));
				break;
			}
		}
		if reason.is_some() {
			break;
		}

		thread::sleep(Duration::from_millis(200));
	}

	for (_, _color, handle) in handles.iter() {
		if matches!(handle.try_wait(), Ok(Some(_))) {
			continue;
		}
		#[cfg(target_os = "windows")]
		{
			for pid in handle.pids() {
				let _ = std::process::Command::new("taskkill")
					.args(["/T", "/F", "/PID", &pid.to_string()])
					.stdout(std::process::Stdio::null())
					.stderr(std::process::Stdio::null())
					.status();
			}
		}
		#[cfg(not(target_os = "windows"))]
		{
			let _ = handle.kill();
		}
	}

	for t in io_threads {
		let _ = t.join();
	}

	match reason {
		Some((label, status)) if !status.success() => Err(Error::Command(std::io::Error::other(format!("supervised child '{label}' exited with status {status}")))),
		_ => Ok(()),
	}
}

pub mod utils {
	use super::*;

	pub fn internal(name: &str) -> Expression {
		let package = format!("cargo-run-internal-{name}");
		cmd!("cargo", "run", "-p", package, "--")
	}

	pub fn npm<I, S>(args: I) -> Expression
	where
		I: IntoIterator<Item = S>,
		S: Into<OsString>,
	{
		let prog = if cfg!(target_os = "windows") { "npm.cmd" } else { "npm" };
		cmd(prog, args)
	}

	pub fn node_bin(rel: &str) -> Expression {
		cmd!("node", format!("node_modules/{rel}"))
	}

	pub fn open_url(url: &str) -> Result<(), Error> {
		#[cfg(target_os = "windows")]
		let expr = cmd!("cmd", "/c", "start", url);
		#[cfg(target_os = "macos")]
		let expr = cmd!("open", url);
		#[cfg(not(any(target_os = "windows", target_os = "macos")))]
		let expr = cmd!("xdg-open", url);
		expr.run()
	}
}

#[cfg(target_os = "windows")]
mod windows_ctrl_c {
	use std::sync::Once;
	use std::sync::atomic::{AtomicBool, Ordering};

	static INTERRUPTED: AtomicBool = AtomicBool::new(false);

	#[link(name = "kernel32")]
	unsafe extern "system" {
		fn SetConsoleCtrlHandler(handler: Option<unsafe extern "system" fn(u32) -> i32>, add: i32) -> i32;
	}

	unsafe extern "system" fn handler(_ctrl_type: u32) -> i32 {
		INTERRUPTED.store(true, Ordering::SeqCst);
		1 // Report the event as handled
	}

	pub fn install() {
		INTERRUPTED.store(false, Ordering::SeqCst);
		static REGISTER: Once = Once::new();
		REGISTER.call_once(|| unsafe {
			SetConsoleCtrlHandler(Some(handler), 1);
		});
	}

	pub fn interrupted() -> bool {
		INTERRUPTED.load(Ordering::SeqCst)
	}
}

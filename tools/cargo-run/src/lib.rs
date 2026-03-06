use std::path::PathBuf;
use std::process;

pub mod requirements;

pub enum Action {
	Run,
	Build,
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

pub fn npm_run_in_frontend_dir(args: &str) -> Result<(), Error> {
	let workspace_dir = std::path::PathBuf::from(env!("CARGO_WORKSPACE_DIR"));
	let frontend_dir = workspace_dir.join("frontend");
	let npm = if cfg!(target_os = "windows") { "npm.cmd" } else { "npm" };
	run_from(&format!("{npm} run {args}"), Some(&frontend_dir))
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

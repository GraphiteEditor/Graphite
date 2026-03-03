pub mod deps;

use std::process;

pub enum Target {
	Web,
	Desktop,
}

pub enum Action {
	Run,
	Build,
}

pub enum Profile {
	Default,
	Release,
	Debug,
	Profiling,
}

pub struct Task {
	pub target: Target,
	pub action: Action,
	pub profile: Profile,
}

impl Task {
	pub fn parse(args: &[&str]) -> Option<Self> {
		let (target, rest) = match args.first() {
			Some(&"desktop") => (Target::Desktop, &args[1..]),
			Some(&"web") => (Target::Web, &args[1..]),
			_ => (Target::Web, args),
		};

		let (action, rest) = match rest.first() {
			Some(&"build") => (Action::Build, &rest[1..]),
			_ => (Action::Run, rest),
		};

		let profile = match rest.first().copied().unwrap_or_default() {
			"" => Profile::Default,
			"release" => Profile::Release,
			"debug" => Profile::Debug,
			"profiling" => Profile::Profiling,
			_ => return None,
		};

		Some(Task { target, action, profile })
	}
}

pub fn run(command: &str) -> Result<(), Error> {
	run_from(command, None)
}

pub fn run_in_frontend_dir(command: &str) -> Result<(), Error> {
	run_from(command, Some("frontend"))
}

pub fn run_from(command: &str, dir: Option<&str>) -> Result<(), Error> {
	let workspace_dir = std::path::PathBuf::from(env!("CARGO_WORKSPACE_DIR"));
	let dir = if let Some(dir) = dir { workspace_dir.join(dir) } else { workspace_dir };
	let command = command.split_whitespace().collect::<Vec<_>>();
	let mut cmd = process::Command::new(command[0]);
	if command.len() > 1 {
		cmd.args(&command[1..]);
	}
	cmd.current_dir(dir);
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

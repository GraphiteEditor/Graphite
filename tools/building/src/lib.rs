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

pub fn run(comand: &str) {
	run_from(comand, None);
}

pub fn run_in_frontend_dir(comand: &str) {
	run_from(comand, Some("frontend"));
}

pub fn run_from(comand: &str, dir: Option<&str>) {
	let workspace_dir = std::path::PathBuf::from(env!("CARGO_WORKSPACE_DIR"));
	let dir = if let Some(dir) = dir { workspace_dir.join(dir) } else { workspace_dir };
	let comand = comand.split_whitespace().collect::<Vec<_>>();
	let mut cmd = process::Command::new(comand[0]);
	if comand.len() > 1 {
		cmd.args(&comand[1..]);
	}
	cmd.current_dir(dir);
	let exit_code = cmd
		.spawn()
		.unwrap_or_else(|e| {
			panic!("Failed to run command '{}': {e}", comand.join(" "));
		})
		.wait()
		.unwrap_or_else(|e| {
			panic!("Failed to wait for command '{}': {e}", comand.join(" "));
		});
	if !exit_code.success() {
		panic!("Command '{}' exited with code {}", comand.join(" "), exit_code);
	}
}

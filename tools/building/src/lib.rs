pub mod checks;

use std::process;

pub enum Profile {
	Default,
	Release,
	Debug,
	Profiling,
	Error,
}

impl From<&[&str]> for Profile {
	fn from(arg: &[&str]) -> Self {
		arg.first().map(|s| s.to_string()).as_deref().unwrap_or_default().into()
	}
}

impl From<&str> for Profile {
	fn from(arg: &str) -> Self {
		match arg {
			"release" => Profile::Release,
			"debug" => Profile::Debug,
			"profiling" => Profile::Profiling,
			_ if arg.is_empty() => Profile::Default,
			_ => Profile::Error,
		}
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
	cmd.spawn()
		.unwrap_or_else(|e| {
			panic!("Failed to run command '{}': {e}", comand.join(" "));
		})
		.wait()
		.unwrap_or_else(|e| {
			panic!("Failed to wait for command '{}': {e}", comand.join(" "));
		});
}

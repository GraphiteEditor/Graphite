use std::path::PathBuf;

pub mod branding;
pub mod cmd;
pub mod frontend;
pub mod requirements;

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

pub fn workspace_dir() -> PathBuf {
	PathBuf::from(env!("CARGO_WORKSPACE_DIR"))
}

pub fn target_dir() -> PathBuf {
	match std::env::var_os("CARGO_TARGET_DIR") {
		Some(custom_dir) => workspace_dir().join(custom_dir),
		None => workspace_dir().join("target"),
	}
}

pub fn install_dir() -> PathBuf {
	target_dir().join("cargo-run")
}

pub fn bin_dir() -> PathBuf {
	install_dir().join("bin")
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("One or more requirements were not met")]
	RequirementsNotMet,

	#[error("{1}: {0}")]
	Io(#[source] std::io::Error, String),
	/// Used by the duct-based `cmd` module; folds in `Command` once call sites are migrated.
	#[error("{0}")]
	Command(#[source] std::io::Error),
}

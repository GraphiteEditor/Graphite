use std::process::ExitCode;

use building::*;

fn usage() {
	eprintln!("usage: cargo run [<command>] [release|debug|profiling]");
	eprintln!();
	eprintln!("commands:");
	eprintln!("  web               Run the dev server");
	eprintln!("  web build         Build the web version");
	eprintln!("  desktop           Run the desktop app");
	eprintln!("  desktop build     Build the desktop version");
}

fn main() -> ExitCode {
	let args: Vec<String> = std::env::args().collect();
	let args: Vec<&str> = args.iter().skip(1).map(String::as_str).collect();

	let task = match Task::parse(&args) {
		Some(run) => run,
		None => {
			usage();
			return ExitCode::FAILURE;
		}
	};

	if let Err(e) = run_task(&task) {
		eprintln!("Error: {e}");
		return ExitCode::FAILURE;
	}
	ExitCode::SUCCESS
}

fn run_task(task: &Task) -> Result<(), Error> {
	deps::check(task)?;

	match (&task.target, &task.action, &task.profile) {
		(Target::Web, Action::Run, Profile::Debug | Profile::Default) => npm_run_in_frontend_dir("start")?,
		(Target::Web, Action::Run, Profile::Release) => npm_run_in_frontend_dir("production")?,
		(Target::Web, Action::Run, Profile::Profiling) => npm_run_in_frontend_dir("profiling")?,

		(Target::Web, Action::Build, Profile::Debug) => npm_run_in_frontend_dir("build-dev")?,
		(Target::Web, Action::Build, Profile::Release | Profile::Default) => npm_run_in_frontend_dir("build")?,
		(Target::Web, Action::Build, Profile::Profiling) => npm_run_in_frontend_dir("build-profiling")?,

		(Target::Desktop, Action::Run, Profile::Debug | Profile::Default) => {
			npm_run_in_frontend_dir("build-native-dev")?;
			run("cargo run -p third-party-licenses --features desktop")?;
			run("cargo run -p graphite-desktop-bundle -- open")?;
		}
		(Target::Desktop, Action::Run, Profile::Release) => {
			npm_run_in_frontend_dir("build-native")?;
			run("cargo run -p third-party-licenses --features desktop")?;
			run("cargo run -r -p graphite-desktop-bundle -- open")?;
		}
		(Target::Desktop, Action::Run, Profile::Profiling) => todo!("profiling run for desktop"),

		(Target::Desktop, Action::Build, Profile::Debug) => {
			npm_run_in_frontend_dir("build-native-dev")?;
			run("cargo run -p third-party-licenses --features desktop")?;
			run("cargo run -p graphite-desktop-bundle")?;
		}
		(Target::Desktop, Action::Build, Profile::Release | Profile::Default) => {
			npm_run_in_frontend_dir("build-native")?;
			run("cargo run -p third-party-licenses --features desktop")?;
			run("cargo run -r -p graphite-desktop-bundle")?;
		}
		(Target::Desktop, Action::Build, Profile::Profiling) => todo!("profiling build for desktop"),
	}
	Ok(())
}

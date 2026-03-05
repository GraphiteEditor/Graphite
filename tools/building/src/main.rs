use std::process::ExitCode;

use building::*;

fn usage() {
	eprintln!();
	eprintln!("Usage: cargo run [<command>] [release|debug|profiling] -- [args...]");
	eprintln!();
	eprintln!("Commands:");
	eprintln!("  web [run]         Run the web app on local dev server");
	eprintln!("  web build         Build the web app");
	eprintln!("  desktop [run]     Run the desktop app");
	eprintln!("  desktop build     Build the desktop app");
	eprintln!("  cli [run]         Run the Graphen CLI");
	eprintln!("  cli build         Build the Graphen CLI");
	eprintln!("  help              Show this message");
	eprintln!();
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

		(Target::Desktop, action, mut profile) => {
			if matches!(profile, Profile::Default) {
				profile = match action {
					Action::Build => &Profile::Release,
					Action::Run => &Profile::Debug,
				}
			}

			if matches!(profile, Profile::Release) {
				npm_run_in_frontend_dir("build-native")?;
			} else {
				npm_run_in_frontend_dir("build-native-dev")?;
			};

			run("cargo run -p third-party-licenses --features desktop")?;

			let cargo_profile = match profile {
				Profile::Debug => "dev",
				Profile::Release => "release",
				Profile::Profiling => "profiling",
				Profile::Default => unreachable!(),
			};
			let args = if matches!(action, Action::Run) {
				format!(" -- open {}", task.args.join(" "))
			} else {
				"".to_string()
			};
			run(&format!("cargo run --profile {cargo_profile} -p graphite-desktop-bundle{args}"))?;
		}

		(Target::Cli, Action::Run, Profile::Debug | Profile::Default) => run(&format!("cargo run -p graphene-cli -- {}", task.args.join(" ")))?,
		(Target::Cli, Action::Run, Profile::Release) => run(&format!("cargo run -r -p graphene-cli -- {}", task.args.join(" ")))?,
		(Target::Cli, Action::Run, Profile::Profiling) => run(&format!("cargo run --profile profiling -p graphene-cli -- {}", task.args.join(" ")))?,

		(Target::Cli, Action::Build, Profile::Debug) => run("cargo build -p graphene-cli")?,
		(Target::Cli, Action::Build, Profile::Release | Profile::Default) => run("cargo build -r -p graphene-cli")?,
		(Target::Cli, Action::Build, Profile::Profiling) => run("cargo build --profile profiling -p graphene-cli")?,
	}
	Ok(())
}

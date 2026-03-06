use std::process::ExitCode;

use cargo_run::*;

fn usage() {
	println!();
	println!("USAGE:");
	println!("  cargo run [<command>] [<target>] [<profile>] [-- [args]...]");
	println!();
	println!("COMMON USAGE:");
	println!("  cargo run            Run the web app");
	println!("  cargo run desktop    Run the desktop app");
	println!();
	println!("OPTIONS:");
	println!("<command>:");
	println!("  [run]        Run the selected target (default)");
	println!("  build        Build the selected target");
	println!("  help         Show this message");
	println!("<target>:");
	println!("  [web]        Web app (default)");
	println!("  desktop      Desktop app");
	println!("  cli          Graphene CLI");
	println!("<profile>:");
	println!("  [debug]      Optimizations disabled (default for run)");
	println!("  [release]    Optimizations enabled (default for build)");
	println!();
	println!("MORE EXAMPLES:");
	println!("  cargo run build desktop");
	println!("  cargo run desktop release");
	println!("  cargo run cli -- --help");
	println!()
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
	requirements::check(task)?;

	match (&task.action, &task.target, &task.profile) {
		(Action::Run, Target::Web, Profile::Debug | Profile::Default) => npm_run_in_frontend_dir("start")?,
		(Action::Run, Target::Web, Profile::Release) => npm_run_in_frontend_dir("production")?,

		(Action::Build, Target::Web, Profile::Debug) => npm_run_in_frontend_dir("build-dev")?,
		(Action::Build, Target::Web, Profile::Release | Profile::Default) => npm_run_in_frontend_dir("build")?,

		(action, Target::Desktop, mut profile) => {
			if matches!(profile, Profile::Default) {
				profile = match action {
					Action::Run => &Profile::Debug,
					Action::Build => &Profile::Release,
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
				Profile::Default => unreachable!(),
			};
			let args = if matches!(action, Action::Run) {
				format!(" -- open {}", task.args.join(" "))
			} else {
				"".to_string()
			};
			run(&format!("cargo run --profile {cargo_profile} -p graphite-desktop-bundle{args}"))?;
		}

		(Action::Run, Target::Cli, Profile::Debug | Profile::Default) => run(&format!("cargo run -p graphene-cli -- {}", task.args.join(" ")))?,
		(Action::Run, Target::Cli, Profile::Release) => run(&format!("cargo run -r -p graphene-cli -- {}", task.args.join(" ")))?,

		(Action::Build, Target::Cli, Profile::Debug) => run("cargo build -p graphene-cli")?,
		(Action::Build, Target::Cli, Profile::Release | Profile::Default) => run("cargo build -r -p graphene-cli")?,
	}
	Ok(())
}

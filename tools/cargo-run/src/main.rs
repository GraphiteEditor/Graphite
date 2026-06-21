use std::process::ExitCode;

use cargo_run::cmd::prelude::*;
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
	println!("  explore      Open an assortment of tools for exploring the codebase");
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
	prepend_path();

	let args: Vec<String> = std::env::args().collect();
	let args: Vec<&str> = args.iter().skip(1).map(String::as_str).collect();

	let task = match Task::parse(&args) {
		Some(run) => run,
		None => {
			usage();
			return ExitCode::SUCCESS;
		}
	};

	if let Err(e) = run_task(&task) {
		eprintln!("Error: {e}");
		return ExitCode::FAILURE;
	}
	ExitCode::SUCCESS
}

fn explore_usage() {
	println!();
	println!("USAGE:");
	println!("  cargo run explore <tool>");
	println!();
	println!("OPTIONS:");
	println!("<tool>:");
	println!("  bisect    Binary search through recent commits to find which introduced a bug or feature");
	println!("  deps      View the crate dependency graph for the workspace");
	println!("  editor    View an interactive outline of the editor's message system architecture");
	println!();
}

fn run_task(task: &Task) -> Result<(), Error> {
	if let Action::Explore(tool) = &task.action {
		match tool.as_deref() {
			Some("bisect") => return utils::open_url("https://graphite.art/volunteer/guide/codebase-overview/debugging-tips/#build-bisect-tool"),
			Some("deps") => return utils::open_url("https://graphite.art/volunteer/guide/codebase-overview/#crate-dependency-graph"),
			Some("editor") => return utils::open_url("https://graphite.art/volunteer/guide/codebase-overview/editor-structure/#editor-outline"),
			None | Some("--help") => {
				explore_usage();
				return Ok(());
			}
			Some(other) => {
				eprintln!("Unknown explore tool: '{other}'");
				explore_usage();
				return Ok(());
			}
		}
	}

	requirements::check(task)?;

	match (&task.action, &task.target, &task.profile) {
		(Action::Run, Target::Web, Profile::Debug | Profile::Default) => frontend::watch(false)?,
		(Action::Run, Target::Web, Profile::Release) => frontend::watch(true)?,

		(Action::Build, Target::Web, Profile::Debug) => {
			frontend::setup()?;
			frontend::build_wasm(false, false)?;
			frontend::vite().args(["build", "--mode", "dev"]).run()?;
		}
		(Action::Build, Target::Web, Profile::Release | Profile::Default) => {
			frontend::setup()?;
			frontend::build_wasm(true, false)?;
			frontend::vite().args(["build"]).run()?;
		}

		(action, Target::Desktop, mut profile) => {
			if matches!(profile, Profile::Default) {
				profile = match action {
					Action::Run => &Profile::Debug,
					Action::Build => &Profile::Release,
					Action::Explore(_) => unreachable!(),
				}
			}

			// Build the editor's Wasm module with the `native` feature, then bundle the frontend with Vite
			frontend::setup()?;
			frontend::build_wasm(matches!(profile, Profile::Release), true)?;
			frontend::vite().args(["build", "--mode", "native"]).run()?;

			cmd!("cargo", "run", "-p", "third-party-licenses", "--features", "desktop").run()?;

			let cargo_profile = match profile {
				Profile::Debug => "dev",
				Profile::Release => "release",
				Profile::Default => unreachable!(),
			};
			cmd!("cargo", "run", "--profile", cargo_profile, "-p", "graphite-desktop-bundle")
				.args_if(matches!(action, Action::Run), ["--", "open"].into_iter().chain(task.args.iter().map(String::as_str)))
				.run()?;
		}

		(Action::Run, Target::Cli, Profile::Debug | Profile::Default) => cmd!("cargo", "run", "-p", "graphene-cli", "--").args(&task.args).run()?,
		(Action::Run, Target::Cli, Profile::Release) => cmd!("cargo", "run", "-r", "-p", "graphene-cli", "--").args(&task.args).run()?,

		(Action::Build, Target::Cli, Profile::Debug) => cmd!("cargo", "build", "-p", "graphene-cli").run()?,
		(Action::Build, Target::Cli, Profile::Release | Profile::Default) => cmd!("cargo", "build", "-r", "-p", "graphene-cli").run()?,

		(Action::Explore(_), _, _) => unreachable!(),
	}
	Ok(())
}

fn prepend_path() {
	let mut paths = vec![target_dir().join("cargo-run").join("bin")];
	if let Some(path) = std::env::var_os("PATH") {
		paths.extend(std::env::split_paths(&path));
	}
	if let Ok(joined) = std::env::join_paths(paths) {
		// Safety: this runs before any other threads are spawned
		unsafe { std::env::set_var("PATH", joined) };
	}
}

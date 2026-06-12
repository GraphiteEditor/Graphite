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
	// Put the managed Binaryen installation (if present) on PATH for this process and its children
	requirements::use_managed_binaryen();

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
			Some("bisect") => return open_url("https://graphite.art/volunteer/guide/codebase-overview/debugging-tips/#build-bisect-tool"),
			Some("deps") => return open_url("https://graphite.art/volunteer/guide/codebase-overview/#crate-dependency-graph"),
			Some("editor") => return open_url("https://graphite.art/volunteer/guide/codebase-overview/editor-structure/#editor-outline"),
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
		(Action::Run, Target::Web, Profile::Debug | Profile::Default) => run_web_dev_server(false)?,
		(Action::Run, Target::Web, Profile::Release) => run_web_dev_server(true)?,

		(Action::Build, Target::Web, Profile::Debug) => {
			run_frontend_setup()?;
			wasm::build(false, false)?;
			run_vite_in_frontend_dir("build --mode dev")?;
		}
		(Action::Build, Target::Web, Profile::Release | Profile::Default) => {
			run_frontend_setup()?;
			wasm::build(true, false)?;
			run_vite_in_frontend_dir("build")?;
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
			run_frontend_setup()?;
			wasm::build(matches!(profile, Profile::Release), true)?;
			run_vite_in_frontend_dir("build --mode native")?;

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

		(Action::Explore(_), _, _) => unreachable!(),
	}
	Ok(())
}

/// Builds the editor's Wasm module, then runs the Vite dev server alongside a `cargo watch` loop that rebuilds the Wasm module when the Rust source changes.
/// The two are run in parallel by `concurrently`, which labels their output and shuts both down when either exits.
/// Both `concurrently` and Vite are invoked through their JS entry points because npm's `.cmd` batch shims trip up Ctrl+C handling on Windows.
fn run_web_dev_server(release: bool) -> Result<(), Error> {
	const VITE: &str = "node node_modules/vite/bin/vite.js";
	const CONCURRENTLY: &str = "node_modules/concurrently/dist/bin/concurrently.js";

	run_frontend_setup()?;
	wasm::build(release, false)?;

	let rebuild_steps = wasm::watch_shell_commands(release).iter().map(|step| format!("--shell \"{step}\"")).collect::<Vec<_>>().join(" ");
	let watcher = format!("cargo watch --postpone --watch-when-idle --workdir=wrapper {rebuild_steps}");
	run_dev_server_in_frontend_dir("node", &[CONCURRENTLY, "-k", "-n", "VITE,RUST", VITE, &watcher])
}

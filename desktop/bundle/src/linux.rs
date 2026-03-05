use crate::common::*;

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
	let app_bin = build_bin("graphite-desktop-platform-linux", None)?;

	// TODO: Implement bundling for linux

	// TODO: Consider adding more useful cli
	let args: Vec<String> = std::env::args().collect();
	if let Some(pos) = args.iter().position(|a| a == "open") {
		let extra_args: Vec<&str> = args[pos + 1..].iter().map(|s| s.as_str()).collect();
		run_command(&app_bin.to_string_lossy(), &extra_args).expect("failed to open app");
	} else {
		eprintln!("Binary built and placed at {}", app_bin.to_string_lossy());
		eprintln!("Bundling for Linux is not yet implemented.");
		eprintln!("You can still start the app with the `open` subcommand. `cargo run -p graphite-desktop-bundle -- open`");
	}

	Ok(())
}

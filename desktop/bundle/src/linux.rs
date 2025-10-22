use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use crate::common::*;

const PACKAGE: &str = "graphite-desktop-platform-linux";

pub fn main() -> Result<(), Box<dyn Error>> {
	let app_bin = build_bin(PACKAGE, None)?;

	//TODO: Implement bundling for linux

	// TODO: Consider adding more useful cli
	if std::env::args().any(|a| a == "open") {
		let executable_path = app_dir.join(EXEC_PATH).join(APP_NAME);
		run_command(&executable_path.to_string_lossy(), &[]).expect("failed to open app");
	} else {
		println!("Binary built and placed at {}", app_bin.to_string_lossy());
		eprintln!("Bundling for Linux is not yet implemented.");
		eprintln!("You can still start the app with the `open` subcommand. `cargo run -p graphite-desktop-bundle -- open`");
		std::process::exit(1);
	}

	Ok(())
}

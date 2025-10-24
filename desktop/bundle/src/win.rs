use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use crate::common::*;

const PACKAGE: &str = "graphite-desktop-platform-win";
const EXECUTABLE: &str = "graphite-editor.exe";

pub fn main() -> Result<(), Box<dyn Error>> {
	let app_bin = build_bin(PACKAGE, None)?;

	let executable = bundle(&profile_path(), &app_bin);

	// TODO: Consider adding more useful cli
	if std::env::args().any(|a| a == "open") {
		let executable_path = executable.to_string_lossy();
		run_command(&executable_path, &[]).expect("failed to open app")
	}

	Ok(())
}

fn bundle(out_dir: &Path, app_bin: &Path) -> PathBuf {
	let app_dir = out_dir.join(APP_NAME);

	clean_dir(&app_dir);

	copy_dir(&cef_path(), &app_dir);

	let bin_path = app_dir.join(EXECUTABLE);
	fs::copy(app_bin, &bin_path).unwrap();

	bin_path
}

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use crate::common::*;

const EXECUTABLE: &str = "Graphite.exe";

pub fn main() -> Result<(), Box<dyn Error>> {
	let app_bin = build_bin("graphite-desktop-platform-win", None)?;

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

	if let Err(e) = remove_unnecessary_cef_files(&app_dir) {
		eprintln!("Failed to remove unnecessary CEF files: {}", e);
	}

	let bin_path = app_dir.join(EXECUTABLE);
	fs::copy(app_bin, &bin_path).unwrap();

	bin_path
}

fn remove_unnecessary_cef_files(app_dir: &Path) -> Result<(), Box<dyn Error>> {
	fs::remove_dir_all(app_dir.join("cmake"))?;
	fs::remove_dir_all(app_dir.join("include"))?;
	fs::remove_dir_all(app_dir.join("libcef_dll"))?;

	for entry in fs::read_dir(app_dir.join("locales"))? {
		let path = entry?.path();
		if path.is_file() && path.file_name() != Some("en-US.pak".as_ref()) {
			fs::remove_file(path)?;
		}
	}

	fs::remove_file(app_dir.join("archive.json"))?;
	fs::remove_file(app_dir.join("CMakeLists.txt"))?;
	fs::remove_file(app_dir.join("bootstrapc.exe"))?;
	fs::remove_file(app_dir.join("bootstrap.exe"))?;
	fs::remove_file(app_dir.join("libcef.lib"))?;

	Ok(())
}

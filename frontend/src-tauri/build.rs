use std::{fs, path::PathBuf};

fn main() {
	// Directory required for compilation, but not tracked by git if empty.
	let dist_dir: PathBuf = ["..", "dist"].iter().collect();
	fs::create_dir_all(dist_dir).unwrap();
	tauri_build::build()
}

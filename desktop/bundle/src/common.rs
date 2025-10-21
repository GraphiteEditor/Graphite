use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub(crate) const APP_NAME: &str = "Graphite";

fn profile_name() -> &'static str {
	let mut profile = env!("CARGO_PROFILE");
	if profile == "debug" {
		profile = "dev";
	}
	profile
}

pub(crate) fn profile_path() -> PathBuf {
	PathBuf::from(env!("CARGO_WORKSPACE_DIR")).join(format!("target/{}", env!("CARGO_PROFILE")))
}

pub(crate) fn cef_path() -> PathBuf {
	PathBuf::from(env!("CEF_PATH"))
}

pub(crate) fn build_bin(package: &str, bin: Option<&str>) -> Result<PathBuf, Box<dyn Error>> {
	let profile = &profile_name();
	let mut args = vec!["build", "--package", package, "--profile", profile];
	if let Some(bin) = bin {
		args.push("--bin");
		args.push(bin);
	}
	run_command("cargo", &args)?;
	let profile_path = profile_path();
	let mut bin_path = if let Some(bin) = bin { profile_path.join(bin) } else { profile_path.join(package) };
	if cfg!(target_os = "windows") {
		bin_path.set_extension("exe");
	}
	Ok(bin_path)
}

pub(crate) fn run_command(program: &str, args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
	let status = Command::new(program).args(args).stdout(Stdio::inherit()).stderr(Stdio::inherit()).status()?;
	if !status.success() {
		std::process::exit(1);
	}
	Ok(())
}

pub(crate) fn clean_dir(dir: &Path) {
	if dir.exists() {
		fs::remove_dir_all(dir).unwrap();
	}
	fs::create_dir_all(dir).unwrap();
}

pub(crate) fn copy_dir(src: &Path, dst: &Path) {
	fs::create_dir_all(dst).unwrap();
	for entry in fs::read_dir(src).unwrap() {
		let entry = entry.unwrap();
		let dst_path = dst.join(entry.file_name());
		if entry.file_type().unwrap().is_dir() {
			copy_dir(&entry.path(), &dst_path);
		} else {
			fs::copy(entry.path(), &dst_path).unwrap();
		}
	}
}

use glam::UVec2;
use std::path::{Path, PathBuf};

pub const DEFAULT_EDITOR_URL: &str = "http://localhost:8080/";
pub const DEFAULT_WINDOW_SIZE: UVec2 = UVec2::new(1600, 1000);
pub const BROWSERS_PATH_VARIABLE: &str = "PLAYWRIGHT_BROWSERS_PATH";

pub fn tool_directory() -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn frontend_directory() -> PathBuf {
	tool_directory().join("..").join("..").join("frontend")
}

pub fn playwright_directory() -> PathBuf {
	tool_directory().join("playwright")
}

/// The browsers the environment provides.
pub fn provided_browsers_directory() -> Option<PathBuf> {
	std::env::var_os(BROWSERS_PATH_VARIABLE).filter(|path| !path.is_empty()).map(PathBuf::from)
}

// Otherwise the browser is downloaded here, so the driver is isolated from any browser, profile, or history already on the machine
pub fn browsers_directory() -> PathBuf {
	provided_browsers_directory().unwrap_or_else(|| tool_directory().join(".browsers"))
}

pub fn session_directory() -> PathBuf {
	tool_directory().join(".session")
}

/// Creates the session folder, which holds the session's token, so on Unix only its owner may read it.
pub fn create_session_directory() -> Result<(), String> {
	let directory = session_directory();
	std::fs::create_dir_all(&directory).map_err(|error| format!("Failed to create the session folder: {error}"))?;

	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;
		std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).map_err(|error| format!("Failed to restrict the session folder: {error}"))?;
	}

	Ok(())
}

pub fn session_file() -> PathBuf {
	session_directory().join("session.json")
}

pub fn session_lock_file() -> PathBuf {
	session_directory().join("session.lock")
}

pub fn session_log_file() -> PathBuf {
	session_directory().join("session.log")
}

pub fn dev_server_file() -> PathBuf {
	session_directory().join("dev-server.json")
}

pub fn dev_server_log_file() -> PathBuf {
	session_directory().join("dev-server.log")
}

pub fn output_directory() -> PathBuf {
	tool_directory().join("output")
}

/// Resolves a path given on the command line or in a scenario, where relative paths land in the output directory.
pub fn output_path(path: &Path) -> PathBuf {
	// Rebuilding the path from its components gives it the platform's own separators
	let path = path.components().collect::<PathBuf>();
	if path.is_absolute() { path } else { output_directory().join(path) }
}

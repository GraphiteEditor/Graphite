use crate::cmd::prelude::*;
use crate::*;
use std::path::{Path, PathBuf};

const WRAPPER_CRATE: &str = "graphite-wasm-wrapper";
const WASM_TARGET: &str = "wasm32-unknown-unknown";
const OUT_NAME: &str = "graphite_wasm_wrapper";
const NODE_MODULES_LOCKED_PREFIX: &str = "node_modules.locked-";

pub fn frontend_dir() -> PathBuf {
	workspace_dir().join("frontend")
}

fn pkg_dir(native: bool) -> PathBuf {
	frontend_dir().join("wrapper").join(if native { "pkg-native" } else { "pkg" })
}

fn wasm_glue_path(native: bool) -> PathBuf {
	pkg_dir(native).join(format!("{OUT_NAME}.js"))
}

pub fn setup() -> Result<(), Error> {
	let frontend = frontend_dir();
	let node_modules = frontend.join("node_modules");
	let timestamp_path = node_modules.join(".install-timestamp");

	sweep_locked_leftovers(frontend.clone());

	let mtime = |p: PathBuf| std::fs::metadata(p).and_then(|m| m.modified()).ok();

	if let Some(install_time) = mtime(timestamp_path.clone())
		&& let Some(package_json_time) = mtime(frontend.join("package.json"))
		&& let Some(package_lock_json_time) = mtime(frontend.join("package-lock.json"))
		&& install_time >= package_json_time
		&& install_time >= package_lock_json_time
	{
		return Ok(());
	}

	eprintln!("Installing npm packages...");
	let install = || utils::npm(["ci", "--include=dev", "--prefer-offline", "--no-audit", "--no-fund"]).dir(&frontend);

	// The first attempt's output is captured, keeping npm's error dump off the screen when the failure gets recovered below
	if !install().output_unchecked()?.status.success() {
		eprintln!("Failed to install npm packages. Clearing `frontend/node_modules` and retrying...");
		force_remove_node_modules(&node_modules);

		// The retry streams live, so a real failure shows npm's errors in full, right above the banner
		if let Err(e) = install().run() {
			eprintln!("\n\n--------------------> Failed to install npm packages, even after clearing `frontend/node_modules`. Check npm's output above for the cause.\n");
			return Err(e);
		}
	}

	std::fs::write(&timestamp_path, "").map_err(|e| Error::Io(e, format!("writing '{}'", timestamp_path.display())))?;
	eprintln!("Finished installing npm packages.");
	Ok(())
}

// Clears `node_modules` for a fresh install. Windows refuses to unlink a file while a running process has it memory-mapped
// (typically a native module held by an orphaned Node.js instance), but renaming still works. So a failed delete renames
// the directory to a sibling and deletes what it can of that in the background; anything still locked is moved and git-ignored.
fn force_remove_node_modules(node_modules: &Path) {
	match std::fs::remove_dir_all(node_modules) {
		Ok(()) => return,
		Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
		Err(_) => {}
	}

	let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|elapsed| elapsed.as_nanos()).unwrap_or(0);
	let relocated = node_modules.with_file_name(format!("{NODE_MODULES_LOCKED_PREFIX}{}-{nanos}", std::process::id()));

	if std::fs::rename(node_modules, &relocated).is_ok() {
		std::thread::spawn(move || best_effort_remove_dir_all(&relocated));
	} else {
		eprintln!("warning: could not remove or relocate `frontend/node_modules`");
	}
}

// Silently sweeps `node_modules.locked-*` leftovers from previous runs, whose locks are potentially gone by now (such as after a reboot)
fn sweep_locked_leftovers(frontend: PathBuf) {
	std::thread::spawn(move || {
		let Ok(entries) = std::fs::read_dir(frontend) else { return };

		for entry in entries.flatten() {
			if entry.file_name().to_string_lossy().starts_with(NODE_MODULES_LOCKED_PREFIX) {
				best_effort_remove_dir_all(&entry.path());
			}
		}
	});
}

// Recursively deletes everything it can inside `dir` and then `dir` itself, skipping (not aborting on) locked entries
fn best_effort_remove_dir_all(dir: &Path) {
	if let Ok(entries) = std::fs::read_dir(dir) {
		for entry in entries.flatten() {
			let path = entry.path();
			if entry.file_type().map(|file_type| file_type.is_dir()).unwrap_or(false) {
				best_effort_remove_dir_all(&path);
			} else if std::fs::remove_file(&path).is_err() {
				// A symlink or junction to a directory must be removed as a directory
				let _ = std::fs::remove_dir(&path);
			}
		}
	}

	let _ = std::fs::remove_dir(dir);
}

pub fn build_wasm(release: bool, native: bool) -> Result<(), Error> {
	sequence(build_wasm_steps(release, native)).wait();
	Ok(())
}

pub fn build_wasm_steps(release: bool, native: bool) -> Vec<Expression> {
	let wasm_artifact = target_dir().join(WASM_TARGET).join(if release { "release" } else { "debug" }).join(format!("{OUT_NAME}.wasm"));
	let pkg_dir = pkg_dir(native);

	let mut steps = vec![
		cmd!("cargo", "build", "--lib", "--package", WRAPPER_CRATE, "--target", WASM_TARGET)
			.arg_if(release, "--release")
			.args_if(native, ["--no-default-features", "--features", "native"])
			.dir(workspace_dir())
			.before_spawn(move |_| {
				if is_build_corrupted(wasm_glue_path(native)) {
					clean_wasm();
				}
				Ok(())
			}),
		cmd!("wasm-bindgen", "--target", "web", "--out-name", OUT_NAME, "--out-dir", &pkg_dir, &wasm_artifact)
			.arg_if(release, "--no-demangle")
			.arg_if(!release, "--debug"),
	];

	if release {
		let wasm_file = pkg_dir.join(format!("{OUT_NAME}_bg.wasm"));
		// `-O3` favors runtime speed over binary size and `-g` preserves the name section,
		// which the panic hook reads at runtime to spot node-graph panics (see wrapper `lib.rs`).
		steps.push(cmd!("wasm-opt", "-O3", "-g", &wasm_file, "-o", &wasm_file));
	}

	steps
}

pub fn clean_wasm() -> bool {
	let wasm_target_dir = target_dir().join(WASM_TARGET);
	eprintln!("The Wasm build emitted undefined `env` imports, a sign of corrupt incremental artifacts (typically from an interrupted build).");
	eprintln!("Fixing by wiping `{}` and rebuilding...", wasm_target_dir.display());
	match std::fs::remove_dir_all(&wasm_target_dir) {
		Ok(()) => {}
		Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
		Err(e) => eprintln!("warning: could not fully clean `{}`: {e}", wasm_target_dir.display()),
	}
	true
}

pub fn is_build_corrupted(path: PathBuf) -> bool {
	let Ok(js) = std::fs::read_to_string(&path) else {
		return false;
	};
	js.contains("from \"env\"") || js.contains("from 'env'")
}

pub fn vite() -> Expression {
	utils::node_bin("vite/bin/vite.js").dir(frontend_dir()).env("CARGO_TARGET_DIR", target_dir())
}

pub fn watch(release: bool) -> Result<(), Error> {
	use crate::cmd::prelude::*;

	setup()?;
	build_wasm(release, false)?;

	let vite = vite().env("FORCE_COLOR", "1").env("CARGO_TERM_COLOR", "always");
	let rust = utils::internal("watch")
		.arg_if(release, "release")
		.dir(workspace_dir())
		.env("CARGO_TARGET_DIR", target_dir())
		.env("CARGO_TERM_COLOR", "always");

	supervise([("VITE", TerminalColor::Magenta, vite), ("RUST", TerminalColor::Blue, rust)])
}

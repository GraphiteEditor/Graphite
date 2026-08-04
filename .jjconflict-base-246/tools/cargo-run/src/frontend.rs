use crate::cmd::prelude::*;
use crate::*;
use std::path::PathBuf;

const WRAPPER_CRATE: &str = "graphite-wasm-wrapper";
const WASM_TARGET: &str = "wasm32-unknown-unknown";
const OUT_NAME: &str = "graphite_wasm_wrapper";

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
	let install = || utils::npm(["ci", "--include=dev", "--prefer-offline", "--no-audit", "--no-fund"]).dir(&frontend).run();
	if install().is_err() {
		eprintln!("Failed to install npm packages. Wiping `frontend/node_modules` and retrying...");
		let _ = std::fs::remove_dir_all(&node_modules);
		install()?;
	}

	std::fs::write(&timestamp_path, "").map_err(|e| Error::Io(e, format!("writing '{}'", timestamp_path.display())))?;
	eprintln!("Finished installing npm packages.");
	Ok(())
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

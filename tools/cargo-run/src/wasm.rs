use crate::Error;
use std::path::PathBuf;
use std::process::Command;

const WRAPPER_CRATE: &str = "graphite-wasm-wrapper";
const WASM_TARGET: &str = "wasm32-unknown-unknown";
const OUT_NAME: &str = "graphite_wasm_wrapper";
const WASM_OPT_ARGS: &[&str] = &["-Os", "-g"];

/// Builds the editor's Wasm module (`/frontend/wrapper`) by running `cargo build`, then `wasm-bindgen` to generate the JS/TS bindings
/// and final `.wasm` binary in `/frontend/wrapper/pkg`, then (for release builds only) `wasm-opt` to optimize the binary for size.
pub fn build(release: bool, native: bool) -> Result<(), Error> {
	let workspace_dir = PathBuf::from(env!("CARGO_WORKSPACE_DIR"));

	// Ensure the Wasm compilation target is installed (quietly skipped where `rustup` isn't used, like Nix)
	ensure_wasm_target_installed();

	// Compile the wrapper crate to Wasm
	let mut cargo_build = Command::new("cargo");
	cargo_build.current_dir(&workspace_dir);
	cargo_build.args(cargo_build_args(release, native));
	run_command(cargo_build)?;

	// Generate the JS/TS bindings and the processed `.wasm` binary in `/frontend/wrapper/pkg`
	let wasm_artifact = target_dir().join(WASM_TARGET).join(if release { "release" } else { "debug" }).join(format!("{OUT_NAME}.wasm"));
	let pkg_dir = workspace_dir.join("frontend").join("wrapper").join("pkg");

	let mut wasm_bindgen = Command::new("wasm-bindgen");
	wasm_bindgen.args(wasm_bindgen_args(release));
	wasm_bindgen.arg("--out-dir").arg(&pkg_dir);
	wasm_bindgen.arg(&wasm_artifact);
	run_command(wasm_bindgen)?;

	// Optimize the binary for size, keeping the name section (`-g`) for usable stack traces and profiling
	if release {
		let wasm_file = pkg_dir.join(format!("{OUT_NAME}_bg.wasm"));
		let optimized_wasm_file = pkg_dir.join(format!("{OUT_NAME}_bg.opt.wasm"));

		let mut wasm_opt = Command::new("wasm-opt");
		wasm_opt.args(WASM_OPT_ARGS).arg(&wasm_file).arg("-o").arg(&optimized_wasm_file);
		run_command(wasm_opt)?;

		std::fs::rename(&optimized_wasm_file, &wasm_file).map_err(|e| Error::Io(e, "Failed to move the wasm-opt output into place".into()))?;
	}

	Ok(())
}

/// Renders the same rebuild steps as [`build`] into shell commands for the dev server's `cargo watch` loop, which
/// runs them in sequence from the wrapper crate's directory on every Rust source change. Invoking the tools directly
/// keeps this tool's own binary out of the loop (rebuilding a running executable fails on Windows). The commands must
/// stay free of quotes and spaces in paths (hence the relative paths), since each one is wrapped in quotes that must
/// survive both `cmd.exe` and `sh` on the way to `cargo watch`.
pub fn watch_shell_commands(release: bool) -> Vec<String> {
	let profile_dir = if release { "release" } else { "debug" };

	// Expressed relative to the wrapper directory when inside the workspace (always true unless `CARGO_TARGET_DIR`
	// points elsewhere), avoiding absolute path prefixes which may contain spaces
	let workspace_dir = PathBuf::from(env!("CARGO_WORKSPACE_DIR"));
	let target_dir = match target_dir().strip_prefix(&workspace_dir) {
		Ok(within_workspace) => format!("../../{}", within_workspace.display()),
		Err(_) => target_dir().display().to_string(),
	};

	let mut steps = vec![
		format!("cargo {} --color=always", cargo_build_args(release, false).join(" ")),
		format!(
			"wasm-bindgen {} --out-dir pkg {target_dir}/{WASM_TARGET}/{profile_dir}/{OUT_NAME}.wasm",
			wasm_bindgen_args(release).join(" ")
		),
	];
	if release {
		// Optimized in place, which is safe because wasm-opt fully reads the input before writing the output
		steps.push(format!("wasm-opt {} pkg/{OUT_NAME}_bg.wasm -o pkg/{OUT_NAME}_bg.wasm", WASM_OPT_ARGS.join(" ")));
	}
	steps
}

/// The `cargo build` arguments shared by [`build`] and [`watch_shell_commands`].
fn cargo_build_args(release: bool, native: bool) -> Vec<&'static str> {
	let mut args = vec!["build", "--lib", "--package", WRAPPER_CRATE, "--target", WASM_TARGET];
	if release {
		args.push("--release");
	}
	if native {
		args.extend(["--no-default-features", "--features", "native"]);
	}
	args
}

/// The `wasm-bindgen` arguments shared by [`build`] and [`watch_shell_commands`], except the input/output paths which differ by context.
fn wasm_bindgen_args(release: bool) -> Vec<&'static str> {
	let mut args = vec!["--target", "web", "--out-name", OUT_NAME];
	if release {
		// Don't demangle Rust symbol names in the name section, saving some space in production builds
		args.push("--no-demangle");
	} else {
		// Include runtime assertions in the generated JS glue code to catch incorrect usage during development
		args.push("--debug");
	}
	args
}

/// The workspace's cargo target directory, honoring the `CARGO_TARGET_DIR` environment variable.
pub(crate) fn target_dir() -> PathBuf {
	let workspace_dir = PathBuf::from(env!("CARGO_WORKSPACE_DIR"));
	match std::env::var_os("CARGO_TARGET_DIR") {
		// Joining handles both forms: an absolute path replaces the workspace prefix entirely, while a relative path
		// is resolved against the workspace root, matching how cargo resolves it when invoked from there
		Some(custom_dir) => workspace_dir.join(custom_dir),
		None => workspace_dir.join("target"),
	}
}

/// Installs the Wasm target through rustup if it's missing. Any failure is ignored because rustup may not exist in
/// environments that preinstall the target (such as Nix); an actual missing target surfaces as a `cargo build` error instead.
fn ensure_wasm_target_installed() {
	let Ok(output) = Command::new("rustup").args(["target", "list", "--installed"]).output() else {
		return;
	};
	if !output.status.success() || String::from_utf8_lossy(&output.stdout).lines().any(|line| line.trim() == WASM_TARGET) {
		return;
	}
	let _ = Command::new("rustup").args(["target", "add", WASM_TARGET]).status();
}

pub(crate) fn run_command(mut command: Command) -> Result<(), Error> {
	let command_str = format!("{command:?}");
	let exit_code = command
		.spawn()
		.map_err(|e| Error::Io(e, format!("Failed to spawn command {command_str}")))?
		.wait()
		.map_err(|e| Error::Io(e, format!("Failed to wait for command {command_str}")))?;
	if !exit_code.success() {
		return Err(Error::Command(command_str, exit_code));
	}
	Ok(())
}

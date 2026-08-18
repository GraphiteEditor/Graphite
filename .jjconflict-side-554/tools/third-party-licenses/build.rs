fn main() {
	println!("cargo:rerun-if-changed=src");

	println!("cargo:rerun-if-env-changed=DEP_CEF_DLL_WRAPPER_CEF_DIR");
	if let Ok(cef_dir) = std::env::var("DEP_CEF_DLL_WRAPPER_CEF_DIR") {
		println!("cargo:rustc-env=CEF_PATH={cef_dir}");
	}

	let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
	if std::env::var("CARGO_FEATURE_DESKTOP").is_ok() {
		let _ = std::fs::remove_file(manifest_dir.join("desktop.hash"));
	} else {
		let _ = std::fs::remove_file(manifest_dir.join("web.hash"));
	}
}

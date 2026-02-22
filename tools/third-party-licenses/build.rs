fn main() {
	println!("cargo:rerun-if-env-changed=DEP_CEF_DLL_WRAPPER_CEF_DIR");
	let cef_dir = std::env::var("DEP_CEF_DLL_WRAPPER_CEF_DIR").unwrap();
	println!("cargo:rustc-env=CEF_PATH={cef_dir}");
}

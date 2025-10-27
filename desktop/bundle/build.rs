fn main() {
	println!("cargo:rerun-if-env-changed=CARGO_PROFILE");
	println!("cargo:rerun-if-env-changed=PROFILE");
	let profile = std::env::var("CARGO_PROFILE").or_else(|_| std::env::var("PROFILE")).unwrap();
	println!("cargo:rustc-env=CARGO_PROFILE={profile}");

	println!("cargo:rerun-if-env-changed=DEP_CEF_DLL_WRAPPER_CEF_DIR");
	let cef_dir = std::env::var("DEP_CEF_DLL_WRAPPER_CEF_DIR").unwrap();
	println!("cargo:rustc-env=CEF_PATH={cef_dir}");
}

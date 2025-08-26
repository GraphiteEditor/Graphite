const RESOURCES: &str = "../frontend/dist";

fn main() {
	cfg_embedded_resources(RESOURCES);
}

// Check if the directory `resources` exists and sets the embedded_resources cfg accordingly
// `resources` is made available via the `RESOURCES` environment variable
// Has no effect if the embedded_resources cargo feature is disabled
fn cfg_embedded_resources(resources: &str) {
	println!("cargo:rerun-if-env-changed=CARGO_FEATURE_EMBEDDED_RESOURCES");
	if std::env::var("CARGO_FEATURE_EMBEDDED_RESOURCES").is_err() {
		return;
	}
	println!("cargo:rerun-if-changed={resources}");
	if let Ok(resources) = std::path::Path::new(resources).canonicalize()
		&& resources.exists()
	{
		println!("cargo:rustc-cfg=embedded_resources");
		println!("cargo:rustc-env=EMBEDDED_RESOURCES={}", resources.to_string_lossy());
	} else {
		println!("cargo:warning=Resource directory does not exist. Resources will not be embedded. Did you forget to build the frontend?");
	}
}

const RESOURCES: &str = "../../frontend/dist";

// Check if the directory `RESOURCES` exists and sets the embedded_resources cfg accordingly
// Absolute path of `RESOURCES` available via the `EMBEDDED_RESOURCES` environment variable
fn main() {
	let crate_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());

	println!("cargo:rerun-if-changed={RESOURCES}");
	if let Ok(resources) = crate_dir.join(RESOURCES).canonicalize()
		&& resources.exists()
	{
		println!("cargo:rustc-cfg=embedded_resources");
		println!("cargo:rustc-env=EMBEDDED_RESOURCES={}", resources.to_string_lossy());
	} else {
		println!("cargo:warning=Resource directory does not exist. Resources will not be embedded. Did you forget to build the frontend?");
	}
}

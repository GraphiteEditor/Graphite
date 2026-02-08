const EMBEDDED_RESOURCES_ENV: &str = "EMBEDDED_RESOURCES";
const DEFAULT_RESOURCES_DIR: &str = "../../frontend/dist";

fn main() {
	let mut embedded_resources: Option<String> = None;

	println!("cargo:rerun-if-env-changed={EMBEDDED_RESOURCES_ENV}");
	if let Ok(embedded_resources_env) = std::env::var(EMBEDDED_RESOURCES_ENV)
		&& std::path::PathBuf::from(&embedded_resources_env).exists()
	{
		embedded_resources = Some(embedded_resources_env);
	}

	if embedded_resources.is_none() {
		// Check if the directory `DEFAULT_RESOURCES_DIR` exists and sets the embedded_resources cfg accordingly
		// Absolute path of `DEFAULT_RESOURCES_DIR` available via the `EMBEDDED_RESOURCES` environment variable
		let crate_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
		println!("cargo:rerun-if-changed={DEFAULT_RESOURCES_DIR}");
		if let Ok(resources) = crate_dir.join(DEFAULT_RESOURCES_DIR).canonicalize()
			&& resources.exists()
		{
			embedded_resources = Some(resources.to_string_lossy().to_string());
		}
	}

	if let Some(embedded_resources) = embedded_resources {
		println!("cargo:rustc-cfg=embedded_resources");
		println!("cargo:rustc-env={EMBEDDED_RESOURCES_ENV}={embedded_resources}");
	} else {
		println!("cargo:warning=Resource directory does not exist. Resources will not be embedded. Did you forget to build the frontend?");
	}
}

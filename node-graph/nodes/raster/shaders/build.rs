use cargo_gpu::InstalledBackend;
use cargo_gpu::spirv_builder::{MetadataPrintout, SpirvMetadata};
use std::path::PathBuf;

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
	env_logger::builder().filter_level(log::LevelFilter::Debug).init();

	// Skip building the shader if they are provided externally
	println!("cargo:rerun-if-env-changed=GRAPHENE_RASTER_NODES_SHADER_PATH");
	if !std::env::var("GRAPHENE_RASTER_NODES_SHADER_PATH").unwrap_or_default().is_empty() {
		return Ok(());
	}

	// Allows overriding the PATH to inject the rust-gpu rust toolchain when building the rest of the project with stable rustc.
	// Used in nix shell. Do not remove without checking with developers using nix.
	println!("cargo:rerun-if-env-changed=RUST_GPU_PATH_OVERRIDE");
	if let Ok(path_override) = std::env::var("RUST_GPU_PATH_OVERRIDE") {
		let current_path = std::env::var("PATH").unwrap_or_default();
		let new_path = format!("{path_override}:{current_path}");
		// SAFETY: Build script is single-threaded therefore this cannot lead to undefined behavior.
		unsafe {
			std::env::set_var("PATH", &new_path);
		}
	}

	let shader_crate = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/entrypoint"));

	println!("cargo:rerun-if-env-changed=RUSTC_CODEGEN_SPIRV_PATH");
	let rustc_codegen_spirv_path = std::env::var("RUSTC_CODEGEN_SPIRV_PATH").unwrap_or_default();
	let backend = if rustc_codegen_spirv_path.is_empty() {
		// install the toolchain and build the `rustc_codegen_spirv` codegen backend with it
		cargo_gpu::Install::from_shader_crate(shader_crate.clone()).run()?
	} else {
		// use the `RUSTC_CODEGEN_SPIRV` environment variable to find the codegen backend
		let mut backend = InstalledBackend::default();
		backend.rustc_codegen_spirv_location = PathBuf::from(rustc_codegen_spirv_path);
		backend.toolchain_channel = "nightly".to_string();
		backend.target_spec_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
		backend
	};

	// build the shader crate
	let mut builder = backend.to_spirv_builder(shader_crate, "spirv-unknown-naga-wgsl");
	builder.print_metadata = MetadataPrintout::DependencyOnly;
	builder.spirv_metadata = SpirvMetadata::Full;
	let wgsl_result = builder.build()?;
	let path_to_spv = wgsl_result.module.unwrap_single();

	// needs to be fixed upstream
	let path_to_wgsl = path_to_spv.with_extension("wgsl");

	println!("cargo::rustc-env=GRAPHENE_RASTER_NODES_SHADER_PATH={}", path_to_wgsl.display());
	Ok(())
}

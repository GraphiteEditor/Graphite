use cargo_gpu::spirv_builder::{MetadataPrintout, SpirvMetadata};
use std::path::PathBuf;

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
	env_logger::builder().init();

	let shader_crate = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/.."));

	let rustc_codegen_spirv_path = std::env::var("RUSTC_CODEGEN_SPIRV_PATH").unwrap_or_default();
	let mut builder = if rustc_codegen_spirv_path.is_empty() {
		// install the toolchain and build the `rustc_codegen_spirv` codegen backend with it
		cargo_gpu::Install::from_shader_crate(shader_crate.clone())
			.run()?
			.to_spirv_builder(shader_crate, "spirv-unknown-naga-wgsl")
	} else {
		// use the `RUSTC_CODEGEN_SPIRV` environment variable to find the codegen backend
		let mut builder = cargo_gpu::spirv_builder::SpirvBuilder::new(shader_crate.clone(), "spirv-unknown-naga-wgsl");
		builder.rustc_codegen_spirv_location = Some(PathBuf::from(rustc_codegen_spirv_path));
		builder.toolchain_overwrite = Some("nightly".to_string());
		builder.path_to_target_spec = Some(PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/spirv-unknown-naga-wgsl.json")));
		builder
	};

	// build the shader crate
	builder.print_metadata = MetadataPrintout::DependencyOnly;
	builder.spirv_metadata = SpirvMetadata::Full;
	builder.shader_crate_features.default_features = false;
	let wgsl_result = builder.build()?;
	let path_to_spv = wgsl_result.module.unwrap_single();

	// needs to be fixed upstream
	let path_to_wgsl = path_to_spv.with_extension("wgsl");

	println!("cargo::rustc-env=WGSL_SHADER_PATH={}", path_to_wgsl.display());
	Ok(())
}

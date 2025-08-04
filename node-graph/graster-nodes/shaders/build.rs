use cargo_gpu::spirv_builder::{MetadataPrintout, SpirvMetadata};
use std::path::PathBuf;

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
	env_logger::builder().init();

	let shader_crate = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/.."));

	// install the toolchain and build the `rustc_codegen_spirv` codegen backend with it
	let backend = cargo_gpu::Install::from_shader_crate(shader_crate.clone()).run()?;

	// build the shader crate
	let mut builder = backend.to_spirv_builder(shader_crate, "spirv-unknown-naga-wgsl");
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

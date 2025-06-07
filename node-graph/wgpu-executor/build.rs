use cargo_gpu::CompileResultNagaExt;
use cargo_gpu::naga::back::wgsl::WriterFlags;
use cargo_gpu::naga::valid::Capabilities;
use cargo_gpu::spirv_builder::{MetadataPrintout, SpirvMetadata};
use std::path::PathBuf;

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
	env_logger::builder().init();

	let shader_crate = PathBuf::from("../gcore-shader");

	// install the toolchain and build the `rustc_codegen_spirv` codegen backend with it
	let backend = cargo_gpu::Install::from_shader_crate(shader_crate.clone()).run()?;

	// build the shader crate
	let mut builder = backend.to_spirv_builder(shader_crate, "spirv-unknown-vulkan1.2");
	builder.print_metadata = MetadataPrintout::DependencyOnly;
	builder.spirv_metadata = SpirvMetadata::Full;
	builder.shader_crate_features.default_features = false;
	builder.shader_crate_features.features = vec![String::from("gpu")];
	let spv_result = builder.build()?;

	// transpile the spv binaries to wgsl
	let wgsl_result = spv_result.naga_transpile(Capabilities::empty())?.to_wgsl(WriterFlags::empty())?;
	let path_to_wgsl = wgsl_result.module.unwrap_single();

	// emit path to wgsl into env var, used in `quad.rs` like this:
	// > include_str!(env!("WGSL_SHADER_PATH"))
	println!("cargo::rustc-env=WGSL_SHADER_PATH={}", path_to_wgsl.display());

	// you could also generate some rust source code into the `std::env::var("OUT_DIR")` dir
	// and use `include!(concat!(env!("OUT_DIR"), "/shader_symbols.rs"));` to include it
	Ok(())
}

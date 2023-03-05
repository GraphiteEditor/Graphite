use std::path::Path;

use graph_craft::proto::*;
use tera::Context;

fn create_cargo_toml(metadata: &Metadata) -> Result<String, tera::Error> {
	let mut tera = tera::Tera::default();
	tera.add_raw_template("cargo_toml", include_str!("templates/Cargo-template.toml"))?;
	let mut context = Context::new();
	context.insert("name", &metadata.name);
	context.insert("authors", &metadata.authors);
	context.insert("gcore_path", &format!("{}{}", env!("CARGO_MANIFEST_DIR"), "/../gcore"));
	tera.render("cargo_toml", &context)
}

pub struct Metadata {
	name: String,
	authors: Vec<String>,
}

impl Metadata {
	pub fn new(name: String, authors: Vec<String>) -> Self {
		Self { name, authors }
	}
}

pub fn create_files(matadata: &Metadata, network: &ProtoNetwork, compile_dir: &Path, input_type: &str, output_type: &str) -> anyhow::Result<()> {
	let src = compile_dir.join("src");
	let cargo_file = compile_dir.join("Cargo.toml");
	let cargo_toml = create_cargo_toml(matadata)?;
	std::fs::write(cargo_file, cargo_toml)?;

	let toolchain_file = compile_dir.join("rust-toolchain.toml");
	let toolchain = include_str!("templates/rust-toolchain.toml");
	std::fs::write(toolchain_file, toolchain)?;

	// create src dir
	match std::fs::create_dir(&src) {
		Ok(_) => {}
		Err(e) => {
			if e.kind() != std::io::ErrorKind::AlreadyExists {
				return Err(e.into());
			}
		}
	}
	let lib = src.join("lib.rs");
	let shader = serialize_gpu(network, input_type, output_type)?;
	println!("{}", shader);
	std::fs::write(lib, shader)?;
	Ok(())
}

pub fn serialize_gpu(network: &ProtoNetwork, input_type: &str, output_type: &str) -> anyhow::Result<String> {
	assert_eq!(network.inputs.len(), 1);
	fn nid(id: &u64) -> String {
		format!("n{id}")
	}

	let mut nodes = Vec::new();
	#[derive(serde::Serialize)]
	struct Node {
		id: String,
		fqn: String,
		args: Vec<String>,
	}
	for (ref id, node) in network.nodes.iter() {
		let fqn = &node.identifier.name;
		let id = nid(id);

		nodes.push(Node {
			id,
			fqn: fqn.to_string().split("<").next().unwrap().to_owned(),
			args: node.construction_args.new_function_args(),
		});
	}

	let template = include_str!("templates/spirv-template.rs");
	let mut tera = tera::Tera::default();
	tera.add_raw_template("spirv", template)?;
	let mut context = Context::new();
	context.insert("input_type", &input_type);
	context.insert("output_type", &output_type);
	context.insert("nodes", &nodes);
	context.insert("last_node", &nid(&network.output));
	context.insert("compute_threads", &64);
	Ok(tera.render("spirv", &context)?)
}

use spirv_builder::{MetadataPrintout, SpirvBuilder, SpirvMetadata};
pub fn compile(dir: &Path) -> Result<spirv_builder::CompileResult, spirv_builder::SpirvBuilderError> {
	dbg!(&dir);
	let result = SpirvBuilder::new(dir, "spirv-unknown-spv1.5")
		.print_metadata(MetadataPrintout::DependencyOnly)
		.multimodule(false)
		.preserve_bindings(true)
		.release(true)
		//.relax_struct_store(true)
		//.relax_block_layout(true)
		.spirv_metadata(SpirvMetadata::Full)
		.build()?;

	Ok(result)
}

#[cfg(test)]
mod test {

	#[test]
	fn test_create_cargo_toml() {
		let cargo_toml = super::create_cargo_toml(&super::Metadata {
			name: "project".to_owned(),
			authors: vec!["Example <john.smith@example.com>".to_owned(), "smith.john@example.com".to_owned()],
		});
		let cargo_toml = cargo_toml.expect("failed to build carog toml template");
		let lines = cargo_toml.split('\n').collect::<Vec<_>>();
		let cargo_toml = lines[..lines.len() - 2].join("\n");
		let reference = r#"[package]
name = "project-node"
version = "0.1.0"
authors = ["Example <john.smith@example.com>", "smith.john@example.com", ]
edition = "2021"
license = "MIT OR Apache-2.0"
publish = false

[lib]
crate-type = ["dylib", "lib"]

[patch.crates-io]
libm = { git = "https://github.com/rust-lang/libm", tag = "0.2.5" }

[dependencies]
spirv-std = { git = "https://github.com/EmbarkStudios/rust-gpu" , features= ["glam"]}"#;

		assert_eq!(cargo_toml, reference);
	}
}

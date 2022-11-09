use crate::proto::*;
use tempdir::TempDir;
use tera::Context;

fn create_tempdir() -> std::io::Result<TempDir> {
	TempDir::new("graphite_compile")
}

fn create_cargo_toml(metadata: &Metadata) -> String {
	let mut tera = tera::Tera::default();
	tera.add_raw_template("cargo_toml", include_str!("templates/Cargo-template.toml")).unwrap();
	let mut context = Context::new();
	context.insert("name", &metadata.name);
	context.insert("authors", &metadata.authors);
	tera.render("cargo_toml", &context).unwrap()
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

pub fn create_files(matadata: &Metadata, network: &ProtoNetwork) -> std::io::Result<TempDir> {
	let dir = create_tempdir()?;
	let cargo_toml = create_cargo_toml(matadata);
	//std::fs::write(dir.path().join("Cargo.toml"), cargo_toml)?;
	std::fs::write("/tmp/graphite_compile/Cargo.toml", cargo_toml)?;
	let src = dir.path().join("src");
	//std::fs::create_dir(&src)?;
	//std::fs::create_dir("/tmp/graphite_compile/src")?;
	let lib = src.join("lib.rs");
	let shader = serialize_gpu(network, "u32".into(), "u32".into());
	//std::fs::write(lib, shader)?;
	std::fs::write("/tmp/graphite_compile/src/lib.rs", shader)?;
	Ok(dir)
}

pub fn serialize_gpu(network: &ProtoNetwork, input_type: String, output_type: String) -> String {
	assert_eq!(network.inputs.len(), 1);
	/*let input = &network.nodes[network.inputs[0] as usize].1;
	let output = &network.nodes[network.output as usize].1;
	let input_type = format!("{}::Input", input.identifier.fully_qualified_name());
	let output_type = format!("{}::Output", output.identifier.fully_qualified_name());
	*/

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
		let fqn = node.identifier.name;
		let id = nid(id);

		nodes.push(Node {
			id,
			fqn: fqn.to_owned(),
			args: node.construction_args.new_function_args(),
		});
	}

	let template = include_str!("templates/spirv-template.rs");
	let mut tera = tera::Tera::default();
	tera.add_raw_template("spirv", template).unwrap();
	let mut context = Context::new();
	nodes.reverse();
	context.insert("input_type", &input_type);
	context.insert("output_type", &output_type);
	context.insert("nodes", &nodes);
	context.insert("compute_threads", &64);
	tera.render("spirv", &context).unwrap()
}

use spirv_builder::{MetadataPrintout, SpirvBuilder, SpirvMetadata};
pub fn compile(dir: &TempDir) -> Result<spirv_builder::CompileResult, spirv_builder::SpirvBuilderError> {
	println!("using hardcoded path");
	//let result = SpirvBuilder::new(dir.path().to_str().unwrap(), "spirv-unknown-spv1.5")
	let result = SpirvBuilder::new("/tmp/graphite_compile", "spirv-unknown-spv1.5")
        .print_metadata(MetadataPrintout::DependencyOnly)
        .multimodule(false)
        .preserve_bindings(true)
        .release(false)
        //.relax_struct_store(true)
        //.relax_block_layout(true)
        .spirv_metadata(SpirvMetadata::Full)
        .build()?;

	println!("{:#?}", result);
	Ok(result)
}

#[cfg(test)]
mod test {

	#[test]
	fn test_create_tempdir() {
		let tempdir = super::create_tempdir().unwrap();
		assert!(tempdir.path().exists());
	}

	#[test]
	fn test_create_cargo_toml() {
		let cargo_toml = super::create_cargo_toml(&super::Metadata {
			name: "project".to_owned(),
			authors: vec!["Example <john.smith@example.com>".to_owned(), "smith.john@example.com".to_owned()],
		});
		let reference = r#"
[package]
name = "project-node"
version = "0.1.0"
authors = ["Example <john.smith@example.com>", "smith.john@example.com", ]
edition = "2021"
license = "MIT OR Apache-2.0"
publish = false

[lib]
crate-type = ["dylib", "lib"]

[dependencies]
spirv-std = { path = "/home/dennis/Projects/rust/rust-gpu/crates/spirv-std" , features= ["glam"]}
graphene-core = {path = "/home/dennis/graphite/node-graph/gcore", default-features = false, features = ["gpu"]}
"#;

		assert_eq!(cargo_toml, reference);
	}
}

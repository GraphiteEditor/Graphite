use gpu_executor::{GPUConstant, ShaderIO, ShaderInput, SpirVCompiler};
use graph_craft::proto::*;
use graphene_core::Cow;

use std::path::{Path, PathBuf};
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

pub fn create_files(metadata: &Metadata, networks: &[ProtoNetwork], compile_dir: &Path, io: &ShaderIO) -> anyhow::Result<()> {
	let src = compile_dir.join("src");
	let cargo_file = compile_dir.join("Cargo.toml");
	let cargo_toml = create_cargo_toml(metadata)?;
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
	let shader = serialize_gpu(networks, io)?;
	eprintln!("{shader}");
	std::fs::write(lib, shader)?;
	Ok(())
}

fn constant_attribute(constant: &GPUConstant) -> &'static str {
	match constant {
		GPUConstant::SubGroupId => "subgroup_id",
		GPUConstant::SubGroupInvocationId => "subgroup_local_invocation_id",
		GPUConstant::SubGroupSize => todo!(),
		GPUConstant::NumSubGroups => "num_subgroups",
		GPUConstant::WorkGroupId => "workgroup_id",
		GPUConstant::WorkGroupInvocationId => "local_invocation_id",
		GPUConstant::WorkGroupSize => todo!(),
		GPUConstant::NumWorkGroups => "num_workgroups",
		GPUConstant::GlobalInvocationId => "global_invocation_id",
		GPUConstant::GlobalSize => todo!(),
	}
}

pub fn construct_argument<T: gpu_executor::GpuExecutor>(input: &ShaderInput<T>, position: u32, binding_offset: u32) -> String {
	let line = match input {
		ShaderInput::Constant(constant) => format!("#[spirv({})] i{}: {}", constant_attribute(constant), position, constant.ty()),
		ShaderInput::UniformBuffer(_, ty) => {
			format!("#[spirv(uniform, descriptor_set = 0, binding = {})] i{}: &{}", position + binding_offset, position, ty,)
		}
		ShaderInput::StorageBuffer(_, ty) | ShaderInput::ReadBackBuffer(_, ty) => {
			format!("#[spirv(storage_buffer, descriptor_set = 0, binding = {})] i{}: &[{}]", position + binding_offset, position, ty,)
		}
		ShaderInput::StorageTextureBuffer(_, ty) => {
			format!("#[spirv(storage_buffer, descriptor_set = 0, binding = {})] i{}: &mut [{}]]", position + binding_offset, position, ty,)
		}
		ShaderInput::TextureView(_, _) => {
			format!(
				"#[spirv(texture, descriptor_set = 0, binding = {})] i{}: spirv_std::image::Image2d",
				position + binding_offset,
				position,
			)
		}
		ShaderInput::TextureBuffer(_, _) => {
			panic!("Texture Buffers cannot be used as inputs use TextureView instead")
		}
		ShaderInput::OutputBuffer(_, ty) => {
			format!("#[spirv(storage_buffer, descriptor_set = 0, binding = {})] o{}: &mut[{}]", position + binding_offset, position, ty,)
		}
		ShaderInput::WorkGroupMemory(_, ty) => format!("#[spirv(workgroup_memory] i{}: {}", position, ty,),
	};
	line.replace("glam::u32::uvec3::UVec3", "spirv_std::glam::UVec3")
}

struct GpuCompiler {
	compile_dir: PathBuf,
}

impl SpirVCompiler for GpuCompiler {
	fn compile(&self, networks: &[ProtoNetwork], io: &ShaderIO) -> anyhow::Result<gpu_executor::Shader> {
		let metadata = Metadata::new("project".to_owned(), vec!["test@example.com".to_owned()]);

		create_files(&metadata, networks, &self.compile_dir, io)?;
		let result = compile(&self.compile_dir)?;

		let bytes = std::fs::read(result.module.unwrap_single())?;
		let words = bytes.chunks(4).map(|chunk| u32::from_ne_bytes(chunk.try_into().unwrap())).collect::<Vec<_>>();

		Ok(gpu_executor::Shader {
			source: Cow::Owned(words),
			name: "",
			io: io.clone(),
		})
	}
}

pub fn serialize_gpu(networks: &[ProtoNetwork], io: &ShaderIO) -> anyhow::Result<String> {
	fn nid(id: &u64) -> String {
		format!("n{id:0x}")
	}

	dbg!(&io);
	let mut inputs = io
		.inputs
		.iter()
		.filter(|x| !x.is_output())
		.enumerate()
		.map(|(i, input)| construct_argument(input, i as u32, 0))
		.collect::<Vec<_>>();
	let offset = inputs.len() as u32;

	inputs.extend(io.inputs.iter().filter(|x| x.is_output()).enumerate().map(|(i, input)| construct_argument(input, i as u32, offset)));

	let mut nodes = Vec::new();
	let mut input_nodes = Vec::new();
	let mut output_nodes = Vec::new();
	for network in networks {
		dbg!(&network);
		//assert_eq!(network.inputs.len(), io.inputs.iter().filter(|x| !x.is_output()).count());
		#[derive(serde::Serialize, Debug)]
		struct Node {
			id: String,
			index: usize,
			fqn: String,
			args: Vec<String>,
		}
		for (i, id) in network.inputs.iter().enumerate() {
			let Some((_, node)) = network.nodes.iter().find(|(i, _)| i == id) else {
				anyhow::bail!("Input node not found");
			};
			let fqn = &node.identifier.name;
			let id = nid(id);
			let node = Node {
				id: id.clone(),
				index: i + 2,
				fqn: fqn.to_string().split('<').next().unwrap().to_owned(),
				args: node.construction_args.new_function_args(),
			};
			dbg!(&node);
			if !io.inputs[i].is_output() {
				if input_nodes.iter().any(|x: &Node| x.id == id) {
					continue;
				}
				input_nodes.push(node);
			}
		}

		for (ref id, node) in network.nodes.iter() {
			if network.inputs.contains(id) {
				continue;
			}

			let fqn = &node.identifier.name;
			let id = nid(id);

			if nodes.iter().any(|x: &Node| x.id == id) {
				continue;
			}
			nodes.push(Node {
				id,
				index: 0,
				fqn: fqn.to_string().split("<").next().unwrap().to_owned(),
				args: node.construction_args.new_function_args(),
			});
		}

		let output = nid(&network.output);
		output_nodes.push(output);
	}
	dbg!(&input_nodes);

	let template = include_str!("templates/spirv-template.rs");
	let mut tera = tera::Tera::default();
	tera.add_raw_template("spirv", template)?;
	let mut context = Context::new();
	context.insert("inputs", &inputs);
	context.insert("input_nodes", &input_nodes);
	context.insert("output_nodes", &output_nodes);
	context.insert("nodes", &nodes);
	context.insert("compute_threads", "12, 8");
	Ok(tera.render("spirv", &context)?)
}

use spirv_builder::{MetadataPrintout, SpirvBuilder, SpirvMetadata};
pub fn compile(dir: &Path) -> Result<spirv_builder::CompileResult, spirv_builder::SpirvBuilderError> {
	dbg!(&dir);
	let result = SpirvBuilder::new(dir, "spirv-unknown-vulkan1.2")
		.print_metadata(MetadataPrintout::DependencyOnly)
		.multimodule(false)
		.preserve_bindings(true)
		.release(true)
		.spirv_metadata(SpirvMetadata::Full)
		//.scalar_block_layout(true)
		.relax_logical_pointer(true)
		//.capability(spirv_builder::Capability::Float64)
		//.capability(spirv_builder::Capability::VariablePointersStorageBuffer)
		.extra_arg("no-early-report-zombies")
		.extra_arg("no-infer-storage-classes")
		.extra_arg("spirt-passes=qptr")
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
		let cargo_toml = cargo_toml.expect("Failed to build cargo toml template");
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

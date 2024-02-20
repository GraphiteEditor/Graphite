use gpu_compiler_bin_wrapper::CompileRequest;
use gpu_executor::{ShaderIO, ShaderInput};
use graph_craft::concrete;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::*;
use graphene_core::raster::adjustments::BlendMode;
use graphene_core::Color;

use std::time::Duration;

fn main() {
	let client = reqwest::blocking::Client::new();

	let network = add_network();
	let compiler = graph_craft::graphene_compiler::Compiler {};
	let proto_network = compiler.compile_single(network).unwrap();

	let io = ShaderIO {
		inputs: vec![
			ShaderInput::StorageBuffer((), concrete!(Color)), // background image
			ShaderInput::StorageBuffer((), concrete!(Color)), // foreground image
			ShaderInput::StorageBuffer((), concrete!(u32)),   // width/height of the foreground image
			ShaderInput::OutputBuffer((), concrete!(Color)),
		],
		output: ShaderInput::OutputBuffer((), concrete!(Color)),
	};

	let compile_request = CompileRequest::new(vec![proto_network], vec![concrete!(Color), concrete!(Color), concrete!(u32)], vec![concrete!(Color)], io);
	let response = client
		.post("http://localhost:3000/compile/spirv")
		.timeout(Duration::from_secs(30))
		.json(&compile_request)
		.send()
		.unwrap();
	println!("response: {response:?}");
}

fn add_network() -> NodeNetwork {
	NodeNetwork {
		inputs: vec![],
		outputs: vec![NodeOutput::new(NodeId(0), 0)],
		disabled: vec![],
		previous_outputs: None,
		nodes: [DocumentNode {
			name: "Blend Image".into(),
			inputs: vec![NodeInput::Inline(InlineRust::new(
				format!(
					r#"graphene_core::raster::adjustments::BlendNode::new(
							graphene_core::value::CopiedNode::new({}),
							graphene_core::value::CopiedNode::new({}),
						).eval((
							i1[_global_index.x as usize],
							if _global_index.x < i2[2] {{
								i0[_global_index.x as usize]
							}} else {{
								Color::from_rgbaf32_unchecked(0.0, 0.0, 0.0, 0.0)
							}},
						))"#,
					TaggedValue::BlendMode(BlendMode::Normal).to_primitive_string(),
					TaggedValue::F32(1.0).to_primitive_string(),
				),
				concrete![Color],
			))],
			implementation: DocumentNodeImplementation::ProtoNode("graphene_core::value::CopiedNode".into()),
			..Default::default()
		}]
		.into_iter()
		.enumerate()
		.map(|(id, node)| (NodeId(id as u64), node))
		.collect(),
	}
}

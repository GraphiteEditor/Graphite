use crate::document::NodeNetwork;
use crate::graphene_compiler::Compiler;
use crate::proto::ProtoNetwork;

pub fn load_network(document_string: &str) -> NodeNetwork {
	let document: serde_json::Value = serde_json::from_str(document_string).expect("Failed to parse document");
	serde_json::from_value::<NodeNetwork>(document["network_interface"]["network"].clone()).expect("Failed to parse document")
}

pub fn compile_with_render_config(network: NodeNetwork) -> ProtoNetwork {
	let compiler = Compiler {};
	compiler.compile_single(network, &[concrete!(graphene_core::application_io::RenderConfig)]).unwrap()
}

pub fn load_from_name(name: &str) -> NodeNetwork {
	let content = std::fs::read(format!("../../demo-artwork/{name}.graphite")).expect("failed to read file");
	let content = std::str::from_utf8(&content).unwrap();
	load_network(content)
}

pub static DEMO_ART: [&str; 6] = ["changing-seasons", "painted-dreams", "red-dress", "valley-of-spires", "isometric-fountain", "procedural-string-lights"];

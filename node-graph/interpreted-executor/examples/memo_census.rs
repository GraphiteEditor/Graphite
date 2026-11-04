//! Temporary, uncommitted: prints the compiled proto paths from the output down to nodes matching a name.
//!
//! Usage: `cargo run --release -p interpreted-executor --example memo_census -- <document name> <identifier substring>`

use core_types::uuid::NodeId;
use graph_craft::application_io::PlatformEditorApi;
use graph_craft::document::NodeNetwork;
use graph_craft::proto::{ConstructionArgs, ProtoNetwork};
use graph_craft::util::load_network;
use graphene_std::runtime::{AsyncRuntime, Spawn, Task};
use interpreted_executor::util::wrap_network_in_scope;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

struct DummySpawner;
impl Spawn for DummySpawner {
	fn spawn(&self, _task: Task) {}
}

fn compile(network: NodeNetwork, editor_api: &Arc<PlatformEditorApi>) -> ProtoNetwork {
	let mut network = wrap_network_in_scope(network, editor_api.clone(), AsyncRuntime::new(std::sync::Arc::new(DummySpawner)).handle());
	preprocessor::Preprocessor::new().preprocess(&mut network, &|_| None).unwrap();
	graph_craft::util::compile(network)
}

fn short_identifier(identifier: &str) -> &str {
	identifier.rsplit("::").next().unwrap_or(identifier)
}

fn main() {
	let mut arguments = std::env::args().skip(1);
	let name = arguments.next().unwrap_or("changing-seasons".into());
	let target = arguments.next().unwrap_or("ScatterPoints".into());

	let editor_api = Arc::new(PlatformEditorApi::default());
	let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../demo-artwork").join(format!("{name}.graphite"));
	let network = load_network(&std::fs::read_to_string(&path).expect("Failed to read the document"));
	let proto = compile(network, &editor_api);

	let nodes: HashMap<NodeId, _> = proto.nodes.iter().map(|(id, node)| (*id, node)).collect();
	let mut counts: HashMap<&str, usize> = HashMap::new();
	for (_, node) in &proto.nodes {
		*counts.entry(short_identifier(node.identifier.as_str())).or_default() += 1;
	}
	println!("{} proto nodes, {} memoize", proto.nodes.len(), counts.get("MemoizeNode").copied().unwrap_or_default());

	// Depth-first search for every path from the output to a matching node, printing the first few
	let mut printed = 0;
	let mut stack = vec![(proto.output, vec![proto.output])];
	while let Some((id, path)) = stack.pop() {
		let node = nodes[&id];
		if short_identifier(node.identifier.as_str()).contains(&target) {
			printed += 1;
			println!("--- path {printed}");
			for step in &path {
				let node = nodes[step];
				let arguments = match &node.construction_args {
					ConstructionArgs::Nodes(inputs) => format!("{} inputs", inputs.len()),
					ConstructionArgs::Value(value) => format!("value {:?}", value.ty()),
				};
				println!("{:40} ({arguments})", short_identifier(node.identifier.as_str()));
			}
			if printed >= 3 {
				break;
			}
			continue;
		}
		if let ConstructionArgs::Nodes(inputs) = &node.construction_args {
			for input in inputs {
				let mut path = path.clone();
				path.push(*input);
				stack.push((*input, path));
			}
		}
	}
}

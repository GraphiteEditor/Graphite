use crate::document::NodeNetwork;
use crate::proto::{GraphErrors, ProtoNetwork};
use std::error::Error;

pub struct Compiler {}

pub struct CompiledNetwork {
	pub network: ProtoNetwork,
	pub warnings: GraphErrors,
}

impl Compiler {
	pub fn compile(&self, mut network: NodeNetwork) -> impl Iterator<Item = Result<CompiledNetwork, String>> {
		network.resolve_scope_inputs();
		network.generate_node_paths(&[]);
		let node_ids = network.nodes.keys().copied().collect::<Vec<_>>();
		network.populate_dependants();
		for id in node_ids {
			network.flatten(id);
		}
		network.remove_all_passthrough_nodes();
		// network.remove_dead_nodes(0);
		let proto_networks = network.into_proto_networks();

		proto_networks.map(move |mut proto_network| {
			proto_network.insert_context_nullification_nodes()?;
			let warnings = proto_network.footprint_invariance_violations();
			proto_network.generate_stable_node_ids();
			Ok(CompiledNetwork { network: proto_network, warnings })
		})
	}
	pub fn compile_single(&self, network: NodeNetwork) -> Result<CompiledNetwork, String> {
		assert_eq!(network.exports.len(), 1, "Graph with multiple outputs not yet handled");
		let Some(compiled_network) = self.compile(network).next() else {
			return Err("Failed to convert graph into proto graph".to_string());
		};
		compiled_network
	}
}

pub trait Executor<I, O> {
	fn execute(&self, input: I) -> Result<O, Box<dyn Error>>;
}

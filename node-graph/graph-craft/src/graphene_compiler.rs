use crate::document::NodeNetwork;
use crate::proto::{LocalFuture, ProtoNetwork};
use std::error::Error;

pub struct Compiler {}

impl Compiler {
	pub fn compile(&self, mut network: NodeNetwork) -> impl Iterator<Item = Result<ProtoNetwork, String>> {
		let node_ids = network.nodes.keys().copied().collect::<Vec<_>>();
		network.populate_dependants();
		for id in node_ids {
			network.flatten(id);
		}
		network.resolve_scope_inputs();
		network.remove_redundant_id_nodes();
		// network.remove_dead_nodes(0);
		let proto_networks = network.into_proto_networks();

		proto_networks.map(move |mut proto_network| {
			proto_network.insert_context_nullification_nodes()?;
			proto_network.generate_stable_node_ids();
			Ok(proto_network)
		})
	}
	pub fn compile_single(&self, network: NodeNetwork) -> Result<ProtoNetwork, String> {
		assert_eq!(network.exports.len(), 1, "Graph with multiple outputs not yet handled");
		let Some(proto_network) = self.compile(network).next() else {
			return Err("Failed to convert graph into proto graph".to_string());
		};
		proto_network
	}
}

pub trait Executor<I, O> {
	fn execute(&self, input: I) -> LocalFuture<'_, Result<O, Box<dyn Error>>>;
}

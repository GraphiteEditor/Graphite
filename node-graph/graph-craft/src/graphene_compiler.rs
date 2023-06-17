use std::error::Error;

use dyn_any::DynAny;

use crate::document::NodeNetwork;
use crate::proto::{LocalFuture, ProtoNetwork};

pub struct Compiler {}

impl Compiler {
	pub fn compile(&self, mut network: NodeNetwork, resolve_inputs: bool) -> impl Iterator<Item = ProtoNetwork> {
		let node_ids = network.nodes.keys().copied().collect::<Vec<_>>();
		println!("flattening");
		network.resolve_extract_nodes();
		for id in node_ids {
			network.flatten(id);
		}
		network.remove_redundant_id_nodes();
		network.remove_dead_nodes();
		let proto_networks = network.into_proto_networks();
		proto_networks.map(move |mut proto_network| {
			if resolve_inputs {
				println!("resolving inputs");
				proto_network.resolve_inputs();
			}
			proto_network.reorder_ids();
			proto_network.generate_stable_node_ids();
			proto_network
		})
	}
	pub fn compile_single(&self, network: NodeNetwork, resolve_inputs: bool) -> Result<ProtoNetwork, String> {
		assert_eq!(network.outputs.len(), 1, "Graph with multiple outputs not yet handled");
		let Some(proto_network) = self.compile(network, resolve_inputs).next() else {
			return Err("Failed to convert graph into proto graph".to_string());
		};
		Ok(proto_network)
	}
}
pub type Any<'a> = Box<dyn DynAny<'a> + 'a>;

pub trait Executor<I, O> {
	fn execute(&self, input: I) -> LocalFuture<Result<O, Box<dyn Error>>>;
}

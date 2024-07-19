use std::error::Error;

use crate::document::NodeNetwork;
use crate::proto::{LocalFuture, ProtoNetwork};

pub struct Compiler {}

impl Compiler {
	pub fn compile(&self, mut network: NodeNetwork) -> Result<impl Iterator<Item = ProtoNetwork>, String> {
		let node_ids = network.nodes.keys().copied().collect::<Vec<_>>();
		for id in node_ids {
			network.flatten(id);
		}
		network.resolve_scope_inputs();
		network.remove_redundant_id_nodes();
		network.remove_dead_nodes(0);
		let proto_networks = network.into_proto_networks();

		let proto_networks_result: Vec<ProtoNetwork> = proto_networks
			.map(move |mut proto_network| {
				proto_network.resolve_inputs()?;
				proto_network.generate_stable_node_ids();
				Ok(proto_network)
			})
			.collect::<Result<Vec<ProtoNetwork>, String>>()?;

		Ok(proto_networks_result.into_iter())
	}
	pub fn compile_single(&self, network: NodeNetwork) -> Result<ProtoNetwork, String> {
		assert_eq!(network.exports.len(), 1, "Graph with multiple outputs not yet handled");
		let Some(proto_network) = self.compile(network)?.next() else {
			return Err("Failed to convert graph into proto graph".to_string());
		};
		Ok(proto_network)
	}
}

pub trait Executor<I, O> {
	fn execute(&self, input: I) -> LocalFuture<Result<O, Box<dyn Error>>>;
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg(feature = "wgpu")]
pub struct CompileRequest {
	pub networks: Vec<ProtoNetwork>,
	pub io: wgpu_executor::ShaderIO,
}

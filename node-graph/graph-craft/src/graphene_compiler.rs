use crate::document::NodeNetwork;
use crate::proto::{GraphErrors, ProtoNetwork};
use std::error::Error;

pub struct Compiler {}

pub struct CompiledNetwork {
	pub network: ProtoNetwork,
	pub warnings: GraphErrors,
}

#[derive(Debug)]
pub enum CompileError {
	Message(String),
	Graph(GraphErrors),
}

impl std::fmt::Display for CompileError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			CompileError::Message(message) => write!(f, "{message}"),
			CompileError::Graph(errors) => write!(f, "{errors:?}"),
		}
	}
}

impl Error for CompileError {}

impl From<String> for CompileError {
	fn from(message: String) -> Self {
		CompileError::Message(message)
	}
}

impl Compiler {
	pub fn compile(&self, mut network: NodeNetwork) -> impl Iterator<Item = Result<CompiledNetwork, CompileError>> {
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

			let errors = proto_network.non_constant_attribute_names();
			if !errors.is_empty() {
				return Err(CompileError::Graph(errors));
			}
			let warnings = proto_network.footprint_invariance_violations();

			proto_network.generate_stable_node_ids();
			Ok(CompiledNetwork { network: proto_network, warnings })
		})
	}
	pub fn compile_single(&self, network: NodeNetwork) -> Result<CompiledNetwork, CompileError> {
		assert_eq!(network.exports.len(), 1, "Graph with multiple outputs not yet handled");
		let Some(compiled_network) = self.compile(network).next() else {
			return Err(CompileError::Message("Failed to convert graph into proto graph".to_string()));
		};
		compiled_network
	}
}

pub trait Executor<I, O> {
	fn execute(&self, input: I) -> Result<O, Box<dyn Error>>;
}

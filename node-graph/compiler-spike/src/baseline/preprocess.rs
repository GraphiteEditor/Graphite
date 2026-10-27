//! A port of the `preprocessor` crate's `Preprocessor::preprocess`, driven by the exported substitution tables.

use super::document::{DocumentNode, DocumentNodeImplementation, NodeInput, NodeNetwork, ValueKind};
use super::types::Type;
use super::{CompileMetadata, NodeId};
use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub struct Substitution {
	pub node: DocumentNode,
	pub wrapped_node_id: NodeId,
}

#[derive(Default)]
pub struct Preprocessor {
	pub substitutions: HashMap<Cow<'static, str>, Substitution>,
	pub inject_scopes: HashMap<Cow<'static, str>, (DocumentNode, Type)>,
	pub compile_metadata: HashMap<Cow<'static, str>, CompileMetadata>,
}

impl Preprocessor {
	pub fn preprocess(&self, network: &mut NodeNetwork) {
		self.insert_inject_scopes(network);
		Self::check_for_producer_inputs(network);
		self.expand_network(network);
	}

	fn insert_inject_scopes(&self, network: &mut NodeNetwork) {
		for (identifier, (template, ty)) in self.inject_scopes.iter() {
			let mut hasher = DefaultHasher::new();
			identifier.as_ref().hash(&mut hasher);
			let producer_id = NodeId(hasher.finish());
			network.nodes.insert(producer_id, template.clone());
			network.scope_injections.insert(identifier.to_string(), (producer_id, ty.clone()));
		}
	}

	/// Walks the network like `replace_inputs_with_producer_nodes`. The exported documents contain no resource or curve values to replace.
	fn check_for_producer_inputs(network: &mut NodeNetwork) {
		for node in network.nodes.values_mut() {
			if let DocumentNodeImplementation::Network(nested) = &mut node.implementation {
				Self::check_for_producer_inputs(nested);
				continue;
			}
			for input in node.inputs.iter_mut() {
				let NodeInput::Value { value, .. } = input else { continue };
				assert!(matches!(value.value.kind, ValueKind::Document { .. }), "Producer values are not supported by the spike");
			}
		}
	}

	fn expand_network(&self, network: &mut NodeNetwork) {
		for node in network.nodes.values_mut() {
			match &mut node.implementation {
				DocumentNodeImplementation::Network(node_network) => self.expand_network(node_network),
				DocumentNodeImplementation::ProtoNode(proto_node_identifier) => {
					let compile_metadata = node.compile_metadata.take().or_else(|| self.compile_metadata.get(proto_node_identifier).copied());

					if let Some(Substitution { node: new_node, wrapped_node_id }) = self.substitutions.get(proto_node_identifier) {
						while node.inputs.len() < new_node.inputs.len() {
							node.inputs.push(new_node.inputs[node.inputs.len()].clone());
						}
						node.inputs.truncate(new_node.inputs.len());

						node.implementation = new_node.implementation.clone();

						if let DocumentNodeImplementation::Network(network) = &mut node.implementation
							&& let Some(wrapped_node) = network.nodes.get_mut(wrapped_node_id)
						{
							wrapped_node.compile_metadata = compile_metadata;
						}
					} else {
						node.compile_metadata = compile_metadata;
					}
				}
				DocumentNodeImplementation::Extract => (),
			}
		}
	}
}

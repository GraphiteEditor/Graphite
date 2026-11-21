use crate::document::*;
use crate::proto::{ConstructionArgs, ProtoNetwork, ProtoNode};

impl DocumentNode {
	pub(crate) fn resolve_proto_node(self) -> ProtoNode {
		let DocumentNodeImplementation::ProtoNode(identifier) = self.implementation else {
			unreachable!("tried to resolve not flattened node on resolved node {self:?}");
		};
		assert!(!self.inputs.iter().any(|input| matches!(input, NodeInput::Import { .. })), "received non-resolved input");

		let mut construction_args = ConstructionArgs::Nodes(vec![]);

		// If we have one input of the type value, set it as the construction args
		if let &[NodeInput::Value { ref tagged_value, .. }] = self.inputs.as_slice() {
			construction_args = ConstructionArgs::Value(tagged_value.clone());
		}
		if let ConstructionArgs::Nodes(nodes) = &mut construction_args {
			nodes.extend(self.inputs.iter().map(|input| match input {
				NodeInput::Node { node_id, .. } => *node_id,
				_ => unreachable!(),
			}));
		}

		ProtoNode {
			identifier,
			call_argument: self.call_argument,
			construction_args,
			original_location: self.original_location,
			compile_metadata: self.compile_metadata.unwrap_or_default(),
			upstream_context_features: Default::default(),
			upstream_index_levels: 0,
		}
	}
}

impl NodeNetwork {
	/// Creates a proto network for evaluating each output of this network.
	pub fn into_proto_networks(self) -> impl Iterator<Item = ProtoNetwork> {
		#[cfg(test)]
		self.validate_dependants();

		let nodes: Vec<_> = self.nodes.into_iter().map(|(id, node)| (id, node.resolve_proto_node())).collect();

		// Create a network to evaluate each output
		if self.exports.len() == 1
			&& let NodeInput::Node { node_id, .. } = self.exports[0]
		{
			return vec![ProtoNetwork {
				inputs: Vec::new(),
				output: node_id,
				nodes,
			}]
			.into_iter();
		}

		// Create a network to evaluate each output
		let networks: Vec<_> = self
			.exports
			.into_iter()
			.filter_map(move |output| {
				if let NodeInput::Node { node_id, .. } = output {
					Some(ProtoNetwork {
						inputs: Vec::new(), // Inputs field is not used. Should be deleted
						// inputs: vec![input_node.expect("Set node should always exist")],
						// inputs: self.imports.clone(),
						output: node_id,
						nodes: nodes.clone(),
					})
				} else {
					None
				}
			})
			.collect();
		networks.into_iter()
	}

	/// Ensure the cached list of dependants exactly matches the real list of dependants
	#[cfg(test)]
	pub(crate) fn validate_dependants(&self) {
		use std::collections::{HashMap, HashSet};

		let mut dependants = HashMap::<NodeId, Vec<HashSet<NodeId>>>::new();

		for (&id, node) in self.nodes.iter() {
			dependants.insert(id, vec![HashSet::new(); node.implementation.output_count()]);
		}

		for (&id, node) in self.nodes.iter() {
			for input in &node.inputs {
				if let NodeInput::Node { node_id, output_index } = *input {
					dependants.get_mut(&node_id).unwrap()[output_index].insert(id);
				}
			}
		}

		for (&id, node) in self.nodes.iter() {
			for output_index in 0..node.implementation.output_count() {
				let real_dependants = &dependants.get(&id).unwrap()[output_index];
				let listed_dependants = node.original_location.dependants.get(output_index).map_or_default(|d| d.iter().copied().collect::<HashSet<_>>());
				assert_eq!(
					real_dependants, &listed_dependants,
					"real {:?} should equal listed {:?} for output index {} in id {:?} in network:\n{:#?}",
					real_dependants, listed_dependants, output_index, id, self
				);
			}
		}
	}
}

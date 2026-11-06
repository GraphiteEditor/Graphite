use crate::document::value::TaggedValue;
use crate::document::*;
use core_types::{ProtoNodeIdentifier, concrete};
use rustc_hash::FxHashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Hash two IDs together, returning a new ID that is always consistent for two input IDs in a specific order.
/// This is used during [`NodeNetwork::flatten`] in order to ensure consistent yet non-conflicting IDs for inner networks.
fn merge_ids(a: NodeId, b: NodeId) -> NodeId {
	let mut hasher = DefaultHasher::new();
	a.hash(&mut hasher);
	b.hash(&mut hasher);
	NodeId(hasher.finish())
}

impl DocumentNode {
	/// Locate the input that is a [`NodeInput::Import`] at index `offset` and replace it with a [`NodeInput::Node`].
	pub fn populate_first_network_input(&mut self, node_id: NodeId, output_index: usize, offset: usize) {
		let (index, _) = self
			.inputs
			.iter()
			.enumerate()
			.nth(offset)
			.unwrap_or_else(|| panic!("no network input found for {self:#?} and offset: {offset}"));

		self.inputs[index] = NodeInput::Node { node_id, output_index };
	}
}

impl NodeNetwork {
	/// Populate the [`DocumentNode::path`], which stores the location of the document node to allow for matching the resulting proto nodes to the document node for the purposes of typing and finding monitor nodes.
	///
	/// Table entries are not visited, since each instance gets its paths when [`Self::flatten`] copies the entry.
	pub fn generate_node_paths(&mut self, prefix: &[NodeId]) {
		let definitions = std::mem::take(&mut self.definitions);
		self.generate_node_paths_with(prefix, &definitions);
		self.definitions = definitions;
	}

	fn generate_node_paths_with(&mut self, prefix: &[NodeId], definitions: &FxHashMap<NetworkId, NodeNetwork>) {
		for (node_id, node) in &mut self.nodes {
			let mut new_path = prefix.to_vec();
			if !self.generated {
				new_path.push(*node_id);
			}
			if let DocumentNodeImplementation::Network(network) = &mut node.implementation {
				network.generate_node_paths_with(new_path.as_slice(), definitions);
			}
			if node.original_location.path.is_some() {
				log::warn!("Attempting to overwrite node path");
			} else {
				node.original_location.path = Some(new_path);
				let output_count = output_count_in(&node.implementation, definitions).unwrap_or(1);
				node.original_location.dependants = vec![Vec::new(); output_count];
			}
		}
	}

	/// A copy of table entry `id` for the instance at `instance_path`, with its node paths and dependant slots in place.
	fn instantiate_definition(&self, id: NetworkId, instance_path: &[NodeId]) -> Option<NodeNetwork> {
		let mut instance = self.definitions.get(&id)?.clone();
		instance.generate_node_paths_with(instance_path, &self.definitions);
		Some(instance)
	}

	pub fn populate_dependants(&mut self) {
		let mut dep_changes = Vec::new();
		for (node_id, node) in &mut self.nodes {
			let len = node.original_location.dependants.len();
			let output_count = output_count_in(&node.implementation, &self.definitions).unwrap_or(len);
			node.original_location.dependants.extend(vec![vec![]; output_count.max(len) - len]);
			for input in &node.inputs {
				if let NodeInput::Node { node_id: dep_id, output_index, .. } = input {
					dep_changes.push((*dep_id, *output_index, *node_id));
				}
			}
		}
		// println!("{:#?}", self.nodes.get(&NodeId(1)));
		for (dep_id, output_index, node_id) in dep_changes {
			let node = self.nodes.get_mut(&dep_id).expect("Encountered invalid node id");
			let len = node.original_location.dependants.len();
			// One must be added to the index to find the length because indexing in rust starts from 0.
			node.original_location.dependants.extend(vec![vec![]; (output_index + 1).max(len) - len]);
			// println!("{node_id} {output_index} {}", node.implementation.output_count());
			node.original_location.dependants[output_index].push(node_id);
		}
	}

	/// Replace all references in any node of `old_input` with `new_input`
	fn replace_node_inputs(&mut self, node_id: NodeId, old_input: (NodeId, usize), new_input: (NodeId, usize)) {
		let Some(node) = self.nodes.get_mut(&node_id) else { return };
		node.inputs.iter_mut().for_each(|input| {
			if let NodeInput::Node { node_id: input_id, output_index, .. } = input
				&& (*input_id, *output_index) == old_input
			{
				(*input_id, *output_index) = new_input;
			}
		});
	}

	/// Replace all references in any node of `old_output` with `new_output`
	fn replace_network_outputs(&mut self, old_output: &NodeInput, new_output: &NodeInput) {
		for output in self.exports.iter_mut() {
			if *output == *old_output {
				*output = new_output.clone();
			}
		}
	}

	/// Removes unused nodes from the graph. Returns a list of booleans which represent if each of the inputs have been retained.
	pub fn remove_dead_nodes(&mut self, number_of_inputs: usize) -> Vec<bool> {
		// Take all the nodes out of the nodes list
		let mut old_nodes = std::mem::take(&mut self.nodes);

		let mut stack = self
			.exports
			.iter()
			.filter_map(|output| if let NodeInput::Node { node_id, .. } = output { Some(*node_id) } else { None })
			.collect::<Vec<_>>();
		while let Some(node_id) = stack.pop() {
			let Some((node_id, mut document_node)) = old_nodes.remove_entry(&node_id) else {
				continue;
			};
			// Remove dead nodes from child networks
			if let DocumentNodeImplementation::Network(network) = &mut document_node.implementation {
				// Remove inputs to the parent node if they have been removed from the child
				let mut retain_inputs = network.remove_dead_nodes(document_node.inputs.len()).into_iter();
				document_node.inputs.retain(|_| retain_inputs.next().unwrap_or(true))
			}
			// Visit all nodes that this node references
			stack.extend(
				document_node
					.inputs
					.iter()
					.filter_map(|input| if let NodeInput::Node { node_id, .. } = input { Some(node_id) } else { None }),
			);
			// Add the node back to the list of nodes
			self.nodes.insert(node_id, document_node);
		}

		// Remove references to nodes that have been deleted
		for node in self.nodes.values_mut() {
			for dependants in &mut node.original_location.dependants {
				dependants.retain(|dependant| !old_nodes.contains_key(dependant));
			}
		}

		// Check if inputs are used and store for return value
		let mut are_inputs_used = vec![false; number_of_inputs];
		for node in &self.nodes {
			for node_input in &node.1.inputs {
				if let NodeInput::Import { import_index, .. } = node_input
					&& let Some(is_used) = are_inputs_used.get_mut(*import_index)
				{
					*is_used = true;
				}
			}
		}
		are_inputs_used
	}

	/// Remove all nodes that contain [`DocumentNodeImplementation::Network`] by moving the nested nodes into the parent network.
	pub fn flatten(&mut self, node_id: NodeId) {
		self.flatten_with_fns(node_id, merge_ids, NodeId::new)
	}

	/// Remove all nodes that contain [`DocumentNodeImplementation::Network`] by moving the nested nodes into the parent network.
	pub fn flatten_with_fns(&mut self, node_id: NodeId, map_ids: impl Fn(NodeId, NodeId) -> NodeId + Copy, gen_id: impl Fn() -> NodeId + Copy) {
		let Some((id, mut node)) = self.nodes.remove_entry(&node_id) else {
			warn!("The node which was supposed to be flattened does not exist in the network, id {node_id} network {self:#?}");
			return;
		};

		// If the node is hidden, replace it with a passthrough node
		let passthrough_node = DocumentNodeImplementation::ProtoNode(graphene_core::ops::passthrough::IDENTIFIER);
		if !node.visible && node.implementation != passthrough_node {
			node.implementation = passthrough_node;

			// A node without inputs has nothing to pass through, so it passes through the unit value instead
			if node.inputs.is_empty() {
				node.inputs.push(NodeInput::value(TaggedValue::None, false));
			}

			// Keep only the primary input, which the passthrough forwards
			let removed_inputs = node.inputs.split_off(1);

			// Remove the dependants on the ignored inputs
			for removed_input in removed_inputs {
				if let NodeInput::Node { node_id, output_index } = removed_input
					&& let Some(former_dependency) = self.nodes.get_mut(&node_id)
					// Ensure that the other node inputs do not also reference the dependency
					&& !node.inputs.iter().any(|input| matches!(input, NodeInput::Node { node_id: other, .. } if *other == node_id))
				{
					former_dependency.original_location.dependants[output_index].retain(|&dependant| dependant != id);
				}
			}

			// Removing the passthrough later splices its input into its consumers, which accept only node wires, so an unwired input becomes a value node
			let path = node.original_location.path.clone().unwrap_or_default();
			Self::replace_value_inputs_with_nodes(&mut node.inputs, &mut self.nodes, &path, gen_id, map_ids, id);

			node.call_argument = concrete!(());
			self.nodes.insert(id, node);
			return;
		}

		let path = node.original_location.path.clone().unwrap_or_default();

		// Replace value inputs with dedicated value nodes
		if node.implementation != DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("core_types::value::ClonedNode")) {
			Self::replace_value_inputs_with_nodes(&mut node.inputs, &mut self.nodes, &path, gen_id, map_ids, id);
		}

		if let DocumentNodeImplementation::Definition(definition) = node.implementation {
			let Some(instance) = self.instantiate_definition(definition, &path) else {
				log::error!("Node {id} names definition {definition:?}, which the definitions table lacks");
				self.nodes.insert(id, node);
				return;
			};
			node.implementation = DocumentNodeImplementation::Network(instance);
		}

		let DocumentNodeImplementation::Network(mut inner_network) = node.implementation else {
			// If the node is not a network, it is a primitive node and can be inserted into the network as is.
			assert!(!self.nodes.contains_key(&id), "Trying to insert a node into the network caused an id conflict");

			self.nodes.insert(id, node);
			return;
		};
		debug_assert!(
			node.compile_metadata.is_none(),
			"Network {id} should not have compile metadata: {compile_metadata:#?}",
			compile_metadata = node.compile_metadata
		);

		// Replace value and reflection imports with value nodes, added inside nested network
		Self::replace_value_inputs_with_nodes(
			&mut inner_network.exports,
			&mut inner_network.nodes,
			node.original_location.path.as_ref().unwrap_or(&vec![]),
			gen_id,
			map_ids,
			id,
		);

		// Connect all network inputs to either the parent network nodes, or newly created value nodes for the parent node.
		inner_network.map_ids(|inner_id| map_ids(id, inner_id));
		inner_network.populate_dependants();
		let new_nodes = inner_network.nodes.keys().cloned().collect::<Vec<_>>();

		for (key, value) in inner_network.scope_injections.into_iter() {
			match self.scope_injections.entry(key) {
				std::collections::hash_map::Entry::Occupied(o) => {
					log::warn!("Found duplicate scope injection for key {}, ignoring", o.key());
				}
				std::collections::hash_map::Entry::Vacant(v) => {
					v.insert(value);
				}
			}
		}

		// Match the document node input and the inputs of the inner network
		for (nested_node_id, mut nested_node) in inner_network.nodes.into_iter() {
			for (nested_input_index, nested_input) in nested_node.clone().inputs.iter().enumerate() {
				if let NodeInput::Import { import_index, .. } = nested_input {
					let parent_input = node
						.inputs
						.get(*import_index)
						.unwrap_or_else(|| panic!("Import index {import_index} of network node implementation {:?} should always exist", nested_node.implementation));
					match *parent_input {
						// If the input to self is a node, connect the corresponding output of the inner network to it
						NodeInput::Node { node_id, output_index } => {
							nested_node.populate_first_network_input(node_id, output_index, nested_input_index);
							let input_node = self.nodes.get_mut(&node_id).unwrap_or_else(|| panic!("Unable to find input node {node_id:?}"));
							input_node.original_location.dependants[output_index].push(nested_node_id);
						}
						NodeInput::Import { import_index, .. } => {
							let parent_input_index = import_index;
							let Some(NodeInput::Import { import_index, .. }) = nested_node.inputs.get_mut(nested_input_index) else {
								log::error!("Nested node should have a network input");
								continue;
							};
							*import_index = parent_input_index;
						}
						NodeInput::Value { .. } => unreachable!("Value inputs should have been replaced with value nodes"),
						NodeInput::Scope(_) => unreachable!("Scope inputs should have been resolved by resolve_scope_inputs_recursive before flattening"),
						NodeInput::Reflection(_) => unreachable!("Reflection inputs should have been replaced with value nodes"),
					}
				}
			}
			self.nodes.insert(nested_node_id, nested_node);
		}

		// Remove the newly flattened node from the dependants
		for parent_input in &node.inputs {
			if let NodeInput::Node { node_id, output_index } = *parent_input
				&& let Some(input_node) = self.nodes.get_mut(&node_id)
			{
				input_node.original_location.dependants[output_index].retain(|&dependant| dependant != id);
			}
		}

		// TODO: Add support for flattening exports that are NodeInput::Import (https://github.com/GraphiteEditor/Graphite/issues/1762)

		self.replace_node_with_its_exports(id, &node.original_location, &inner_network.exports);

		for node_id in new_nodes {
			self.flatten_with_fns(node_id, map_ids, gen_id);
		}
	}

	/// Connect all nodes that were previously connected to this node to the nodes of the inner network
	fn replace_node_with_its_exports(&mut self, id: NodeId, original_location: &OriginalLocation, exports: &[NodeInput]) {
		for (i, export) in exports.iter().enumerate() {
			if let NodeInput::Node { node_id, output_index, .. } = &export {
				for deps in &original_location.dependants {
					for dep in deps {
						self.replace_node_inputs(*dep, (id, i), (*node_id, *output_index));
					}
				}

				if let Some(new_output_node) = self.nodes.get_mut(node_id) {
					for dep in &original_location.dependants[i] {
						new_output_node.original_location.dependants[*output_index].push(*dep);
					}

					// Remove references from dependants on wrapper
					new_output_node.original_location.dependants[*output_index].retain(|&dependant| dependant != id);
				}
			}

			self.replace_network_outputs(&NodeInput::node(id, i), export);
		}
	}

	#[inline(never)]
	fn replace_value_inputs_with_nodes(
		inputs: &mut [NodeInput],
		collection: &mut FxHashMap<NodeId, DocumentNode>,
		path: &[NodeId],
		gen_id: impl Fn() -> NodeId + Copy,
		map_ids: impl Fn(NodeId, NodeId) -> NodeId + Copy,
		id: NodeId,
	) {
		// Replace value exports and imports with value nodes, added inside the nested network
		for export in inputs {
			let previous_export = std::mem::replace(export, NodeInput::import(concrete!(()), 0));

			let (tagged_value, exposed) = match previous_export {
				NodeInput::Value { tagged_value, exposed } => (tagged_value, exposed),
				NodeInput::Reflection(reflect) => match reflect {
					DocumentNodeMetadata::DocumentNodePath => (TaggedValue::NodeIdPath(core_types::list::NodeIdPath::from(path.to_vec())).into(), false),
					DocumentNodeMetadata::SourceId => (TaggedValue::SourceId(core_types::SourceId::from_path(path)).into(), false),
				},
				previous_export => {
					*export = previous_export;
					continue;
				}
			};
			let value_node_id = gen_id();
			let merged_node_id = map_ids(id, value_node_id);
			let mut original_location = OriginalLocation {
				path: Some(path.to_vec()),
				dependants: vec![vec![id]],
				..Default::default()
			};

			if let Some(path) = &mut original_location.path {
				path.push(value_node_id);
			}
			collection.insert(
				merged_node_id,
				DocumentNode {
					inputs: vec![NodeInput::Value { tagged_value, exposed }],
					implementation: DocumentNodeImplementation::ProtoNode(ProtoNodeIdentifier::new("core_types::value::ClonedNode")),
					original_location,
					..Default::default()
				},
			);
			*export = NodeInput::Node {
				node_id: merged_node_id,
				output_index: 0,
			};
		}
	}

	fn remove_passthrough_node(&mut self, passthrough_id: NodeId) {
		let node = self.nodes.remove(&passthrough_id).unwrap_or_else(|| panic!("Node with id {passthrough_id} does not exist"));

		let implementation = &node.implementation;
		assert!(
			matches!(implementation, DocumentNodeImplementation::ProtoNode(ident) if ident == &graphene_core::ops::passthrough::IDENTIFIER),
			"remove_passthrough_node was given a node {implementation:?} and id {passthrough_id:?}",
		);
		assert_eq!(node.inputs.len(), 1, "Passthrough node has more than one input");

		// Consider the upstream node (the input to the passthrough)
		if let NodeInput::Node {
			node_id: upstream_node_id,
			output_index: upstream_output_index,
			..
		} = node.inputs[0]
			&& let Some(upstream_node) = self.nodes.get_mut(&upstream_node_id)
		{
			let dependants_of_upstream = &mut upstream_node.original_location.dependants[upstream_output_index];
			// Add our dependants to the list of dependants
			dependants_of_upstream.extend(node.original_location.dependants[0].iter().copied());
			// Remove ourselves from the list of dependants
			dependants_of_upstream.retain(|&dependant| dependant != passthrough_id);
		};

		// Check if the passthrough output is connected to a network export
		for node_input in self.exports.iter_mut() {
			if matches!(node_input,  NodeInput::Node { node_id, .. } if *node_id == passthrough_id) {
				*node_input = node.inputs[0].clone();
			}
		}

		// Look at all downstream nodes
		for &dependant_id in &node.original_location.dependants[0] {
			let Some(dependant_node) = self.nodes.get_mut(&dependant_id) else {
				error!("remove_passthrough_node got a dependant id {dependant_id:?} that did not exist in the network\n{self:#?}");
				continue;
			};
			for input in dependant_node.inputs.iter_mut() {
				if matches!(input, NodeInput::Node { node_id, .. } if *node_id == passthrough_id) {
					*input = node.inputs[0].clone();
				}
			}
		}
	}

	/// Strips out all [`graphene_core::ops::PassthroughNode`]s.
	pub fn remove_all_passthrough_nodes(&mut self) {
		let passthrough_nodes = self
			.nodes
			.iter()
			.filter(|(_, node)| matches!(&node.implementation, DocumentNodeImplementation::ProtoNode(ident) if ident == &graphene_core::ops::passthrough::IDENTIFIER))
			.map(|(id, _)| *id)
			.collect::<Vec<_>>();
		for id in passthrough_nodes {
			self.remove_passthrough_node(id);
		}
	}
}

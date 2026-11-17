//! Writes each layer's path onto what it renders, so what's rendered can be traced back to its layer.

use crate::node_registry::NODE_REGISTRY;
use graph_craft::ProtoNodeIdentifier;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, DocumentNodeMetadata, NodeId, NodeInput, NodeNetwork};
use graphene_std::NodeParameter;
use graphene_std::graphic::{extend, write_layer_path};
use graphene_std::list::NodeIdPath;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

/// What a layer adds to the document, which is what its thumbnail shows and what its path is written onto.
pub enum LayerContent {
	/// What the Extend node the layer ends in is given to add, with the path of the network the node is in.
	Added { network_path: Vec<NodeId>, extend: NodeId },
	/// Everything the layer outputs, from the node at this path.
	Output(Vec<NodeId>),
}

/// The content of the layer at this path, unless it's hidden or passes through what's below it and so has none of its own.
pub fn layer_content(layer_path: &[NodeId], network: &NodeNetwork) -> Option<LayerContent> {
	let output = network.output_source(layer_path, 0)?;
	if !output.starts_with(layer_path) {
		return None;
	}

	let (node_id, network_path) = output.split_last()?;
	let node = network.nested_network(network_path)?.nodes.get(node_id)?;
	if node.implementation != DocumentNodeImplementation::ProtoNode(extend::IDENTIFIER) {
		return Some(LayerContent::Output(output));
	}

	Some(LayerContent::Added {
		network_path: network_path.to_vec(),
		extend: *node_id,
	})
}

impl LayerContent {
	/// The path of the node producing the content once compiled.
	pub fn source(&self, network: &NodeNetwork) -> Option<Vec<NodeId>> {
		match self {
			Self::Added { network_path, extend } => {
				let added = network.nested_network(network_path)?.nodes.get(extend)?.inputs.get(extend::NewInput::INDEX)?;
				network.input_source(network_path, added)
			}
			Self::Output(output) => Some(output.clone()),
		}
	}
}

/// Puts a node after each layer's content that writes the layer's path onto it.
/// A layer has none if its content may be of a type that can't carry the path.
pub fn write_layer_paths(network: &mut NodeNetwork, layer_paths: &[Vec<NodeId>]) {
	let mut carries_layer_path = HashMap::new();

	// O(layers * depth)
	for layer_path in layer_paths {
		let Some((layer, parent_path)) = layer_path.split_last() else { continue };
		let Some(content) = layer_content(layer_path, network) else { continue };

		// An Extend node deeper inside the layer may be in a network other layers share, so the writer goes on the layer's whole output instead
		let content = match content {
			LayerContent::Added { network_path, extend } if network_path.len() > layer_path.len() => LayerContent::Output([network_path.as_slice(), &[extend]].concat()),
			content => content,
		};

		let Some(source) = content.source(network) else { continue };
		if !source_can_carry_layer_path(source, network, &mut carries_layer_path) {
			continue;
		}

		match content {
			// The writer goes between the Extend node and what it's given to add, inside the layer or beside it if the layer is that node
			LayerContent::Added { network_path, extend } => {
				let writer = layer_path_writer_id(extend);
				let layer_beside = if network_path.len() == layer_path.len() { Vec::new() } else { vec![*layer] };
				let Some(holder) = network_mut(network, &network_path) else { continue };
				let Some(added) = holder.nodes.get_mut(&extend).and_then(|node| node.inputs.get_mut(extend::NewInput::INDEX)) else {
					continue;
				};

				// Every layer made from the same network shares the writer
				if *added == NodeInput::node(writer, 0) {
					continue;
				}

				let content = std::mem::replace(added, NodeInput::node(writer, 0));
				holder.nodes.insert(writer, layer_path_writer(content, layer_beside));
			}
			// The writer goes beside the layer, taking over everything its output feeds
			LayerContent::Output(_) => {
				let writer = layer_path_writer_id(*layer);
				let Some(holder) = network_mut(network, parent_path) else { continue };
				if holder.nodes.contains_key(&writer) {
					continue;
				}

				let (output, written) = (NodeInput::node(*layer, 0), NodeInput::node(writer, 0));
				let inputs = holder.nodes.values_mut().flat_map(|node| &mut node.inputs).chain(&mut holder.exports);
				inputs.filter(|input| **input == output).for_each(|input| *input = written.clone());

				holder.nodes.insert(writer, layer_path_writer(output, vec![*layer]));
			}
		}
	}
}

/// How the layer path writer fares with what a proto node outputs.
#[derive(Clone, Copy)]
enum CarriesLayerPath {
	Always,
	/// The node outputs the type its one input is given, so that decides it.
	AsItsInput,
	Never,
}

/// Whether the layer path writer can take everything the proto node at this path may output.
fn source_can_carry_layer_path(mut source: Vec<NodeId>, network: &NodeNetwork, carries_layer_path: &mut HashMap<ProtoNodeIdentifier, CarriesLayerPath>) -> bool {
	loop {
		let Some((node_id, network_path)) = source.split_last() else { return false };
		let Some(node) = network.nested_network(network_path).and_then(|network| network.nodes.get(node_id)) else {
			return false;
		};
		let DocumentNodeImplementation::ProtoNode(identifier) = &node.implementation else { return false };

		match *carries_layer_path.entry(identifier.clone()).or_insert_with(|| output_can_carry_layer_path(identifier)) {
			CarriesLayerPath::Always => return true,
			CarriesLayerPath::Never => return false,
			CarriesLayerPath::AsItsInput => {
				let Some(input_source) = node.inputs.first().and_then(|input| network.input_source(network_path, input)) else {
					return false;
				};
				source = input_source;
			}
		}
	}
}

fn output_can_carry_layer_path(identifier: &ProtoNodeIdentifier) -> CarriesLayerPath {
	let (Some(outputs), Some(writers)) = (NODE_REGISTRY.get(identifier), NODE_REGISTRY.get(&write_layer_path::IDENTIFIER)) else {
		return CarriesLayerPath::Never;
	};

	let writes_type = |output: &graph_craft::Type| {
		let content = |writer: &graphene_std::NodeIOTypes| writer.inputs.get(write_layer_path::ContentInput::INDEX).is_some_and(|content| content.nested_type() == output);
		writers.keys().any(content)
	};
	if outputs.keys().all(|output| writes_type(output.return_value.nested_type())) {
		return CarriesLayerPath::Always;
	}

	let as_its_input = |output: &graphene_std::NodeIOTypes| matches!(output.inputs.as_slice(), [input] if input.nested_type() == output.return_value.nested_type());
	if outputs.keys().all(as_its_input) {
		CarriesLayerPath::AsItsInput
	} else {
		CarriesLayerPath::Never
	}
}

/// The network at this path, which is either an entry of the definitions table or held by its node.
fn network_mut<'a>(network: &'a mut NodeNetwork, path: &[NodeId]) -> Option<&'a mut NodeNetwork> {
	// The last table entry along the path, which holds any networks after it
	let mut table_entry = None;
	let mut current = &*network;
	for (index, node_id) in path.iter().enumerate() {
		current = match &current.nodes.get(node_id)?.implementation {
			DocumentNodeImplementation::Definition(id) => {
				table_entry = Some((*id, index + 1));
				network.definitions.get(id)?
			}
			DocumentNodeImplementation::Network(held) => held,
			_ => return None,
		};
	}

	let (mut current, held_path) = match table_entry {
		Some((id, held_from)) => (network.definitions.get_mut(&id)?, &path[held_from..]),
		None => (network, path),
	};
	for node_id in held_path {
		let DocumentNodeImplementation::Network(held) = &mut current.nodes.get_mut(node_id)?.implementation else {
			return None;
		};
		current = held;
	}

	Some(current)
}

/// The ID of the node writing the layer path onto what the node with this ID outputs or is given, which stays the same between compilations.
pub fn layer_path_writer_id(node_id: NodeId) -> NodeId {
	let mut hasher = DefaultHasher::new();
	"layer path writer".hash(&mut hasher);
	node_id.hash(&mut hasher);
	NodeId(hasher.finish())
}

fn layer_path_writer(content: NodeInput, layer_beside: Vec<NodeId>) -> DocumentNode {
	DocumentNode {
		inputs: vec![
			content,
			NodeInput::value(TaggedValue::NodeIdPath(NodeIdPath::from(layer_beside)), false),
			NodeInput::Reflection(DocumentNodeMetadata::DocumentNodePath),
		],
		implementation: DocumentNodeImplementation::ProtoNode(write_layer_path::IDENTIFIER),
		..Default::default()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use graph_craft::document::NetworkId;

	#[test]
	fn content_that_cannot_carry_a_layer_path_is_left_alone() {
		let path_node = DocumentNode {
			inputs: vec![NodeInput::Reflection(DocumentNodeMetadata::DocumentNodePath)],
			implementation: DocumentNodeImplementation::ProtoNode(graphene_std::graphic::path_of_subgraph::IDENTIFIER),
			..Default::default()
		};
		let mut network = NodeNetwork {
			exports: vec![NodeInput::node(NodeId(1), 0)],
			nodes: [(NodeId(1), path_node)].into_iter().collect(),
			..Default::default()
		};
		let unchanged = network.clone();

		write_layer_paths(&mut network, &[vec![NodeId(1)]]);

		assert_eq!(network, unchanged);
	}

	fn proto_node(identifier: ProtoNodeIdentifier, inputs: Vec<NodeInput>) -> DocumentNode {
		DocumentNode {
			inputs,
			implementation: DocumentNodeImplementation::ProtoNode(identifier),
			..Default::default()
		}
	}

	fn definition_node(id: NetworkId) -> DocumentNode {
		DocumentNode {
			implementation: DocumentNodeImplementation::Definition(id),
			..Default::default()
		}
	}

	/// A network shaped like 'Merge', where an Extend node adds what the node beside it gives.
	fn merge_like(added: DocumentNode) -> NodeNetwork {
		let extend = proto_node(extend::IDENTIFIER, vec![NodeInput::value(TaggedValue::None, false), NodeInput::node(NodeId(2), 0)]);
		NodeNetwork {
			exports: vec![NodeInput::node(NodeId(1), 0)],
			nodes: [(NodeId(1), extend), (NodeId(2), added)].into_iter().collect(),
			..Default::default()
		}
	}

	// One network is shared by a layer at the root and by a layer inside another layer, so the one writer inside it has to suit both
	#[test]
	fn a_network_shared_by_layers_at_different_depths_writes_each_layer_path() {
		let (merge, group) = (NetworkId(1), NetworkId(2));
		let rectangle = proto_node(graphene_std::vector::generator_nodes::rectangle::IDENTIFIER, Vec::new());
		let group_network = NodeNetwork {
			exports: vec![NodeInput::node(NodeId(30), 0)],
			nodes: [(NodeId(30), definition_node(merge))].into_iter().collect(),
			..Default::default()
		};
		let network = NodeNetwork {
			exports: vec![NodeInput::node(NodeId(10), 0)],
			nodes: [(NodeId(10), definition_node(merge)), (NodeId(20), definition_node(group))].into_iter().collect(),
			definitions: [(merge, merge_like(rectangle)), (group, group_network)].into_iter().collect(),
			..Default::default()
		};

		// The outer layer comes first in one order and last in the other
		let orders = [
			[vec![NodeId(20)], vec![NodeId(10)], vec![NodeId(20), NodeId(30)]],
			[vec![NodeId(10)], vec![NodeId(20), NodeId(30)], vec![NodeId(20)]],
		];
		let written = orders.map(|layer_paths| {
			let mut network = network.clone();
			write_layer_paths(&mut network, &layer_paths);
			network
		});
		assert_eq!(written[0], written[1], "The order the layers are given in should not change what's written");

		// The writer writes the path of the layer holding its network, whichever layer that is
		let writer = &written[0].definitions[&merge].nodes[&layer_path_writer_id(NodeId(1))];
		assert_eq!(writer.inputs[1], NodeInput::value(TaggedValue::NodeIdPath(NodeIdPath::from(Vec::new())), false));
	}

	// Content that already writes its own layer path, like the 'Merge' networks of older documents, gets no second writer
	#[test]
	fn a_network_writing_its_own_layer_path_is_left_alone() {
		let write_attribute = proto_node(graphene_std::graphic::write_attribute::IDENTIFIER, Vec::new());
		let mut network = NodeNetwork {
			exports: vec![NodeInput::node(NodeId(10), 0)],
			nodes: [(NodeId(10), definition_node(NetworkId(1)))].into_iter().collect(),
			definitions: [(NetworkId(1), merge_like(write_attribute))].into_iter().collect(),
			..Default::default()
		};
		let unchanged = network.clone();

		write_layer_paths(&mut network, &[vec![NodeId(10)]]);

		assert_eq!(network, unchanged);
	}
}

use graph_craft::ProtoNodeIdentifier;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, DocumentNodeMetadata, NodeId, NodeInput, NodeNetwork};
use graphene_std::NodeParameter;
use graphene_std::graphic::{extend, stamp_layer_path};
use graphene_std::list::NodeIdPath;
use interpreted_executor::node_registry::NODE_REGISTRY;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

/// What a layer adds to the document, which is what its thumbnail shows and what its path is stamped on.
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

/// Puts a node after each layer's content that stamps it with the layer's path, which is how the editor traces what's rendered back to its layer.
/// A layer has none if its content may be of a type that can't be stamped.
pub fn stamp_layer_paths(network: &mut NodeNetwork, layer_paths: &[Vec<NodeId>]) {
	let mut stampable = HashMap::new();

	// O(layers * depth)
	for layer_path in layer_paths {
		let Some((layer, parent_path)) = layer_path.split_last() else { continue };
		let Some(content) = layer_content(layer_path, network) else { continue };
		let Some(source) = content.source(network) else { continue };
		if !source_is_stampable(source, network, &mut stampable) {
			continue;
		}

		match content {
			// The stamp goes inside the layer, between the Extend node and what it's given to add
			LayerContent::Added { network_path, extend } => {
				let stamp = stamp_id(extend);
				let levels_up = network_path.len() - layer_path.len() + 1;
				let Some(holder) = network_mut(network, &network_path) else { continue };
				let Some(added) = holder.nodes.get_mut(&extend).and_then(|node| node.inputs.get_mut(extend::NewInput::INDEX)) else {
					continue;
				};

				// Every layer made from the same network shares the stamp
				if *added == NodeInput::node(stamp, 0) {
					continue;
				}

				let content = std::mem::replace(added, NodeInput::node(stamp, 0));
				holder.nodes.insert(stamp, stamp_node(content, levels_up, Vec::new()));
			}
			// The stamp goes beside the layer, taking over everything its output feeds
			LayerContent::Output(_) => {
				let stamp = stamp_id(*layer);
				let Some(holder) = network_mut(network, parent_path) else { continue };
				if holder.nodes.contains_key(&stamp) {
					continue;
				}

				let (output, stamped) = (NodeInput::node(*layer, 0), NodeInput::node(stamp, 0));
				let inputs = holder.nodes.values_mut().flat_map(|node| &mut node.inputs).chain(&mut holder.exports);
				inputs.filter(|input| **input == output).for_each(|input| *input = stamped.clone());

				holder.nodes.insert(stamp, stamp_node(output, 1, vec![*layer]));
			}
		}
	}
}

/// How the stamp node fares with what a proto node outputs.
#[derive(Clone, Copy)]
enum Stampable {
	Always,
	/// The node outputs the type its one input is given, so that decides it.
	AsItsInput,
	Never,
}

/// Whether the stamp node can take everything the proto node at this path may output.
fn source_is_stampable(mut source: Vec<NodeId>, network: &NodeNetwork, stampable: &mut HashMap<ProtoNodeIdentifier, Stampable>) -> bool {
	loop {
		let Some((node_id, network_path)) = source.split_last() else { return false };
		let Some(node) = network.nested_network(network_path).and_then(|network| network.nodes.get(node_id)) else {
			return false;
		};
		let DocumentNodeImplementation::ProtoNode(identifier) = &node.implementation else { return false };

		match *stampable.entry(identifier.clone()).or_insert_with(|| output_is_stampable(identifier)) {
			Stampable::Always => return true,
			Stampable::Never => return false,
			Stampable::AsItsInput => {
				let Some(input_source) = node.inputs.first().and_then(|input| network.input_source(network_path, input)) else {
					return false;
				};
				source = input_source;
			}
		}
	}
}

fn output_is_stampable(identifier: &ProtoNodeIdentifier) -> Stampable {
	let (Some(outputs), Some(stamps)) = (NODE_REGISTRY.get(identifier), NODE_REGISTRY.get(&stamp_layer_path::IDENTIFIER)) else {
		return Stampable::Never;
	};

	let stamps_type = |output: &graph_craft::Type| {
		let content = |stamp: &graphene_std::NodeIOTypes| stamp.inputs.get(stamp_layer_path::ContentInput::INDEX).is_some_and(|content| content.nested_type() == output);
		stamps.keys().any(content)
	};
	if outputs.keys().all(|output| stamps_type(output.return_value.nested_type())) {
		return Stampable::Always;
	}

	let as_its_input = |output: &graphene_std::NodeIOTypes| matches!(output.inputs.as_slice(), [input] if input.nested_type() == output.return_value.nested_type());
	if outputs.keys().all(as_its_input) { Stampable::AsItsInput } else { Stampable::Never }
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

/// The ID of the node stamping what the node with this ID outputs or is given, which stays the same between compilations.
pub fn stamp_id(stamped: NodeId) -> NodeId {
	let mut hasher = DefaultHasher::new();
	"layer path stamp".hash(&mut hasher);
	stamped.hash(&mut hasher);
	NodeId(hasher.finish())
}

fn stamp_node(content: NodeInput, levels_up: usize, layer_beside: Vec<NodeId>) -> DocumentNode {
	DocumentNode {
		inputs: vec![
			content,
			NodeInput::value(TaggedValue::Integer(levels_up as i64), false),
			NodeInput::value(TaggedValue::NodeIdPath(NodeIdPath::from(layer_beside)), false),
			NodeInput::Reflection(DocumentNodeMetadata::DocumentNodePath),
		],
		implementation: DocumentNodeImplementation::ProtoNode(stamp_layer_path::IDENTIFIER),
		..Default::default()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;
	use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
	use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, OutputConnector};
	use crate::test_utils::test_prelude::*;

	// A layer that ends in no Extend node has its whole output stamped, which gives the editor its click targets
	#[tokio::test]
	async fn a_node_shown_as_a_layer_is_traced_back_to() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let rectangle = DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::rectangle::IDENTIFIER);
		let rectangle = editor.create_node_by_name_at(rectangle, 0, 0).await;
		editor
			.handle_message(NodeGraphMessage::CreateWire {
				output_connector: OutputConnector::node(rectangle, 0),
				input_connector: InputConnector::Export(0),
			})
			.await;
		assert!(editor.active_document().metadata().click_targets.is_empty(), "A node not shown as a layer should have no click targets");

		editor.handle_message(NodeGraphMessage::SetToNodeOrLayer { node_id: rectangle, is_layer: true }).await;

		let document = editor.active_document();
		let layer = LayerNodeIdentifier::new(rectangle, &document.network_interface);
		let click_targets = document.metadata().click_targets(layer);
		assert!(
			click_targets.is_some_and(|click_targets| !click_targets.is_empty()),
			"A node shown as a layer should have click targets"
		);
	}

	#[test]
	fn content_of_a_type_without_a_stamp_is_left_alone() {
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
		let unstamped = network.clone();

		stamp_layer_paths(&mut network, &[vec![NodeId(1)]]);

		assert_eq!(network, unstamped);
	}
}

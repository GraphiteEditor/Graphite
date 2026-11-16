use graph_craft::ProtoNodeIdentifier;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{DocumentNode, DocumentNodeImplementation, DocumentNodeMetadata, NodeId, NodeInput, NodeNetwork};
use graphene_std::NodeParameter;
use graphene_std::graphic::{extend, write_layer_path};
use graphene_std::list::NodeIdPath;
use interpreted_executor::node_registry::NODE_REGISTRY;
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

/// Puts a node after each layer's content that writes the layer's path onto it, which is how the editor traces what's rendered back to its layer.
/// A layer has none if its content may be of a type that can't carry the path.
pub fn write_layer_paths(network: &mut NodeNetwork, layer_paths: &[Vec<NodeId>]) {
	let mut carries_layer_path = HashMap::new();

	// O(layers * depth)
	for layer_path in layer_paths {
		let Some((layer, parent_path)) = layer_path.split_last() else { continue };
		let Some(content) = layer_content(layer_path, network) else { continue };
		let Some(source) = content.source(network) else { continue };
		if !source_can_carry_layer_path(source, network, &mut carries_layer_path) {
			continue;
		}

		match content {
			// The writer goes between the Extend node and what it's given to add, inside the layer or beside it if the layer is that node
			LayerContent::Added { network_path, extend } => {
				let writer = layer_path_writer_id(extend);
				let (levels_up, layer_beside) = match network_path.len().checked_sub(layer_path.len()) {
					Some(depth_inside_layer) => (depth_inside_layer + 1, Vec::new()),
					None => (1, vec![*layer]),
				};
				let Some(holder) = network_mut(network, &network_path) else { continue };
				let Some(added) = holder.nodes.get_mut(&extend).and_then(|node| node.inputs.get_mut(extend::NewInput::INDEX)) else {
					continue;
				};

				// Every layer made from the same network shares the writer
				if *added == NodeInput::node(writer, 0) {
					continue;
				}

				let content = std::mem::replace(added, NodeInput::node(writer, 0));
				holder.nodes.insert(writer, layer_path_writer(content, levels_up, layer_beside));
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

				holder.nodes.insert(writer, layer_path_writer(output, 1, vec![*layer]));
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

fn layer_path_writer(content: NodeInput, levels_up: usize, layer_beside: Vec<NodeId>) -> DocumentNode {
	DocumentNode {
		inputs: vec![
			content,
			NodeInput::value(TaggedValue::Integer(levels_up as i64), false),
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
	use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;
	use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
	use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, OutputConnector};
	use crate::test_utils::test_prelude::*;

	// A layer that ends in no Extend node has its path written onto its whole output, which gives the editor its click targets
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

	// A layer that is itself an Extend node has its path written, from beside it, onto what it's given to add
	#[tokio::test]
	async fn an_extend_node_shown_as_a_layer_is_traced_back_to() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let extend = editor.create_node_by_name_at(DefinitionIdentifier::ProtoNode(extend::IDENTIFIER), 0, 0).await;
		let rectangle = DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::rectangle::IDENTIFIER);
		let (below, added) = (editor.create_node_by_name_at(rectangle.clone(), -10, 0).await, editor.create_node_by_name_at(rectangle, -10, 4).await);
		for (rectangle, input) in [(below, InputConnector::node(extend, extend::BaseInput)), (added, InputConnector::node(extend, extend::NewInput))] {
			editor
				.handle_message(NodeGraphMessage::CreateWire {
					output_connector: OutputConnector::node(rectangle, 0),
					input_connector: input,
				})
				.await;
		}
		editor
			.handle_message(NodeGraphMessage::CreateWire {
				output_connector: OutputConnector::node(extend, 0),
				input_connector: InputConnector::Export(0),
			})
			.await;

		editor.handle_message(NodeGraphMessage::SetToNodeOrLayer { node_id: extend, is_layer: true }).await;

		let document = editor.active_document();
		let layer = LayerNodeIdentifier::new(extend, &document.network_interface);
		let click_targets = document.metadata().click_targets(layer);
		assert!(
			click_targets.is_some_and(|click_targets| !click_targets.is_empty()),
			"An Extend node shown as a layer should have click targets for what it adds"
		);
	}

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
}

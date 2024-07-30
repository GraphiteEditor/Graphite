use super::transform_utils;
use crate::messages::portfolio::document::node_graph::document_node_types::resolve_document_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use crate::messages::portfolio::document::utility_types::nodes::SelectedNodes;
use crate::messages::prelude::*;

use bezier_rs::Subpath;
use graph_craft::concrete;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{generate_uuid, DocumentNode, DocumentNodeImplementation, NodeId, NodeInput, NodeNetwork, Previewing};
use graphene_core::raster::{BlendMode, ImageFrame};
use graphene_core::text::Font;
use graphene_core::vector::brush_stroke::BrushStroke;
use graphene_core::vector::style::{Fill, Stroke};
use graphene_core::vector::{PointId, VectorModificationType};
use graphene_core::{Artboard, Color, Type};
use interpreted_executor::dynamic_executor::ResolvedDocumentNodeTypes;
use interpreted_executor::node_registry::NODE_REGISTRY;

use glam::{DAffine2, DVec2, IVec2};
use std::hash::{DefaultHasher, Hash, Hasher};

#[derive(PartialEq, Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub enum TransformIn {
	Local,
	Scope { scope: DAffine2 },
	Viewport,
}

// TODO: This is helpful to prevent passing the same arguments to multiple functions, but is currently inefficient due to the collect_outwards_wires. Move it into a function and use only when needed.
/// NodeGraphMessage or GraphOperationMessage cannot be added in ModifyInputsContext, since the functions are called by both messages handlers
pub struct ModifyInputsContext<'a> {
	pub document_metadata: &'a mut DocumentMetadata,
	pub document_network: &'a mut NodeNetwork,
	pub node_graph: &'a mut NodeGraphMessageHandler,
	pub responses: &'a mut VecDeque<Message>,
	pub outwards_wires: HashMap<NodeId, Vec<NodeId>>,
	pub layer_node: Option<NodeId>,
}

impl<'a> ModifyInputsContext<'a> {
	/// Get the node network from the document
	pub fn new(document_network: &'a mut NodeNetwork, document_metadata: &'a mut DocumentMetadata, node_graph: &'a mut NodeGraphMessageHandler, responses: &'a mut VecDeque<Message>) -> Self {
		Self {
			outwards_wires: document_network.collect_outwards_wires(),
			document_network,
			node_graph,
			responses,
			layer_node: None,
			document_metadata,
		}
	}

	pub fn new_with_layer(
		id: NodeId,
		document_network: &'a mut NodeNetwork,
		document_metadata: &'a mut DocumentMetadata,
		node_graph: &'a mut NodeGraphMessageHandler,
		responses: &'a mut VecDeque<Message>,
	) -> Option<Self> {
		let mut document = Self::new(document_network, document_metadata, node_graph, responses);

		let mut id = id;
		while !document.document_network.nodes.get(&id)?.is_layer {
			id = document.outwards_wires.get(&id)?.first().copied()?;
		}

		document.layer_node = Some(id);
		Some(document)
	}

	/// Updates the input of an existing node
	pub fn modify_existing_node_inputs(&mut self, node_id: NodeId, update_input: impl FnOnce(&mut Vec<NodeInput>, NodeId, &DocumentMetadata)) {
		let document_node = self.document_network.nodes.get_mut(&node_id).unwrap();
		update_input(&mut document_node.inputs, node_id, self.document_metadata);
	}

	#[allow(clippy::too_many_arguments)]
	pub fn insert_between(
		node_graph: &mut NodeGraphMessageHandler,
		document_network: &mut NodeNetwork,
		id: NodeId,
		mut new_node: DocumentNode,
		new_node_input: NodeInput,
		new_node_input_index: usize,
		post_node_id: NodeId,
		post_node_input: NodeInput,
		post_node_input_index: usize,
		shift_upstream: IVec2,
	) -> Option<NodeId> {
		assert!(!document_network.nodes.contains_key(&id), "Creating already existing node");
		let pre_node = document_network.nodes.get_mut(&new_node_input.as_node().expect("Input should reference a node"))?;
		new_node.metadata.position = pre_node.metadata.position;

		let post_node = document_network.nodes.get_mut(&post_node_id)?;
		new_node.inputs[new_node_input_index] = new_node_input;
		post_node.inputs[post_node_input_index] = post_node_input;

		node_graph.insert_node(id, new_node, document_network, &Vec::new());

		ModifyInputsContext::shift_upstream(node_graph, document_network, &Vec::new(), id, shift_upstream, false);

		Some(id)
	}

	pub fn insert_node_before(
		node_graph: &mut NodeGraphMessageHandler,
		document_network: &mut NodeNetwork,
		new_id: NodeId,
		node_id: NodeId,
		input_index: usize,
		mut document_node: DocumentNode,
		offset: IVec2,
	) -> Option<NodeId> {
		assert!(!document_network.nodes.contains_key(&new_id), "Creating already existing node");

		let post_node = document_network.nodes.get_mut(&node_id)?;
		post_node.inputs[input_index] = NodeInput::node(new_id, 0);
		document_node.metadata.position = post_node.metadata.position + offset;
		node_graph.insert_node(new_id, document_node, document_network, &Vec::new());

		Some(new_id)
	}

	/// Inserts a node as an export. If there is already a root node connected to the export, that node will be connected to the new node at node_input_index
	pub fn insert_node_as_primary_export(node_graph: &mut NodeGraphMessageHandler, document_network: &mut NodeNetwork, id: NodeId, mut new_node: DocumentNode) -> Option<NodeId> {
		assert!(!document_network.nodes.contains_key(&id), "Creating already existing node");

		if let Some(root_node) = document_network.get_root_node() {
			let previous_root_node = document_network.nodes.get_mut(&root_node.id).expect("Root node should always exist");

			// Insert whatever non artboard node previously fed into export as a child of the new node
			let node_input_index = if new_node.is_artboard() && !previous_root_node.is_artboard() { 1 } else { 0 };
			new_node.inputs[node_input_index] = NodeInput::node(root_node.id, root_node.output_index);
			ModifyInputsContext::shift_upstream(node_graph, document_network, &Vec::new(), root_node.id, IVec2::new(8, 0), true);
		}

		let Some(export) = document_network.exports.get_mut(0) else {
			log::error!("Could not get primary export when adding node");
			return None;
		};
		*export = NodeInput::node(id, 0);

		node_graph.insert_node(id, new_node, document_network, &Vec::new());

		ModifyInputsContext::shift_upstream(node_graph, document_network, &Vec::new(), id, IVec2::new(-8, 3), false);

		Some(id)
	}

	/// Starts at any folder, or the output, and skips layer nodes based on insert_index. Non layer nodes are always skipped. Returns the post node id, pre node id, and the input index.
	///       -----> Post node input_index: 0
	///      |      if skip_layer_nodes == 0, return (Post node, Some(Layer1), 1)
	/// -> Layer1   input_index: 1
	///      ↑      if skip_layer_nodes == 1, return (Layer1, Some(Layer2), 0)
	/// -> Layer2   input_index: 2
	///      ↑
	/// -> NonLayerNode
	///      ↑      if skip_layer_nodes == 2, return (NonLayerNode, Some(Layer3), 0)
	/// -> Layer3   input_index: 3
	///             if skip_layer_nodes == 3, return (Layer3, None, 0)
	pub fn get_post_node_with_index(network: &NodeNetwork, parent: LayerNodeIdentifier, insert_index: usize) -> (Option<NodeId>, Option<NodeId>, usize) {
		let post_node_information = if parent != LayerNodeIdentifier::ROOT_PARENT {
			Some((parent.to_node(), 1))
		} else {
			network.get_root_node().map(|root_node| (root_node.id, 0))
		};

		let Some((mut post_node_id, mut post_node_input_index)) = post_node_information else {
			return (None, None, 0);
		};

		// Skip layers based on skip_layer_nodes, which inserts the new layer at a certain index of the layer stack.
		let mut current_index = 0;

		if parent == LayerNodeIdentifier::ROOT_PARENT {
			if insert_index == 0 {
				return (None, Some(post_node_id), 0);
			}
			current_index += 1;
		}

		loop {
			if current_index == insert_index {
				break;
			}
			let next_node_in_stack_id = network
				.nodes
				.get(&post_node_id)
				.expect("Post node should always exist")
				.inputs
				.get(post_node_input_index)
				.and_then(|input| if let NodeInput::Node { node_id, .. } = input { Some(*node_id) } else { None });

			if let Some(next_node_in_stack_id) = next_node_in_stack_id {
				// Only increment index for layer nodes
				let next_node_in_stack = network.nodes.get(&next_node_in_stack_id).expect("Stack node should always exist");
				if next_node_in_stack.is_layer {
					current_index += 1;
				}

				post_node_id = next_node_in_stack_id;

				// Input as a sibling to the Layer node above
				post_node_input_index = 0;
			} else {
				log::error!("Error creating layer: insert_index out of bounds");
				break;
			};
		}

		// Move post_node to the end of the non layer chain that feeds into post_node, such that pre_node is the layer node at index 1 + insert_index
		let mut post_node = network.nodes.get(&post_node_id).expect("Post node should always exist");
		let mut pre_node_id = post_node
			.inputs
			.get(post_node_input_index)
			.and_then(|input| if let NodeInput::Node { node_id, .. } = input { Some(*node_id) } else { None });

		// Skip until pre_node is either a layer or does not exist
		while let Some(pre_node_id_value) = pre_node_id {
			let pre_node = network.nodes.get(&pre_node_id_value).expect("pre_node_id should be a layer");
			if !pre_node.is_layer {
				post_node = pre_node;
				post_node_id = pre_node_id_value;
				pre_node_id = post_node
					.inputs
					.first()
					.and_then(|input| if let NodeInput::Node { node_id, .. } = input { Some(*node_id) } else { None });
				post_node_input_index = 0;
			} else {
				break;
			}
		}

		(Some(post_node_id), pre_node_id, post_node_input_index)
	}

	pub fn create_layer(&mut self, new_id: NodeId, parent: LayerNodeIdentifier, insert_index: isize) -> Option<NodeId> {
		let skip_layer_nodes = if insert_index < 0 { (-1 - insert_index) as usize } else { insert_index as usize };

		assert!(!self.document_network.nodes.contains_key(&new_id), "Creating already existing layer");
		// TODO: Smarter placement of layers into artboards https://github.com/GraphiteEditor/Graphite/issues/1507

		let mut parent = parent;
		if parent == LayerNodeIdentifier::ROOT_PARENT {
			if let Some(root_node) = self.document_network.get_root_node() {
				// If the current root node is the artboard, then the new layer should be a child of the artboard
				let current_root_node = self.document_network.nodes.get(&root_node.id).expect("Root node should always exist");
				if current_root_node.is_artboard() && current_root_node.is_layer {
					parent = LayerNodeIdentifier::new(root_node.id, self.document_network);
				}
			}
		}

		let new_layer_node = resolve_document_node_type("Merge").expect("Merge node").default_document_node();
		let (post_node_id, pre_node_id, post_node_input_index) = ModifyInputsContext::get_post_node_with_index(self.document_network, parent, skip_layer_nodes);

		if let Some(post_node_id) = post_node_id {
			if let Some(pre_node_id) = pre_node_id {
				ModifyInputsContext::insert_between(
					self.node_graph,
					self.document_network,
					new_id,
					new_layer_node,
					NodeInput::node(pre_node_id, 0),
					0, // pre_node is a sibling so it connects to the first input
					post_node_id,
					NodeInput::node(new_id, 0),
					post_node_input_index,
					IVec2::new(0, 3),
				);
			} else {
				let offset = if post_node_input_index == 1 { IVec2::new(-8, 3) } else { IVec2::new(0, 3) };
				ModifyInputsContext::insert_node_before(self.node_graph, self.document_network, new_id, post_node_id, post_node_input_index, new_layer_node, offset);
			};
		} else {
			// If post_node does not exist, then network is empty
			ModifyInputsContext::insert_node_as_primary_export(self.node_graph, self.document_network, new_id, new_layer_node);
		}

		Some(new_id)
	}

	/// Creates an artboard that outputs to the output node.
	pub fn create_artboard(node_graph: &mut NodeGraphMessageHandler, document_network: &mut NodeNetwork, new_id: NodeId, artboard: Artboard) -> Option<NodeId> {
		let artboard_node = resolve_document_node_type("Artboard").expect("Node").to_document_node_default_inputs(
			[
				Some(NodeInput::value(TaggedValue::ArtboardGroup(graphene_std::ArtboardGroup::EMPTY), true)),
				Some(NodeInput::value(TaggedValue::GraphicGroup(graphene_core::GraphicGroup::EMPTY), true)),
				Some(NodeInput::value(TaggedValue::IVec2(artboard.location), false)),
				Some(NodeInput::value(TaggedValue::IVec2(artboard.dimensions), false)),
				Some(NodeInput::value(TaggedValue::Color(artboard.background), false)),
				Some(NodeInput::value(TaggedValue::Bool(artboard.clip), false)),
			],
			Default::default(),
		);

		ModifyInputsContext::insert_node_as_primary_export(node_graph, document_network, new_id, artboard_node)
	}

	pub fn insert_vector_data(&mut self, subpaths: Vec<Subpath<PointId>>, layer: NodeId) {
		let shape = {
			let node_type: &crate::messages::portfolio::document::node_graph::document_node_types::DocumentNodeDefinition = resolve_document_node_type("Shape").expect("Shape node does not exist");
			node_type.to_document_node_default_inputs([Some(NodeInput::value(TaggedValue::Subpaths(subpaths), false))], Default::default())
		};
		let transform = resolve_document_node_type("Transform").expect("Transform node does not exist").default_document_node();
		let fill = resolve_document_node_type("Fill").expect("Fill node does not exist").default_document_node();
		let stroke = resolve_document_node_type("Stroke").expect("Stroke node does not exist").default_document_node();

		let stroke_id = NodeId(generate_uuid());
		ModifyInputsContext::insert_node_before(self.node_graph, self.document_network, stroke_id, layer, 1, stroke, IVec2::new(-7, 0));
		let fill_id = NodeId(generate_uuid());
		ModifyInputsContext::insert_node_before(self.node_graph, self.document_network, fill_id, stroke_id, 0, fill, IVec2::new(-6, 0));
		let transform_id = NodeId(generate_uuid());
		ModifyInputsContext::insert_node_before(self.node_graph, self.document_network, transform_id, fill_id, 0, transform, IVec2::new(-6, 0));
		let shape_id = NodeId(generate_uuid());
		ModifyInputsContext::insert_node_before(self.node_graph, self.document_network, shape_id, transform_id, 0, shape, IVec2::new(-6, 0));
		self.responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	pub fn insert_text(&mut self, text: String, font: Font, size: f64, layer: NodeId) {
		let text = resolve_document_node_type("Text").expect("Text node does not exist").to_document_node(
			[
				NodeInput::scope("editor-api"),
				NodeInput::value(TaggedValue::String(text), false),
				NodeInput::value(TaggedValue::Font(font), false),
				NodeInput::value(TaggedValue::F64(size), false),
			],
			Default::default(),
		);
		let transform = resolve_document_node_type("Transform").expect("Transform node does not exist").default_document_node();
		let fill = resolve_document_node_type("Fill").expect("Fill node does not exist").default_document_node();
		let stroke = resolve_document_node_type("Stroke").expect("Stroke node does not exist").default_document_node();

		let stroke_id = NodeId(generate_uuid());
		ModifyInputsContext::insert_node_before(self.node_graph, self.document_network, stroke_id, layer, 1, stroke, IVec2::new(-7, 0));
		let fill_id = NodeId(generate_uuid());
		ModifyInputsContext::insert_node_before(self.node_graph, self.document_network, fill_id, stroke_id, 0, fill, IVec2::new(-6, 0));
		let transform_id = NodeId(generate_uuid());
		ModifyInputsContext::insert_node_before(self.node_graph, self.document_network, transform_id, fill_id, 0, transform, IVec2::new(-6, 0));
		let text_id = NodeId(generate_uuid());
		ModifyInputsContext::insert_node_before(self.node_graph, self.document_network, text_id, transform_id, 0, text, IVec2::new(-6, 0));
		self.responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	pub fn insert_image_data(node_graph: &mut NodeGraphMessageHandler, document_network: &mut NodeNetwork, image_frame: ImageFrame<Color>, layer: NodeId, responses: &mut VecDeque<Message>) {
		let image = {
			let node_type = resolve_document_node_type("Image").expect("Image node does not exist");
			node_type.to_document_node_default_inputs([Some(NodeInput::value(TaggedValue::ImageFrame(image_frame), false))], Default::default())
		};
		let transform = resolve_document_node_type("Transform").expect("Transform node does not exist").default_document_node();

		let transform_id = NodeId(generate_uuid());
		ModifyInputsContext::insert_node_before(node_graph, document_network, transform_id, layer, 1, transform, IVec2::new(-6, 0));

		let image_id = NodeId(generate_uuid());
		ModifyInputsContext::insert_node_before(node_graph, document_network, image_id, transform_id, 0, image, IVec2::new(-5, 0));

		responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	pub fn shift_upstream(node_graph: &mut NodeGraphMessageHandler, document_network: &mut NodeNetwork, network_path: &[NodeId], node_id: NodeId, shift: IVec2, shift_self: bool) {
		let Some(network) = document_network.nested_network(network_path) else {
			log::error!("Could not get nested network for shift_upstream");
			return;
		};

		let mut shift_nodes = HashSet::new();
		if shift_self {
			shift_nodes.insert(node_id);
		}

		let mut stack = vec![node_id];
		while let Some(node_id) = stack.pop() {
			let Some(node) = network.nodes.get(&node_id) else { continue };
			for input in &node.inputs {
				let NodeInput::Node { node_id, .. } = input else { continue };
				if shift_nodes.insert(*node_id) {
					stack.push(*node_id);
				}
			}
		}

		for node_id in shift_nodes {
			if let Some(node) = document_network.nodes.get_mut(&node_id) {
				node.metadata.position += shift;
				node_graph.update_click_target(node_id, document_network, network_path.to_owned());
			}
		}
	}

	/// Inserts a new node and modifies the inputs
	pub fn modify_new_node(&mut self, name: &'static str, update_input: impl FnOnce(&mut Vec<NodeInput>, NodeId, &DocumentMetadata)) {
		let output_node_id = self.layer_node.or_else(|| {
			if let Some(NodeInput::Node { node_id, .. }) = self.document_network.exports.first() {
				Some(*node_id)
			} else {
				log::error!("Could not modify new node with empty network");
				None
			}
		});
		let Some(output_node_id) = output_node_id else {
			warn!("Output node id doesn't exist");
			return;
		};

		let Some(output_node) = self.document_network.nodes.get_mut(&output_node_id) else {
			warn!("Output node doesn't exist");
			return;
		};

		let input_index = if output_node.is_layer { 1 } else { 0 };
		let metadata = output_node.metadata.clone();
		let new_input = output_node.inputs.get(input_index).cloned().filter(|input| input.as_node().is_some());
		let node_id = NodeId(generate_uuid());

		output_node.inputs[input_index] = NodeInput::node(node_id, 0);

		let Some(node_type) = resolve_document_node_type(name) else {
			warn!("Node type \"{name}\" doesn't exist");
			return;
		};
		let mut new_document_node = node_type.to_document_node_default_inputs([new_input], metadata);
		update_input(&mut new_document_node.inputs, node_id, self.document_metadata);
		self.node_graph.insert_node(node_id, new_document_node, self.document_network, &Vec::new());

		let upstream_nodes = self
			.document_network
			.upstream_flow_back_from_nodes(vec![node_id], graph_craft::document::FlowType::HorizontalFlow)
			.map(|(_, id)| id)
			.collect::<Vec<_>>();
		for node_id in upstream_nodes {
			let Some(node) = self.document_network.nodes.get_mut(&node_id) else { continue };
			node.metadata.position.x -= 8;
			self.node_graph.update_click_target(node_id, self.document_network, Vec::new());
		}
	}

	/// Find a node id as part of the layer
	fn existing_node_id(&mut self, name: &'static str) -> Option<NodeId> {
		// Start from the layer node or export
		let node_ids = self
			.layer_node
			.map_or_else(|| self.document_network.exports.iter().filter_map(graph_craft::document::NodeInput::as_node).collect(), |id| vec![id]);
		let upstream = self.document_network.upstream_flow_back_from_nodes(node_ids, graph_craft::document::FlowType::HorizontalFlow);

		// Take until another layer node is found (but not the first layer node)
		let is_input = |node_id: NodeId| self.layer_node == Some(node_id) || self.document_network.exports.iter().any(|export| export.as_node() == Some(node_id));
		let mut upstream_until_layer = upstream.take_while(|&(node, id)| is_input(id) || !node.is_layer);

		upstream_until_layer.find(|(node, _)| node.name == name).map(|(_, id)| id)
	}

	/// Changes the input of a specific node; skipping if it doesn't exist
	pub fn modify_existing_inputs(&mut self, name: &'static str, update_input: impl FnOnce(&mut Vec<NodeInput>, NodeId, &DocumentMetadata)) {
		let existing_node_id = self.existing_node_id(name);
		if let Some(node_id) = existing_node_id {
			self.modify_existing_node_inputs(node_id, update_input);
		}
	}

	/// Changes the inputs of a specific node; creating it if it doesn't exist
	pub fn modify_inputs(&mut self, name: &'static str, skip_rerender: bool, update_input: impl FnOnce(&mut Vec<NodeInput>, NodeId, &DocumentMetadata)) {
		let existing_node_id = self.existing_node_id(name);
		if let Some(node_id) = existing_node_id {
			self.modify_existing_node_inputs(node_id, update_input);
		} else {
			self.modify_new_node(name, update_input);
		}

		self.responses.add(PropertiesPanelMessage::Refresh);

		if !skip_rerender {
			self.responses.add(NodeGraphMessage::RunDocumentGraph);
		}
	}

	/// Changes the inputs of a all of the existing instances of a node name
	pub fn modify_all_node_inputs(&mut self, name: &'static str, skip_rerender: bool, mut update_input: impl FnMut(&mut Vec<NodeInput>, NodeId, &DocumentMetadata)) {
		let existing_nodes: Vec<_> = self
			.document_network
			.upstream_flow_back_from_nodes(
				self.layer_node.map_or_else(
					|| {
						self.document_network
							.exports
							.iter()
							.filter_map(|output| if let NodeInput::Node { node_id, .. } = output { Some(*node_id) } else { None })
							.collect()
					},
					|id| vec![id],
				),
				graph_craft::document::FlowType::HorizontalFlow,
			)
			.filter(|(node, _)| node.name == name)
			.map(|(_, id)| id)
			.collect();
		for existing_node_id in existing_nodes {
			self.modify_existing_node_inputs(existing_node_id, &mut update_input);
		}

		self.responses.add(PropertiesPanelMessage::Refresh);

		if !skip_rerender {
			self.responses.add(NodeGraphMessage::RunDocumentGraph);
		} else {
			// Code was removed from here which cleared the frame
		}
	}

	/// Returns true if the network structure is updated
	pub fn set_input(
		node_graph: &mut NodeGraphMessageHandler,
		document_network: &mut NodeNetwork,
		network_path: &[NodeId],
		node_id: NodeId,
		input_index: usize,
		input: NodeInput,
		is_document_network: bool,
	) -> bool {
		let Some(network) = document_network.nested_network_mut(network_path) else {
			log::error!("Could not get nested network for set_input");
			return false;
		};
		if let Some(node) = network.nodes.get_mut(&node_id) {
			let Some(node_input) = node.inputs.get_mut(input_index) else {
				log::error!("Tried to set input {input_index} to {input:?}, but the index was invalid. Node {node_id}:\n{node:#?}");
				return false;
			};
			let structure_changed = node_input.as_node().is_some() || input.as_node().is_some();

			let previously_exposed = node_input.is_exposed();
			*node_input = input;
			let currently_exposed = node_input.is_exposed();
			if previously_exposed != currently_exposed {
				node_graph.update_click_target(node_id, document_network, network_path.to_owned());
			}

			// Only load network structure for changes to document_network
			structure_changed && is_document_network
		} else if node_id == network.exports_metadata.0 {
			let Some(export) = network.exports.get_mut(input_index) else {
				log::error!("Tried to set export {input_index} to {input:?}, but the index was invalid. Network:\n{network:#?}");
				return false;
			};

			let previously_exposed = export.is_exposed();
			*export = input;
			let currently_exposed = export.is_exposed();

			if let NodeInput::Node { node_id, output_index, .. } = *export {
				network.update_root_node(node_id, output_index);
			} else if let NodeInput::Value { .. } = *export {
				if input_index == 0 {
					network.stop_preview();
				}
			} else {
				log::error!("Network export input not supported");
			}

			if previously_exposed != currently_exposed {
				node_graph.update_click_target(node_id, document_network, network_path.to_owned());
			}

			// Only load network structure for changes to document_network
			is_document_network
		} else {
			false
		}
	}

	pub fn fill_set(&mut self, fill: Fill) {
		let fill_index = 1;
		let backup_color_index = 2;
		let backup_gradient_index = 3;

		self.modify_inputs("Fill", false, |inputs, _node_id, _metadata| {
			match &fill {
				Fill::None => {
					inputs[backup_color_index] = NodeInput::value(TaggedValue::OptionalColor(None), false);
				}
				Fill::Solid(color) => {
					inputs[backup_color_index] = NodeInput::value(TaggedValue::OptionalColor(Some(*color)), false);
				}
				Fill::Gradient(gradient) => {
					inputs[backup_gradient_index] = NodeInput::value(TaggedValue::Gradient(gradient.clone()), false);
				}
			}

			inputs[fill_index] = NodeInput::value(TaggedValue::Fill(fill), false);
		});
	}

	pub fn opacity_set(&mut self, opacity: f64) {
		self.modify_inputs("Opacity", false, |inputs, _node_id, _metadata| {
			inputs[1] = NodeInput::value(TaggedValue::F64(opacity * 100.), false);
		});
	}

	pub fn blend_mode_set(&mut self, blend_mode: BlendMode) {
		self.modify_inputs("Blend Mode", false, |inputs, _node_id, _metadata| {
			inputs[1] = NodeInput::value(TaggedValue::BlendMode(blend_mode), false);
		});
	}

	pub fn stroke_set(&mut self, stroke: Stroke) {
		self.modify_inputs("Stroke", false, |inputs, _node_id, _metadata| {
			inputs[1] = NodeInput::value(TaggedValue::OptionalColor(stroke.color), false);
			inputs[2] = NodeInput::value(TaggedValue::F64(stroke.weight), false);
			inputs[3] = NodeInput::value(TaggedValue::VecF64(stroke.dash_lengths), false);
			inputs[4] = NodeInput::value(TaggedValue::F64(stroke.dash_offset), false);
			inputs[5] = NodeInput::value(TaggedValue::LineCap(stroke.line_cap), false);
			inputs[6] = NodeInput::value(TaggedValue::LineJoin(stroke.line_join), false);
			inputs[7] = NodeInput::value(TaggedValue::F64(stroke.line_join_miter_limit), false);
		});
	}

	pub fn transform_change(&mut self, transform: DAffine2, transform_in: TransformIn, parent_transform: DAffine2, skip_rerender: bool) {
		self.modify_inputs("Transform", skip_rerender, |inputs, _node_id, _metadata| {
			let layer_transform = transform_utils::get_current_transform(inputs);
			let to = match transform_in {
				TransformIn::Local => DAffine2::IDENTITY,
				TransformIn::Scope { scope } => scope * parent_transform,
				TransformIn::Viewport => parent_transform,
			};
			let transform = to.inverse() * transform * to * layer_transform;
			transform_utils::update_transform(inputs, transform);
		});
	}

	pub fn transform_set(&mut self, mut transform: DAffine2, transform_in: TransformIn, parent_transform: DAffine2, current_transform: Option<DAffine2>, skip_rerender: bool) {
		self.modify_inputs("Transform", skip_rerender, |inputs, node_id, metadata| {
			let upstream_transform = metadata.upstream_transform(node_id);

			let to = match transform_in {
				TransformIn::Local => DAffine2::IDENTITY,
				TransformIn::Scope { scope } => scope * parent_transform,
				TransformIn::Viewport => parent_transform,
			};

			if current_transform
				.filter(|transform| transform.matrix2.determinant() != 0. && upstream_transform.matrix2.determinant() != 0.)
				.is_some()
			{
				transform *= upstream_transform.inverse();
			}
			let final_transform = to.inverse() * transform;
			transform_utils::update_transform(inputs, final_transform);
		});
	}

	pub fn pivot_set(&mut self, new_pivot: DVec2) {
		self.modify_inputs("Transform", false, |inputs, _node_id, _metadata| {
			inputs[5] = NodeInput::value(TaggedValue::DVec2(new_pivot), false);
		});
	}

	pub fn vector_modify(&mut self, modification_type: VectorModificationType) {
		self.modify_inputs("Path", false, |inputs, _node_id, _metadata| {
			let Some(NodeInput::Value { tagged_value, .. }) = inputs.iter_mut().skip(1).next() else {
				panic!("Path node does not have modification input");
			};
			let TaggedValue::VectorModification(modification) = &mut *tagged_value.inner_mut() else {
				panic!("Path node does not have modification input");
			};
			modification.modify(&modification_type);
		});
	}

	pub fn brush_modify(&mut self, strokes: Vec<BrushStroke>) {
		self.modify_inputs("Brush", false, |inputs, _node_id, _metadata| {
			inputs[2] = NodeInput::value(TaggedValue::BrushStrokes(strokes), false);
		});
	}

	pub fn resize_artboard(&mut self, location: IVec2, dimensions: IVec2) {
		self.modify_inputs("Artboard", false, |inputs, _node_id, _metadata| {
			let mut dimensions = dimensions;
			let mut location = location;

			if dimensions.x < 0 {
				dimensions.x *= -1;
				location.x -= dimensions.x;
			}
			if dimensions.y < 0 {
				dimensions.y *= -1;
				location.y -= dimensions.y;
			}

			inputs[2] = NodeInput::value(TaggedValue::IVec2(location), false);
			inputs[3] = NodeInput::value(TaggedValue::IVec2(dimensions), false);
		});
	}

	/// Deletes all nodes in `node_ids` and any sole dependents in the horizontal chain if the node to delete is a layer node.
	pub fn delete_nodes(
		node_graph: &mut NodeGraphMessageHandler,
		document_network: &mut NodeNetwork,
		selected_nodes: &mut SelectedNodes,
		node_ids: Vec<NodeId>,
		reconnect: bool,
		responses: &mut VecDeque<Message>,
		network_path: Vec<NodeId>,
	) {
		let Some(network) = document_network.nested_network_for_selected_nodes(&network_path, selected_nodes.selected_nodes_ref().iter()) else {
			return;
		};
		let mut delete_nodes = HashSet::new();

		for node_id in &node_ids {
			delete_nodes.insert(*node_id);

			if !reconnect {
				continue;
			};
			let Some(node) = network.nodes.get(node_id) else {
				continue;
			};
			let child_id = node.inputs.get(1).and_then(|input| if let NodeInput::Node { node_id, .. } = input { Some(node_id) } else { None });
			let Some(child_id) = child_id else {
				continue;
			};

			let outward_wires = network.collect_outwards_wires();

			for (_, upstream_id) in network.upstream_flow_back_from_nodes(vec![*child_id], graph_craft::document::FlowType::UpstreamFlow) {
				// This does a downstream traversal starting from the current node, and ending at either a node in the `delete_nodes` set or the output.
				// If the traversal find as child node of a node in the `delete_nodes` set, then it is a sole dependent. If the output node is eventually reached, then it is not a sole dependent.
				let mut stack = vec![upstream_id];
				let mut can_delete = true;

				while let Some(current_node) = stack.pop() {
					let Some(downstream_nodes) = outward_wires.get(&current_node) else { continue };
					for downstream_node in downstream_nodes {
						// If the traversal reaches the root node, and the root node should not be deleted, then the current node is not a sole dependent
						if network
							.get_root_node()
							.is_some_and(|root_node| root_node.id == *downstream_node && !delete_nodes.contains(&root_node.id))
						{
							can_delete = false;
						} else if !delete_nodes.contains(downstream_node) {
							stack.push(*downstream_node);
						}
						// Continue traversing over the downstream sibling, which happens if the current node is a sibling to a node in node_ids
						else {
							for deleted_node_id in &node_ids {
								let Some(output_node) = network.nodes.get(deleted_node_id) else { continue };
								let Some(input) = output_node.inputs.first() else { continue };

								if let NodeInput::Node { node_id, .. } = input {
									if *node_id == current_node {
										stack.push(*deleted_node_id);
									}
								}
							}
						}
					}
				}
				if can_delete {
					delete_nodes.insert(upstream_id);
				}
			}
		}

		let network_path = if selected_nodes
			.selected_nodes_ref()
			.iter()
			.any(|node_id| document_network.nodes.contains_key(node_id) || document_network.exports_metadata.0 == *node_id || document_network.imports_metadata.0 == *node_id)
		{
			Vec::new()
		} else {
			network_path.clone()
		};

		selected_nodes.add_selected_nodes(delete_nodes.iter().cloned().collect(), document_network, &network_path);

		for delete_node_id in delete_nodes {
			ModifyInputsContext::remove_node(node_graph, document_network, selected_nodes, delete_node_id, reconnect, responses, &network_path);
		}
	}

	/// Tries to remove a node from the network, returning `true` on success.
	fn remove_node(
		node_graph: &mut NodeGraphMessageHandler,
		document_network: &mut NodeNetwork,
		selected_nodes: &mut SelectedNodes,
		node_id: NodeId,
		reconnect: bool,
		responses: &mut VecDeque<Message>,
		network_path: &[NodeId],
	) -> bool {
		if !ModifyInputsContext::remove_references_from_network(node_graph, document_network, node_id, reconnect, network_path) {
			log::error!("could not remove_references_from_network");
			return false;
		}
		let Some(network) = document_network.nested_network_mut(network_path) else { return false };

		network.nodes.remove(&node_id);
		selected_nodes.retain_selected_nodes(|&id| id != node_id || id == network.exports_metadata.0 || id == network.imports_metadata.0);
		node_graph.update_click_target(node_id, document_network, network_path.to_owned());

		responses.add(BroadcastEvent::SelectionChanged);

		true
	}

	pub fn remove_references_from_network(node_graph: &mut NodeGraphMessageHandler, document_network: &mut NodeNetwork, deleting_node_id: NodeId, reconnect: bool, network_path: &[NodeId]) -> bool {
		let Some(network) = document_network.nested_network(network_path) else { return false };
		let mut reconnect_to_input: Option<NodeInput> = None;

		if reconnect {
			// Check whether the being-deleted node's first (primary) input is a node
			if let Some(node) = network.nodes.get(&deleting_node_id) {
				// Reconnect to the node below when deleting a layer node.
				if matches!(&node.inputs.first(), Some(NodeInput::Node { .. })) || matches!(&node.inputs.first(), Some(NodeInput::Network { .. })) {
					reconnect_to_input = Some(node.inputs[0].clone());
				}
			}
		}

		let mut nodes_to_set_input = Vec::new();

		// Boolean flag if the downstream input can be reconnected to the upstream node
		let mut can_reconnect = true;

		for (node_id, input_index, input) in network
			.nodes
			.iter()
			.filter_map(|(node_id, node)| {
				if *node_id == deleting_node_id {
					None
				} else {
					Some(node.inputs.iter().enumerate().map(|(index, input)| (*node_id, index, input)))
				}
			})
			.flatten()
			.chain(network.exports.iter().enumerate().map(|(index, input)| (network.exports_metadata.0, index, input)))
		{
			let NodeInput::Node { node_id: upstream_node_id, .. } = input else { continue };
			if *upstream_node_id != deleting_node_id {
				continue;
			}

			// Do not reconnect export to import until (#1762) is solved
			if node_id == network.exports_metadata.0 && reconnect_to_input.as_ref().is_some_and(|reconnect| matches!(reconnect, NodeInput::Network { .. })) {
				can_reconnect = false;
			}

			// Do not reconnect to EditorApi network input in the document network.
			if network_path.is_empty() && reconnect_to_input.as_ref().is_some_and(|reconnect| matches!(reconnect, NodeInput::Network { .. })) {
				can_reconnect = false;
			}

			// Only reconnect if the output index for the node to be deleted is 0
			if can_reconnect && reconnect_to_input.is_some() {
				// None means to use reconnect_to_input, which can be safely unwrapped
				nodes_to_set_input.push((node_id, input_index, None));

				// Only one node can be reconnected
				can_reconnect = false;
			} else {
				// Disconnect input
				let tagged_value = TaggedValue::from_type(&ModifyInputsContext::get_input_type(document_network, network_path, node_id, &node_graph.resolved_types, input_index));
				let value_input = NodeInput::value(tagged_value, true);
				nodes_to_set_input.push((node_id, input_index, Some(value_input)));
			}
		}

		//let Some(network) = document_network.nested_network(network_path) else { return false };

		if let Some(Previewing::Yes {
			root_node_to_restore: Some(root_node_to_restore),
		}) = document_network.nested_network(network_path).map(|network| &network.previewing)
		{
			if root_node_to_restore.id == deleting_node_id {
				document_network.nested_network_mut(network_path).unwrap().start_previewing_without_restore();
			}
		}

		let is_document_network = network_path.is_empty();
		for (node_id, input_index, value_input) in nodes_to_set_input {
			if let Some(value_input) = value_input {
				// Disconnect input to root node only if not previewing
				if document_network
					.nested_network(network_path)
					.is_some_and(|network| node_id != network.exports_metadata.0 || matches!(&network.previewing, Previewing::No))
				{
					ModifyInputsContext::set_input(node_graph, document_network, network_path, node_id, input_index, value_input, is_document_network);
				} else if let Some(Previewing::Yes { root_node_to_restore }) = document_network.nested_network(network_path).map(|network| &network.previewing) {
					if let Some(root_node) = root_node_to_restore {
						if node_id == root_node.id {
							document_network.nested_network_mut(network_path).unwrap().start_previewing_without_restore();
						} else {
							ModifyInputsContext::set_input(
								node_graph,
								document_network,
								network_path,
								node_id,
								input_index,
								NodeInput::node(root_node.id, root_node.output_index),
								is_document_network,
							);
						}
					} else {
						ModifyInputsContext::set_input(node_graph, document_network, network_path, node_id, input_index, value_input, is_document_network);
					}
				}
			}
			// Reconnect to node upstream of the deleted node
			else if document_network
				.nested_network(network_path)
				.is_some_and(|network| node_id != network.exports_metadata.0 || matches!(network.previewing, Previewing::No))
			{
				if let Some(reconnect_to_input) = reconnect_to_input.clone() {
					ModifyInputsContext::set_input(node_graph, document_network, network_path, node_id, input_index, reconnect_to_input, is_document_network);
				}
			}
			// Reconnect previous root node to the export, or disconnect export
			else if let Some(Previewing::Yes { root_node_to_restore }) = document_network.nested_network(network_path).map(|network| &network.previewing) {
				if let Some(root_node) = root_node_to_restore {
					ModifyInputsContext::set_input(
						node_graph,
						document_network,
						network_path,
						node_id,
						input_index,
						NodeInput::node(root_node.id, root_node.output_index),
						is_document_network,
					);
				} else if let Some(reconnect_to_input) = reconnect_to_input.clone() {
					ModifyInputsContext::set_input(node_graph, document_network, network_path, node_id, input_index, reconnect_to_input, is_document_network);
					document_network.nested_network_mut(network_path).unwrap().start_previewing_without_restore();
				}
			}
		}
		true
	}

	/// Get the [`Type`] for any `node_id` and `input_index`. The `network_path` is the path to the encapsulating node (including the encapsulating node). The `node_id` is the selected node.
	pub fn get_input_type(document_network: &NodeNetwork, network_path: &[NodeId], node_id: NodeId, resolved_types: &ResolvedDocumentNodeTypes, input_index: usize) -> Type {
		let Some(network) = document_network.nested_network(network_path) else {
			log::error!("Could not get network in get_tagged_value");
			return concrete!(());
		};

		// TODO: Store types for all document nodes, not just the compiled proto nodes, which currently skips isolated nodes
		let node_id_path = &[network_path, &[node_id]].concat();
		let input_type = resolved_types.inputs.get(&graph_craft::document::Source {
			node: node_id_path.clone(),
			index: input_index,
		});

		if let Some(input_type) = input_type {
			input_type.clone()
		} else if node_id == network.exports_metadata.0 {
			if let Some(parent_node_id) = network_path.last() {
				let mut parent_path = network_path.to_owned();
				parent_path.pop();

				let parent_node = document_network
					.nested_network(&parent_path)
					.expect("Parent path should always exist")
					.nodes
					.get(parent_node_id)
					.expect("Last path node should always exist in parent network");

				let output_types = NodeGraphMessageHandler::get_output_types(parent_node, resolved_types, network_path);
				output_types.get(input_index).map_or_else(
					|| {
						warn!("Could not find output type for export node {node_id}");
						concrete!(())
					},
					|output_type| output_type.clone().map_or(concrete!(()), |output| output),
				)
			} else {
				concrete!(graphene_core::ArtboardGroup)
			}
		} else {
			// TODO: Once there is type inference (#1621), replace this workaround approach when disconnecting node inputs with NodeInput::Node(ToDefaultNode),
			// TODO: which would be a new node that implements the Default trait (i.e. `Default::default()`)

			// Resolve types from proto nodes in node_registry
			let Some(node) = network.nodes.get(&node_id) else {
				return concrete!(());
			};

			fn get_type_from_node(node: &DocumentNode, input_index: usize) -> Type {
				match &node.implementation {
					DocumentNodeImplementation::ProtoNode(protonode) => {
						let Some(node_io_hashmap) = NODE_REGISTRY.get(protonode) else {
							log::error!("Could not get hashmap for proto node: {protonode:?}");
							return concrete!(());
						};

						let mut all_node_io_types = node_io_hashmap.keys().collect::<Vec<_>>();
						all_node_io_types.sort_by_key(|node_io_types| {
							let mut hasher = DefaultHasher::new();
							node_io_types.hash(&mut hasher);
							hasher.finish()
						});
						let Some(node_types) = all_node_io_types.first() else {
							log::error!("Could not get node_types from hashmap");
							return concrete!(());
						};

						let skip_footprint = if node.manual_composition.is_some() { 1 } else { 0 };

						let Some(input_type) = std::iter::once(node_types.input.clone()).chain(node_types.parameters.clone()).nth(input_index + skip_footprint) else {
							log::error!("Could not get type");
							return concrete!(());
						};

						input_type
					}
					DocumentNodeImplementation::Network(network) => {
						for node in &network.nodes {
							for (network_node_input_index, input) in node.1.inputs.iter().enumerate() {
								if let NodeInput::Network { import_index, .. } = input {
									if *import_index == input_index {
										return get_type_from_node(node.1, network_node_input_index);
									}
								}
							}
						}
						// Input is disconnected
						concrete!(())
					}
					_ => concrete!(()),
				}
			}

			get_type_from_node(node, input_index)
		}
	}
}

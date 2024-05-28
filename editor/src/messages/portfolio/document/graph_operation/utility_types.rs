use crate::messages::portfolio::document::node_graph::document_node_types::resolve_document_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use crate::messages::prelude::*;

use crate::messages::portfolio::document::utility_types::nodes::SelectedNodes;
use bezier_rs::Subpath;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{generate_uuid, DocumentNode, NodeId, NodeInput, NodeNetwork, RootNode};
use graphene_core::raster::{BlendMode, ImageFrame};
use graphene_core::text::Font;
use graphene_core::uuid::ManipulatorGroupId;
use graphene_core::vector::brush_stroke::BrushStroke;
use graphene_core::vector::style::{Fill, FillType, Stroke};
use graphene_core::{Artboard, Color};
use graphene_std::vector::ManipulatorPointId;
use interpreted_executor::dynamic_executor::ResolvedDocumentNodeTypes;

use glam::{DAffine2, DVec2, IVec2};
use graphene_std::ArtboardGroup;

use super::transform_utils::{self, LayerBounds};

#[derive(PartialEq, Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub enum TransformIn {
	Local,
	Scope { scope: DAffine2 },
	Viewport,
}

type ManipulatorGroup = bezier_rs::ManipulatorGroup<ManipulatorGroupId>;

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum VectorDataModification {
	AddEndManipulatorGroup { subpath_index: usize, manipulator_group: ManipulatorGroup },
	AddManipulatorGroup { manipulator_group: ManipulatorGroup, after_id: ManipulatorGroupId },
	AddStartManipulatorGroup { subpath_index: usize, manipulator_group: ManipulatorGroup },
	RemoveManipulatorGroup { id: ManipulatorGroupId },
	RemoveManipulatorPoint { point: ManipulatorPointId },
	SetClosed { index: usize, closed: bool },
	SetManipulatorColinearHandlesState { id: ManipulatorGroupId, colinear: bool },
	SetManipulatorPosition { point: ManipulatorPointId, position: DVec2 },
	ToggleManipulatorColinearHandlesState { id: ManipulatorGroupId },
	UpdateSubpaths { subpaths: Vec<Subpath<ManipulatorGroupId>> },
}

// TODO: Generalize for any network, rewrite as static functions since there only a few fields are used for each function, so when calling only the necessary data will
// be provided
/// NodeGraphMessage or GraphOperationMessage cannot be added in ModifyInputsContext, since the functions are called by both messages handlers
pub struct ModifyInputsContext<'a> {
	pub document_metadata: &'a mut DocumentMetadata,
	pub document_network: &'a mut NodeNetwork,
	pub node_graph: &'a mut NodeGraphMessageHandler,
	pub responses: &'a mut VecDeque<Message>,
	pub outwards_links: HashMap<NodeId, Vec<NodeId>>,
	pub layer_node: Option<NodeId>,
}

impl<'a> ModifyInputsContext<'a> {
	/// Get the node network from the document
	pub fn new(document_network: &'a mut NodeNetwork, document_metadata: &'a mut DocumentMetadata, node_graph: &'a mut NodeGraphMessageHandler, responses: &'a mut VecDeque<Message>) -> Self {
		Self {
			outwards_links: document_network.collect_outwards_links(),
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
			id = document.outwards_links.get(&id)?.first().copied()?;
		}

		document.layer_node = Some(id);
		Some(document)
	}

	/// Updates the input of an existing node
	pub fn modify_existing_node_inputs(&mut self, node_id: NodeId, update_input: impl FnOnce(&mut Vec<NodeInput>, NodeId, &DocumentMetadata)) {
		let document_node = self.document_network.nodes.get_mut(&node_id).unwrap();
		update_input(&mut document_node.inputs, node_id, self.document_metadata);
	}

	pub fn insert_between(
		&mut self,
		id: NodeId,
		mut new_node: DocumentNode,
		new_node_input: NodeInput,
		new_node_input_index: usize,
		post_node_id: NodeId,
		post_node_input: NodeInput,
		post_node_input_index: usize,
		shift_upstream: IVec2,
	) -> Option<NodeId> {
		assert!(!self.document_network.nodes.contains_key(&id), "Creating already existing node");
		let pre_node = self.document_network.nodes.get_mut(&new_node_input.as_node().expect("Input should reference a node"))?;
		new_node.metadata.position = pre_node.metadata.position;

		let post_node = self.document_network.nodes.get_mut(&post_node_id)?;
		new_node.inputs[new_node_input_index] = new_node_input;
		post_node.inputs[post_node_input_index] = post_node_input;

		self.document_network.nodes.insert(id, new_node);

		ModifyInputsContext::shift_upstream(self.document_network, id, shift_upstream, false);

		Some(id)
	}

	pub fn insert_node_before(&mut self, new_id: NodeId, node_id: NodeId, input_index: usize, mut document_node: DocumentNode, offset: IVec2) -> Option<NodeId> {
		assert!(!self.document_network.nodes.contains_key(&new_id), "Creating already existing node");

		let post_node = self.document_network.nodes.get_mut(&node_id)?;
		post_node.inputs[input_index] = NodeInput::node(new_id, 0);
		document_node.metadata.position = post_node.metadata.position + offset;
		self.document_network.nodes.insert(new_id, document_node);

		Some(new_id)
	}

	// Inserts a node as an export. If there is already a root node connected to the export, that node will be connected to the new node at node_input_index
	pub fn insert_node_as_primary_export(&mut self, id: NodeId, mut new_node: DocumentNode, node_input_index: usize) -> Option<NodeId> {
		assert!(!self.document_network.nodes.contains_key(&id), "Creating already existing node");
		let Some(export) = self.document_network.exports.get_mut(0) else {
			log::error!("Could not get primary export when adding node");
			return None;
		};
		*export = NodeInput::node(id, 0);

		if let Some(root_node) = self.document_network.root_node {
			new_node.inputs[node_input_index] = NodeInput::node(root_node.id, 0);
		}

		self.document_network.nodes.insert(id, new_node);
		if let Some(root_node) = self.document_network.root_node.as_mut() {
			root_node.id = id;
		} else {
			self.document_network.root_node = Some(RootNode { id, output_index: 0 });
		}

		ModifyInputsContext::shift_upstream(self.document_network, id, IVec2::new(-8, 3), false);

		Some(id)
	}
	/// Starts at any folder, or the output, and skips layer nodes based on insert_index. Non layer nodes are always skipped. Returns the post node id, pre node id, and the input index.
	///      -----> Post node input_index: 0
	///      |      if skip_layer_nodes == 0, return (Post node, Some(Layer1), 1)
	/// -> Layer1   input_index: 1
	///      ↑      if skip_layer_nodes == 1, return (Layer1, Some(Layer2), 0)
	/// -> Layer2   input_index: 2
	///      ↑
	///	-> NonLayerNode
	///      ↑      if skip_layer_nodes == 2, return (NonLayerNode, Some(Layer3), 0)
	/// -> Layer3   input_index: 3
	///             if skip_layer_nodes == 3, return (Layer3, None, 0)
	pub fn get_post_node_with_index(network: &NodeNetwork, parent: LayerNodeIdentifier, insert_index: usize) -> (Option<NodeId>, Option<NodeId>, usize) {
		let post_node_information = if parent != LayerNodeIdentifier::ROOT_PARENT {
			Some((parent.to_node(), 1))
		} else {
			network.root_node.map(|root_node| (root_node.id, 0))
		};

		let Some((mut post_node_id, mut post_node_input_index)) = post_node_information else {
			return (None, None, 0);
		};
		// Skip layers based on skip_layer_nodes, which inserts the new layer at a certain index of the layer stack.
		let mut current_index = 0;
		loop {
			if current_index == insert_index {
				if parent == LayerNodeIdentifier::ROOT_PARENT {
					post_node_input_index = 1;
				}
				break;
			}
			let next_node_in_stack_id = network
				.nodes
				.get(&post_node_id)
				.expect("Post node should always exist")
				.inputs
				.get(post_node_input_index)
				.and_then(|input| if let NodeInput::Node { node_id, .. } = input { Some(node_id.clone()) } else { None });

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
			.and_then(|input| if let NodeInput::Node { node_id, .. } = input { Some(node_id.clone()) } else { None });

		// Skip until pre_node is either a layer or does not exist
		while let Some(pre_node_id_value) = pre_node_id {
			let pre_node = network.nodes.get(&pre_node_id_value).expect("pre_node_id should be a layer");
			if !pre_node.is_layer {
				post_node = pre_node;
				post_node_id = pre_node_id_value;
				pre_node_id = post_node
					.inputs
					.get(0)
					.and_then(|input| if let NodeInput::Node { node_id, .. } = input { Some(node_id.clone()) } else { None });
				post_node_input_index = 0;
			} else {
				break;
			}
		}

		(Some(post_node_id), pre_node_id, post_node_input_index)
	}

	pub fn create_layer(&mut self, new_id: NodeId, parent: LayerNodeIdentifier, skip_layer_nodes: usize) -> Option<NodeId> {
		assert!(!self.document_network.nodes.contains_key(&new_id), "Creating already existing layer");
		// Get the node which the new layer will output to (post node). First check if the output_node_id is the Output node, and set the output_node_id to the top-most artboard,
		// if there is one. Then skip layers based on skip_layer_nodes from the post_node.
		// TODO: Smarter placement of layers into artboards https://github.com/GraphiteEditor/Graphite/issues/1507
		let new_layer_node = resolve_document_node_type("Merge").expect("Merge node").default_document_node();
		let (post_node_id, pre_node_id, post_node_input_index) = Self::get_post_node_with_index(self.document_network, parent, skip_layer_nodes);

		if let Some(post_node_id) = post_node_id {
			if let Some(pre_node_id) = pre_node_id {
				self.insert_between(
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
				self.insert_node_before(new_id, post_node_id, post_node_input_index, new_layer_node, offset);
			};
		} else {
			//If post_node does not exist, then network is empty
			self.document_network.nodes.insert(new_id, new_layer_node);
			if let Some(export) = self.document_network.exports.get_mut(0) {
				*export = NodeInput::node(new_id, 0);
			}
			self.document_network.root_node = Some(RootNode { id: new_id, output_index: 0 });
		}

		Some(new_id)
	}

	pub fn create_layer_with_insert_index(&mut self, new_id: NodeId, insert_index: isize, parent: LayerNodeIdentifier) -> Option<NodeId> {
		let skip_layer_nodes = if insert_index < 0 { (-1 - insert_index) as usize } else { insert_index as usize };
		self.create_layer(new_id, parent, skip_layer_nodes)
	}

	/// Creates an artboard that outputs to the output node.
	pub fn create_artboard(&mut self, new_id: NodeId, artboard: Artboard) -> Option<NodeId> {
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
		//If the root node either doesn't exist or is a non artboard node, connect new artboard directly to export
		if self
			.document_network
			.root_node
			.map_or(true, |root_node| self.document_network.nodes.get(&root_node.id).map_or(false, |node| !node.is_artboard()))
		{
			self.insert_node_as_primary_export(new_id, artboard_node, 0)
		} else {
			let root_node = self.document_network.root_node.unwrap();
			// Get node that feeds into the root node. If it exists, connect the new artboard node in between.
			let output_node_primary_input = self.document_network.nodes.get(&root_node.id)?.inputs.get(0);
			let created_node_id = if let NodeInput::Node { node_id, .. } = &output_node_primary_input? {
				let pre_node = self.document_network.nodes.get(node_id)?;
				// If the node currently connected the Output is an artboard, connect to input 0 (Artboards input) of the new artboard. Else connect to the Over input.
				let artboard_input_index = if pre_node.is_artboard() { 0 } else { 1 };

				self.insert_between(
					new_id,
					artboard_node,
					NodeInput::node(*node_id, 0),
					artboard_input_index,
					root_node.id,
					NodeInput::node(new_id, 0),
					0,
					IVec2::new(0, 3),
				)
			} else {
				//Else connect the new artboard directly to the root node.
				self.insert_node_before(new_id, root_node.id, 0, artboard_node, IVec2::new(-8, 3))
			};

			created_node_id
		}
	}
	pub fn insert_vector_data(&mut self, subpaths: Vec<Subpath<ManipulatorGroupId>>, layer: NodeId) {
		let shape = {
			let node_type = resolve_document_node_type("Shape").expect("Shape node does not exist");
			node_type.to_document_node_default_inputs([Some(NodeInput::value(TaggedValue::Subpaths(subpaths), false))], Default::default())
		};
		let transform = resolve_document_node_type("Transform").expect("Transform node does not exist").default_document_node();
		let fill = resolve_document_node_type("Fill").expect("Fill node does not exist").default_document_node();
		let stroke = resolve_document_node_type("Stroke").expect("Stroke node does not exist").default_document_node();

		let stroke_id = NodeId(generate_uuid());
		self.insert_node_before(stroke_id, layer, 1, stroke, IVec2::new(-8, 0));
		let fill_id = NodeId(generate_uuid());
		self.insert_node_before(fill_id, stroke_id, 0, fill, IVec2::new(-8, 0));
		let transform_id = NodeId(generate_uuid());
		self.insert_node_before(transform_id, fill_id, 0, transform, IVec2::new(-8, 0));
		let shape_id = NodeId(generate_uuid());
		self.insert_node_before(shape_id, transform_id, 0, shape, IVec2::new(-8, 0));
		self.responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	pub fn insert_text(&mut self, text: String, font: Font, size: f64, layer: NodeId) {
		let text = resolve_document_node_type("Text").expect("Text node does not exist").to_document_node(
			[
				NodeInput::network(graph_craft::concrete!(graphene_std::wasm_application_io::WasmEditorApi), 0),
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
		self.insert_node_before(stroke_id, layer, 1, stroke, IVec2::new(-8, 0));
		let fill_id = NodeId(generate_uuid());
		self.insert_node_before(fill_id, stroke_id, 0, fill, IVec2::new(-8, 0));
		let transform_id = NodeId(generate_uuid());
		self.insert_node_before(transform_id, fill_id, 0, transform, IVec2::new(-8, 0));
		let text_id = NodeId(generate_uuid());
		self.insert_node_before(text_id, transform_id, 0, text, IVec2::new(-8, 0));
		self.responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	pub fn insert_image_data(&mut self, image_frame: ImageFrame<Color>, layer: NodeId) {
		let image = {
			let node_type = resolve_document_node_type("Image").expect("Image node does not exist");
			node_type.to_document_node_default_inputs([Some(NodeInput::value(TaggedValue::ImageFrame(image_frame), false))], Default::default())
		};
		let transform = resolve_document_node_type("Transform").expect("Transform node does not exist").default_document_node();

		let transform_id = NodeId(generate_uuid());
		self.insert_node_before(transform_id, layer, 1, transform, IVec2::new(-8, 0));

		let image_id = NodeId(generate_uuid());
		self.insert_node_before(image_id, transform_id, 0, image, IVec2::new(-8, 0));

		self.responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	pub fn shift_upstream(network: &mut NodeNetwork, node_id: NodeId, shift: IVec2, shift_self: bool) {
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
			if let Some(node) = network.nodes.get_mut(&node_id) {
				node.metadata.position += shift;
			}
		}
	}

	/// Inserts a new node and modifies the inputs
	pub fn modify_new_node(&mut self, name: &'static str, update_input: impl FnOnce(&mut Vec<NodeInput>, NodeId, &DocumentMetadata)) {
		let output_node_id = self.layer_node.or_else(|| {
			if let Some(NodeInput::Node { node_id, .. }) = self.document_network.exports.get(0) {
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
		self.document_network.nodes.insert(node_id, new_document_node);

		let upstream_nodes = self
			.document_network
			.upstream_flow_back_from_nodes(vec![node_id], graph_craft::document::FlowType::HorizontalFlow)
			.map(|(_, id)| id)
			.collect::<Vec<_>>();
		for node_id in upstream_nodes {
			let Some(node) = self.document_network.nodes.get_mut(&node_id) else { continue };
			node.metadata.position.x -= 8;
		}
	}

	/// Changes the inputs of a specific node
	pub fn modify_inputs(&mut self, name: &'static str, skip_rerender: bool, update_input: impl FnOnce(&mut Vec<NodeInput>, NodeId, &DocumentMetadata)) {
		let existing_node_id = self
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
			.find(|(node, _)| node.name == name)
			.map(|(_, id)| id);
		if let Some(node_id) = existing_node_id {
			self.modify_existing_node_inputs(node_id, update_input);
		} else {
			self.modify_new_node(name, update_input);
		}

		//self.node_graph.network.clear();
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
							.filter_map(|output| if let NodeInput::Node { node_id, .. } = output { Some(node_id.clone()) } else { None })
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

	/// Returns a true if the network structure is updated
	pub fn set_input(network: &mut NodeNetwork, node_id: NodeId, input_index: usize, input: NodeInput, is_document_network: bool) -> bool {
		let mut load_network_structure = false;
		if let Some(node) = network.nodes.get_mut(&node_id) {
			let Some(node_input) = node.inputs.get_mut(input_index) else {
				error!("Tried to set input {input_index} to {input:?}, but the index was invalid. Node {node_id}:\n{node:#?}");
				return false;
			};
			let structure_changed = node_input.as_node().is_some() || input.as_node().is_some();
			*node_input = input;
			//Only load network structure for changes to document_network
			load_network_structure = structure_changed && is_document_network;
		} else if node_id == network.exports_metadata.0 {
			let Some(export) = network.exports.get_mut(input_index) else {
				error!("Tried to set export {input_index} to {input:?}, but the index was invalid. Network:\n{network:#?}");
				return false;
			};
			*export = input;
			//Only load network structure for changes to document_network
			load_network_structure = is_document_network;
		}
		load_network_structure
	}
	pub fn fill_set(&mut self, fill: Fill) {
		self.modify_inputs("Fill", false, |inputs, _node_id, _metadata| {
			let fill_type = match fill {
				Fill::None | Fill::Solid(_) => FillType::Solid,
				Fill::Gradient(_) => FillType::Gradient,
			};
			inputs[1] = NodeInput::value(TaggedValue::FillType(fill_type), false);
			if Fill::None == fill {
				inputs[2] = NodeInput::value(TaggedValue::OptionalColor(None), false);
			} else if let Fill::Solid(color) = fill {
				inputs[2] = NodeInput::value(TaggedValue::OptionalColor(Some(color)), false);
			} else if let Fill::Gradient(gradient) = fill {
				inputs[3] = NodeInput::value(TaggedValue::GradientType(gradient.gradient_type), false);
				inputs[4] = NodeInput::value(TaggedValue::DVec2(gradient.start), false);
				inputs[5] = NodeInput::value(TaggedValue::DVec2(gradient.end), false);
				inputs[6] = NodeInput::value(TaggedValue::DAffine2(gradient.transform), false);
				inputs[7] = NodeInput::value(TaggedValue::GradientPositions(gradient.positions), false);
			}
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

	pub fn transform_change(&mut self, transform: DAffine2, transform_in: TransformIn, parent_transform: DAffine2, bounds: LayerBounds, skip_rerender: bool) {
		self.modify_inputs("Transform", skip_rerender, |inputs, node_id, metadata| {
			let layer_transform = transform_utils::get_current_transform(inputs);
			let upstream_transform = metadata.upstream_transform(node_id);
			let to = match transform_in {
				TransformIn::Local => DAffine2::IDENTITY,
				TransformIn::Scope { scope } => scope * parent_transform,
				TransformIn::Viewport => parent_transform,
			};
			let pivot = DAffine2::from_translation(upstream_transform.transform_point2(bounds.layerspace_pivot(transform_utils::get_current_normalized_pivot(inputs))));
			let transform = pivot.inverse() * to.inverse() * transform * to * pivot * layer_transform;
			transform_utils::update_transform(inputs, transform);
		});
	}

	pub fn transform_set(&mut self, mut transform: DAffine2, transform_in: TransformIn, parent_transform: DAffine2, current_transform: Option<DAffine2>, bounds: LayerBounds, skip_rerender: bool) {
		self.modify_inputs("Transform", skip_rerender, |inputs, node_id, metadata| {
			let upstream_transform = metadata.upstream_transform(node_id);

			let to = match transform_in {
				TransformIn::Local => DAffine2::IDENTITY,
				TransformIn::Scope { scope } => scope * parent_transform,
				TransformIn::Viewport => parent_transform,
			};
			let pivot = DAffine2::from_translation(upstream_transform.transform_point2(bounds.layerspace_pivot(transform_utils::get_current_normalized_pivot(inputs))));

			if current_transform
				.filter(|transform| transform.matrix2.determinant() != 0. && upstream_transform.matrix2.determinant() != 0.)
				.is_some()
			{
				transform *= upstream_transform.inverse();
			}
			let final_transform = pivot.inverse() * to.inverse() * transform * pivot;
			transform_utils::update_transform(inputs, final_transform);
		});
	}

	pub fn pivot_set(&mut self, new_pivot: DVec2, bounds: LayerBounds) {
		self.modify_inputs("Transform", false, |inputs, node_id, metadata| {
			let layer_transform = transform_utils::get_current_transform(inputs);
			let upstream_transform = metadata.upstream_transform(node_id);
			let old_pivot_transform = DAffine2::from_translation(upstream_transform.transform_point2(bounds.local_pivot(transform_utils::get_current_normalized_pivot(inputs))));
			let new_pivot_transform = DAffine2::from_translation(upstream_transform.transform_point2(bounds.local_pivot(new_pivot)));
			let transform = new_pivot_transform.inverse() * old_pivot_transform * layer_transform * old_pivot_transform.inverse() * new_pivot_transform;
			transform_utils::update_transform(inputs, transform);
			inputs[5] = NodeInput::value(TaggedValue::DVec2(new_pivot), false);
		});
	}

	pub fn update_bounds(&mut self, [old_bounds_min, old_bounds_max]: [DVec2; 2], [new_bounds_min, new_bounds_max]: [DVec2; 2]) {
		self.modify_all_node_inputs("Transform", false, |inputs, node_id, metadata| {
			let upstream_transform = metadata.upstream_transform(node_id);
			let layer_transform = transform_utils::get_current_transform(inputs);
			let normalized_pivot = transform_utils::get_current_normalized_pivot(inputs);

			let old_layerspace_pivot = (old_bounds_max - old_bounds_min) * normalized_pivot + old_bounds_min;
			let new_layerspace_pivot = (new_bounds_max - new_bounds_min) * normalized_pivot + new_bounds_min;
			let new_pivot_transform = DAffine2::from_translation(upstream_transform.transform_point2(new_layerspace_pivot));
			let old_pivot_transform = DAffine2::from_translation(upstream_transform.transform_point2(old_layerspace_pivot));

			let transform = new_pivot_transform.inverse() * old_pivot_transform * layer_transform * old_pivot_transform.inverse() * new_pivot_transform;
			transform_utils::update_transform(inputs, transform);
		});
	}

	pub fn vector_modify(&mut self, modification: VectorDataModification) -> Option<LayerNodeIdentifier> {
		let [mut old_bounds_min, mut old_bounds_max] = [DVec2::ZERO, DVec2::ONE];
		let [mut new_bounds_min, mut new_bounds_max] = [DVec2::ZERO, DVec2::ONE];
		let mut empty = false;

		self.modify_inputs("Shape", false, |inputs, _node_id, _metadata| {
			let [subpaths, colinear_manipulators] = inputs.as_mut_slice() else {
				panic!("Shape does not have both `subpath` and `colinear_manipulators` inputs");
			};

			let NodeInput::Value {
				tagged_value: TaggedValue::Subpaths(subpaths),
				..
			} = subpaths
			else {
				return;
			};
			let NodeInput::Value {
				tagged_value: TaggedValue::ManipulatorGroupIds(colinear_manipulators),
				..
			} = colinear_manipulators
			else {
				return;
			};

			[old_bounds_min, old_bounds_max] = transform_utils::nonzero_subpath_bounds(subpaths);

			transform_utils::VectorModificationState { subpaths, colinear_manipulators }.modify(modification);
			empty = !subpaths.iter().any(|subpath| !subpath.is_empty());

			[new_bounds_min, new_bounds_max] = transform_utils::nonzero_subpath_bounds(subpaths);
		});

		self.update_bounds([old_bounds_min, old_bounds_max], [new_bounds_min, new_bounds_max]);

		if empty {
			self.layer_node.map(|layer_id| LayerNodeIdentifier::new(layer_id, &self.document_network))
		} else {
			None
		}
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

	/// Deletes all nodes in node_ids and any sole dependents in the horizontal chain if the node to delete is a layer node
	/// network is the network that the node to be deleted is part of
	pub fn delete_nodes(
		document_network: &mut NodeNetwork,
		selected_nodes: &mut SelectedNodes,
		node_ids: Vec<NodeId>,
		reconnect: bool,
		responses: &mut VecDeque<Message>,
		network_path: Vec<NodeId>,
		resolved_types: &ResolvedDocumentNodeTypes,
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

			let Some(node) = network.nodes.get(&node_id) else {
				continue;
			};
			let child_id = node.inputs.get(1).and_then(|input| if let NodeInput::Node { node_id, .. } = input { Some(node_id) } else { None });
			let Some(child_id) = child_id else {
				continue;
			};

			let outward_links = network.collect_outwards_links();

			for (_, upstream_id) in network.upstream_flow_back_from_nodes(vec![*child_id], graph_craft::document::FlowType::UpstreamFlow) {
				// This does a downstream traversal starting from the current node, and ending at either a node in the delete_nodes set or the output.
				// If the traversal find as child node of a node in the delete_nodes set, then it is a sole dependent. If the output node is eventually reached, then it is not a sole dependent.
				let mut stack = vec![upstream_id];
				let mut can_delete = true;

				while let Some(current_node) = stack.pop() {
					if let Some(downstream_nodes) = outward_links.get(&current_node) {
						for downstream_node in downstream_nodes {
							if network.root_node.expect("Root node should always exist if a node is being deleted").id == *downstream_node {
								can_delete = false;
							} else if !delete_nodes.contains(downstream_node) {
								stack.push(*downstream_node);
							}
							// Continue traversing over the downstream sibling, which happens if the current node is a sibling to a node in node_ids
							else {
								for deleted_node_id in &node_ids {
									let Some(output_node) = network.nodes.get(&deleted_node_id) else {
										continue;
									};
									let Some(input) = output_node.inputs.get(0) else {
										continue;
									};

									if let NodeInput::Node { node_id, .. } = input {
										if *node_id == current_node {
											stack.push(*deleted_node_id);
										};
									};
								}
							};
						}
					}
				}

				if can_delete {
					delete_nodes.insert(upstream_id);
				}
			}
		}
		for delete_node_id in delete_nodes {
			ModifyInputsContext::remove_node(document_network, selected_nodes, delete_node_id, reconnect, responses, &network_path, resolved_types);
		}
	}

	/// Tries to remove a node from the network, returning true on success.
	fn remove_node(
		document_network: &mut NodeNetwork,
		selected_nodes: &mut SelectedNodes,
		node_id: NodeId,
		reconnect: bool,
		responses: &mut VecDeque<Message>,
		network_path: &Vec<NodeId>,
		resolved_types: &ResolvedDocumentNodeTypes,
	) -> bool {
		if !ModifyInputsContext::remove_references_from_network(document_network, node_id, reconnect, selected_nodes, network_path, resolved_types) {
			log::error!("could not remove_references_from_network");
			return false;
		}
		let selected_nodes_iter = selected_nodes.selected_nodes_ref().iter();
		let Some(network) = document_network.nested_network_for_selected_nodes_mut(&network_path, selected_nodes_iter) else {
			return false;
		};

		network.nodes.remove(&node_id);
		if network.root_node.is_some_and(|root_node| root_node.id == node_id) {
			network.root_node = None;
		}
		selected_nodes.retain_selected_nodes(|&id| id != node_id);
		responses.add(BroadcastEvent::SelectionChanged);
		true
	}

	fn remove_references_from_network(
		document_network: &mut NodeNetwork,
		deleting_node_id: NodeId,
		reconnect: bool,
		selected_nodes: &mut SelectedNodes,
		network_path: &Vec<NodeId>,
		resolved_types: &ResolvedDocumentNodeTypes,
	) -> bool {
		let Some(network) = document_network.nested_network_for_selected_nodes(&network_path, selected_nodes.selected_nodes_ref().iter()) else {
			return false;
		};
		let mut reconnect_to_input: Option<NodeInput> = None;

		if reconnect {
			// Check whether the being-deleted node's first (primary) input is a node
			if let Some(node) = network.nodes.get(&deleting_node_id) {
				// Reconnect to the node below when deleting a layer node.
				if matches!(&node.inputs.get(0), Some(NodeInput::Node { .. })) {
					reconnect_to_input = Some(node.inputs[0].clone());
				}
			}
		}

		let mut nodes_to_set_input = Vec::new();
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
			let NodeInput::Node {
				node_id: upstream_node_id,
				output_index,
				..
			} = input
			else {
				continue;
			};
			if *upstream_node_id != deleting_node_id {
				continue;
			}

			// Only reconnect if the output index for the node to be deleted is 0
			if *output_index != 0 {
				reconnect_to_input = None
			};
			if reconnect_to_input.is_some() {
				// None means to use reconnect_to_input, which can be safely unwrapped
				nodes_to_set_input.push((node_id, input_index, None));
			} else {
				//Disconnect input
				let tagged_value = ModifyInputsContext::get_input_tagged_value(document_network, network_path, node_id, resolved_types, input_index);
				let value_input = NodeInput::value(tagged_value, true);
				nodes_to_set_input.push((node_id, input_index, Some(value_input)));
			}
		}

		let Some(network) = document_network.nested_network_for_selected_nodes_mut(&network_path, selected_nodes.selected_nodes_ref().iter()) else {
			return false;
		};
		let is_document_network = network_path.is_empty();
		for (node_id, input_index, value_input) in nodes_to_set_input {
			if let Some(value_input) = value_input {
				ModifyInputsContext::set_input(network, node_id, input_index, value_input, is_document_network);
				if network.root_node.is_some_and(|root_node| root_node.id == deleting_node_id) {
					//Update the root node to be None
					network.root_node = None;
				}
			}
			// Reconnect to node upstream of the deleted node
			else {
				ModifyInputsContext::set_input(network, node_id, input_index, reconnect_to_input.clone().unwrap(), is_document_network);

				if network.root_node.is_some_and(|root_node| root_node.id == deleting_node_id) {
					// Update the root node to be the reconnected node
					if let NodeInput::Node { node_id, output_index, .. } = reconnect_to_input.as_ref().unwrap() {
						network.root_node = Some(RootNode {
							id: *node_id,
							output_index: *output_index,
						});
					}
				}
			}
		}
		true
	}

	/// Get the tagged_value for any node id and input index. Network path is the path to the encapsulating node (including the encapsulating node), node_id is the selected node
	pub fn get_input_tagged_value(document_network: &NodeNetwork, network_path: &Vec<NodeId>, node_id: NodeId, resolved_types: &ResolvedDocumentNodeTypes, input_index: usize) -> TaggedValue {
		let Some(network) = document_network.nested_network(&network_path) else {
			log::error!("Could not get network in get_tagged_value");
			return TaggedValue::None;
		};
		//TODO: Store types for all document nodes, not just the compiled proto nodes, which currently skips isolated nodes
		let node_id_path = &[&network_path[..], &[node_id]].concat();
		let input_type = resolved_types.inputs.get(&graph_craft::document::Source {
			node: node_id_path.clone(),
			index: input_index,
		});
		if let Some(input_type) = input_type {
			TaggedValue::try_from_type(input_type)
		} else if node_id == network.exports_metadata.0 {
			if let Some(parent_node_id) = network_path.last() {
				let mut parent_path = network_path.clone();
				parent_path.pop();
				let parent_node = document_network
					.nested_network(&parent_path)
					.expect("Parent path should always exist")
					.nodes
					.get(&parent_node_id)
					.expect("Last path node should always exist in parent network");

				let output_types = NodeGraphMessageHandler::get_output_types(parent_node, &resolved_types, network_path);
				output_types.iter().nth(input_index).map_or_else(
					|| {
						warn!("Could not find output type for export node {node_id}");
						TaggedValue::None
					},
					|output_type| output_type.clone().map_or(TaggedValue::None, |input| TaggedValue::try_from_type(&input)),
				)
			} else {
				TaggedValue::ArtboardGroup(ArtboardGroup::EMPTY)
			}
		} else {
			TaggedValue::None
		}
	}
}

use super::transform_utils::{self, LayerBounds};
use crate::messages::portfolio::document::node_graph::document_node_types::{resolve_document_node_type, DocumentNodeDefinition};
use crate::messages::portfolio::document::utility_types::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use crate::messages::portfolio::document::utility_types::network_metadata::NodeNetworkInterface;
use crate::messages::portfolio::document::utility_types::nodes::SelectedNodes;
use crate::messages::prelude::*;

use bezier_rs::Subpath;
use graph_craft::concrete;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{generate_uuid, DocumentNode, DocumentNodeImplementation, NodeId, NodeInput, NodeNetwork, Previewing};
use graphene_core::raster::{BlendMode, ImageFrame};
use graphene_core::text::Font;
use graphene_core::uuid::ManipulatorGroupId;
use graphene_core::vector::brush_stroke::BrushStroke;
use graphene_core::vector::style::{Fill, Stroke};
use graphene_core::Type;
use graphene_core::{Artboard, Color};
use graphene_std::vector::ManipulatorPointId;
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
// This struct is helpful to prevent passing the same arguments to multiple functions
// Should only be used by GraphOperationMessage, since it only affects the document network.
pub struct ModifyInputsContext<'a> {
	pub network_interface: &'a mut NodeNetworkInterface,
	pub document_metadata: &'a mut DocumentMetadata,
	pub responses: &'a mut VecDeque<Message>,
	pub layer_node: Option<LayerNodeIdentifier>,
}

impl<'a> ModifyInputsContext<'a> {
	/// Get the node network from the document
	pub fn new(network_interface: &'a mut NodeNetwork, document_metadata: &'a mut DocumentMetadata, responses: &'a mut VecDeque<Message>) -> Self {
		Self {
			network_interface,
			document_metadata,
			responses,
			layer_node: None,
		}
	}

	pub fn new_with_layer(id: NodeId, network_interface: &'a mut NodeNetworkInterface, document_metadata: &'a mut DocumentMetadata, responses: &'a mut VecDeque<Message>) -> Option<Self> {
		let mut document = Self::new(network_interface, document_metadata, responses);
		document.layer_node = network_interface.downstream_layer(&id);
		Some(document)
	}

	// TODO: Replace return values with InputConnector/OutputConnector
	/// Starts at any folder, or the output, and skips layer nodes based on insert_index. Non layer nodes are always skipped. Returns the post node id, pre node id, and the input index.
	///       -----> Post node input_index: 0
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

	pub fn create_layer(&mut self, new_id: NodeId, parent: LayerNodeIdentifier, insert_index: isize) -> LayerNodeIdentifier {
		let skip_layer_nodes = if insert_index < 0 { (-1 - insert_index) as usize } else { insert_index as usize };

		assert!(!self.network_interface.document_network().nodes.contains_key(&new_id), "Creating already existing layer");
		// TODO: Smarter placement of layers into artboards https://github.com/GraphiteEditor/Graphite/issues/1507

		let mut parent = parent;
		if parent == LayerNodeIdentifier::ROOT_PARENT {
			if let Some(root_node) = self.network_interface.document_network().get_root_node() {
				// If the current root node is the artboard, then the new layer should be a child of the artboard
				let current_root_node = self.network_interface.document_network().nodes.get(&root_node.id).expect("Root node should always exist");
				if current_root_node.is_artboard() && current_root_node.is_layer {
					parent = LayerNodeIdentifier::new(root_node.id, &self.network_interface);
				}
			}
		}

		let new_layer_node = resolve_document_node_type("Merge").expect("Merge node");

		self.insert_layer_to_stack(new_id, new_layer_node_definition, parent, 0);
		LayerNodeIdentifier::new(new_id, &self.network_interface.document_network())
	}

	/// Creates an artboard as the primary export for the document network
	pub fn create_artboard(&self, new_id: NodeId, artboard: Artboard) -> Option<NodeId> {
		let mut artboard_node_definition = resolve_document_node_type("Artboard").expect("Node");
		artboard_node_definition.override_definition_inputs(
			[
				Some(NodeInput::value(TaggedValue::ArtboardGroup(graphene_std::ArtboardGroup::EMPTY), true)),
				Some(NodeInput::value(TaggedValue::GraphicGroup(graphene_core::GraphicGroup::EMPTY), true)),
				Some(NodeInput::value(TaggedValue::IVec2(artboard.location), false)),
				Some(NodeInput::value(TaggedValue::IVec2(artboard.dimensions), false)),
				Some(NodeInput::value(TaggedValue::Color(artboard.background), false)),
				Some(NodeInput::value(TaggedValue::Bool(artboard.clip), false)),
			],
		);

		self.insert_layer_to_stack(new_id, artboard_node_definition, LayerNodeIdentifier::ROOT_PARENT, 0);
	}
	pub fn insert_vector_data(&mut self, subpaths: Vec<Subpath<ManipulatorGroupId>>, layer: LayerNodeIdentifier) {
		let shape = {
			let mut node_type: &crate::messages::portfolio::document::node_graph::document_node_types::DocumentNodeDefinition = resolve_document_node_type("Shape").expect("Shape node does not exist");
			node_type.override_definition_inputs([Some(NodeInput::value(TaggedValue::Subpaths(subpaths), false))])
			node_type
		};
		let transform = resolve_document_node_type("Transform").expect("Transform node does not exist");
		let fill = resolve_document_node_type("Fill").expect("Fill node does not exist");
		let stroke = resolve_document_node_type("Stroke").expect("Stroke node does not exist");

		let stroke_id = NodeId(generate_uuid());
		self.insert_node_to_chain(stroke_id, layer, stroke);
		let fill_id = NodeId(generate_uuid());
		self.insert_node_to_chain(stroke_id, layer, fill);
		let transform_id = NodeId(generate_uuid());
		self.insert_node_to_chain(stroke_id, layer, transform);
		let shape_id = NodeId(generate_uuid());
		self.insert_node_to_chain(stroke_id, layer, shape);
		self.responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	pub fn insert_text(&mut self, text: String, font: Font, size: f64, layer: LayerNodeIdentifier) {
		let mut text = resolve_document_node_type("Text").expect("Text node does not exist");
		text.override_definition_inputs(
			[
				NodeInput::network(graph_craft::concrete!(graphene_std::wasm_application_io::WasmEditorApi), 0),
				NodeInput::value(TaggedValue::String(text), false),
				NodeInput::value(TaggedValue::Font(font), false),
				NodeInput::value(TaggedValue::F64(size), false),
			],
		);

		let transform = resolve_document_node_type("Transform").expect("Transform node does not exist");
		let fill = resolve_document_node_type("Fill").expect("Fill node does not exist");
		let stroke = resolve_document_node_type("Stroke").expect("Stroke node does not exist");

		let stroke_id = NodeId(generate_uuid());
		self.insert_node_to_chain(stroke_id, layer, stroke);
		let fill_id = NodeId(generate_uuid());
		self.insert_node_to_chain(stroke_id, layer, fill);
		let transform_id = NodeId(generate_uuid());
		self.insert_node_to_chain(stroke_id, layer, transform);
		let text_id = NodeId(generate_uuid());
		self.insert_node_to_chain(text_id, layer, text);
		self.responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	pub fn insert_image_data(&self, image_frame: ImageFrame<Color>, layer: LayerNodeIdentifier, responses: &mut VecDeque<Message>) {
		let image = {
			let mut node_type = resolve_document_node_type("Image").expect("Image node does not exist");
			node_type.override_definition_inputs([Some(NodeInput::value(TaggedValue::ImageFrame(image_frame), false))])
			node_type
		};
		let transform = resolve_document_node_type("Transform").expect("Transform node does not exist");

		let transform_id = NodeId(generate_uuid());
		self.insert_node_to_chain(transform_id, layer, transform);
		let image_id = NodeId(generate_uuid());
		self.insert_node_to_chain(image_id, layer, transform);
		responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	pub fn get_existing_node_id(&self, name: &'static str) -> NodeId {
		let existing_node_id = self
			.network_interface
			.document_network()
			.upstream_flow_back_from_nodes(
				self.layer_node.map_or_else(
					|| {
						self.network_interface
							.document_network()
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
			.map(|(_, id)| id)
			.unwrap_or_else(|| {
				//Insert node into the network
				let output_layer = self.layer_node.unwrap_or_else(|| {
					log::debug!("Creating node without self.layer_node. Ensure this behavior is correct.");
					LayerNodeIdentifier::ROOT_PARENT.first_child(&self.document_metadata).unwrap_or(LayerNodeIdentifier::ROOT_PARENT)
				});
				let new_node_id = NodeId(generate_uuid());
				self.insert_node_to_chain(
					new_node_id,
					output_layer,
					resolve_document_node_type(name).expect("Node type \"{name}\" doesn't exist when inserting node by name"),
				);
				new_node_id
			});
	}

	/// Changes the inputs of a specific node
	/// TODO: Remove once Vector Modify PR is merged, which modifies the Transform logic, and replace with what is done in fill_set
	pub fn modify_inputs(&mut self, name: &'static str, skip_rerender: bool, update_input: impl FnOnce(&mut Vec<NodeInput>, NodeId, &DocumentMetadata)) {}

	/// Updates the input of an existing node
	// TODO: Remove and use network_interface API to update the inputs
	pub fn modify_existing_node_inputs(&mut self, node_id: NodeId, update_input: impl FnOnce(&mut Vec<NodeInput>, NodeId, &DocumentMetadata)) {
		let document_node: &mut DocumentNode = self.network_interface.document_network().nodes.get_mut(&node_id).unwrap();
		update_input(&mut document_node.inputs, node_id, self.document_metadata);
	}

	/// Changes the inputs of a all of the existing instances of a node name
	pub fn modify_all_node_inputs(&mut self, name: &'static str, skip_rerender: bool, mut update_input: impl FnMut(&mut Vec<NodeInput>, NodeId, &DocumentMetadata)) {
		let existing_nodes: Vec<_> = self
			.network_interface
			.upstream_flow_back_from_nodes(
				self.layer_node.map_or_else(
					|| {
						self.network_interface
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

	pub fn fill_set(&mut self, fill: Fill) {
		let fill_index = 1;
		let backup_color_index = 2;
		let backup_gradient_index = 3;

		let fill_node_id = self.get_existing_node_id("Fill");
		match &fill {
			Fill::None => {
				let input_connector = InputConnector::node(fill_node_id, backup_color_index);
				self.set_input(input_connector, NodeInput::value(TaggedValue::OptionalColor(None), false), false);
			}
			Fill::Solid(color) => {
				let input_connector = InputConnector::node(fill_node_id, backup_color_index);
				self.set_input(input_connector, NodeInput::value(TaggedValue::OptionalColor(Some(*color)), false), false);
			}
			Fill::Gradient(gradient) => {
				let input_connector = InputConnector::node(fill_node_id, backup_gradient_index);
				self.set_input(input_connector, NodeInput::value(TaggedValue::Gradient(gradient.clone()), false), false);
			}
		}
		let input_connector = InputConnector::node(fill_node_id, fill_index);
		self.set_input(input_connector, NodeInput::value(TaggedValue::Fill(fill), false), false);
	}

	pub fn opacity_set(&mut self, opacity: f64) {
		let opacity_node_id = self.get_existing_node_id("Opacity");
		let input_connector = InputConnector::node(opacity_node_id, 1);
		self.set_input(input_connector, NodeInput::value(TaggedValue::F64(opacity * 100.), false), false);
	}

	pub fn blend_mode_set(&mut self, blend_mode: BlendMode) {
		let blend_mode_node_id = self.get_existing_node_id("Blend Mode");
		let input_connector = InputConnector::node(blend_mode_node_id, 1);
		self.set_input(input_connector, NodeInput::value(TaggedValue::BlendMode(blend_mode), false), false);
	}

	pub fn stroke_set(&mut self, stroke: Stroke) {
		let stroke_node_id = self.get_existing_node_id("Stroke");

		let input_connector = InputConnector::node(stroke_node_id, 1);
		self.set_input(input_connector, NodeInput::value(TaggedValue::OptionalColor(stroke.color), false), false);
		let input_connector = InputConnector::node(stroke_node_id, 2);
		self.set_input(input_connector, NodeInput::value(TaggedValue::F64(stroke.weight), false), false);
		let input_connector = InputConnector::node(stroke_node_id, 3);
		self.set_input(input_connector, NodeInput::value(TaggedValue::VecF64(stroke.dash_lengths), false), false);
		let input_connector = InputConnector::node(stroke_node_id, 4);
		self.set_input(input_connector, NodeInput::value(TaggedValue::F64(stroke.dash_offset), false), false);
		let input_connector = InputConnector::node(stroke_node_id, 5);
		self.set_input(input_connector, NodeInput::value(TaggedValue::LineCap(stroke.line_cap), false), false);
		let input_connector = InputConnector::node(stroke_node_id, 6);
		self.set_input(input_connector, NodeInput::value(TaggedValue::LineJoin(stroke.line_join), false), false);
		let input_connector = InputConnector::node(stroke_node_id, 7);
		self.set_input(input_connector, NodeInput::value(TaggedValue::F64(stroke.line_join_miter_limit), false), false);
	}

	pub fn brush_modify(&mut self, strokes: Vec<BrushStroke>) {
		let brush_node_id = self.get_existing_node_id("Brush");

		let input_connector = InputConnector::node(brush_node_id, 2);
		self.set_input(input_connector, NodeInput::value(TaggedValue::BrushStrokes(strokes), false), false);
	}

	pub fn resize_artboard(&mut self, location: IVec2, dimensions: IVec2) {
		let artboard_node_id = self.get_existing_node_id("Artboard");

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

		let input_connector = InputConnector::node(artboard_node_id, 2);
		self.set_input(input_connector, NodeInput::value(TaggedValue::IVec2(location), false), false);
		let input_connector = InputConnector::node(artboard_node_id, 3);
		self.set_input(input_connector, NodeInput::value(TaggedValue::IVec2(dimensions), false), false);
	}

	//TODO: Transfer all transform input setting to use interface after the Vector Modify PR is merged
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
			self.layer_node.map(|layer_id| LayerNodeIdentifier::new(layer_id, &self.network_interface))
		} else {
			None
		}
	}

	/// Always modifies the document network. Returns true if the network structure is updated
	pub fn set_input(&self, input_connector: InputConnector, input: NodeInput, skip_rerender: bool) -> bool {
		self.network_interface.set_input(input_connector, input, skip_rerender, true);
		self.responses.add(PropertiesPanelMessage::Refresh);
		if !skip_rerender {
			self.responses.add(NodeGraphMessage::RunDocumentGraph);
		}

		// let Some(network) = document_network.nested_network_mut(network_path) else {
		// 	log::error!("Could not get nested network for set_input");
		// 	return false;
		// };
		// if let Some(node) = network.nodes.get_mut(&node_id) {
		// 	let Some(node_input) = node.inputs.get_mut(input_index) else {
		// 		log::error!("Tried to set input {input_index} to {input:?}, but the index was invalid. Node {node_id}:\n{node:#?}");
		// 		return false;
		// 	};
		// 	let structure_changed = node_input.as_node().is_some() || input.as_node().is_some();

		// 	let previously_exposed = node_input.is_exposed();
		// 	*node_input = input;
		// 	let currently_exposed = node_input.is_exposed();
		// 	if previously_exposed != currently_exposed {
		// 		node_graph.update_click_target(node_id, document_network, network_path.clone());
		// 	}

		// 	// Only load network structure for changes to document_network
		// 	structure_changed && is_document_network
		// } else if node_id == network.exports_metadata.0 {
		// 	let Some(export) = network.exports.get_mut(input_index) else {
		// 		log::error!("Tried to set export {input_index} to {input:?}, but the index was invalid. Network:\n{network:#?}");
		// 		return false;
		// 	};

		// 	let previously_exposed = export.is_exposed();
		// 	*export = input;
		// 	let currently_exposed = export.is_exposed();

		// 	if let NodeInput::Node { node_id, output_index, .. } = *export {
		// 		network.update_root_node(node_id, output_index);
		// 	} else if let NodeInput::Value { .. } = *export {
		// 		if input_index == 0 {
		// 			network.stop_preview();
		// 		}
		// 	} else {
		// 		log::error!("Network export input not supported");
		// 	}

		// 	if previously_exposed != currently_exposed {
		// 		node_graph.update_click_target(node_id, document_network, network_path.clone());
		// 	}

		// 	// Only load network structure for changes to document_network
		// 	is_document_network
		// } else {
		// 	false
		// }
	}

	/// Inserts a node at the end of the horizontal node chain from a layer node. The position will be `Position::Chain`
	pub fn insert_node_to_chain(&mut self, new_id: NodeId, parent: LayerNodeIdentifier, mut document_node: DocumentNodeDefinition) {
		assert!(
			self.network_interface.document_network().nodes.contains_key(&node_id),
			"add_node_to_chain only works in the document network"
		);
		// TODO: node layout system and implementation
	}

	/// Inserts a node as a child of a layer at a certain stack index. The position will be `Position::Stack(calculated y position)`
	pub fn insert_layer_to_stack(&mut self, new_id: NodeId, mut document_node: DocumentNodeDefinition, parent: LayerNodeIdentifier, insert_index: usize) {
		assert!(
			self.network_interface.document_network().nodes.contains_key(&node_id),
			"add_node_to_stack only works in the document network"
		);
		// TODO: node layout system and implementation
		// Basic implementation
		// assert!(!self.network_interface.document_network().nodes.contains_key(&id), "Creating already existing node");

		// let previous_root_node = self.network_interface.document_network().get_root_node();

		// // Add the new node as the first child of the exports
		// self.network_interface.insert_layer_to_stack(id, self.network_interface.document_network().exports_metadata.0, 0, new_node);
		// self.network_interface.set_input(self.network_interface.document_network().exports_metadata.0, id, 0);

		// // If a node was previous connected to the exports
		// if let Some(root_node) = previous_root_node {
		// 	let previous_root_node = self.network_interface.document_network().nodes.get(&root_node.id).expect("Root node should always exist");

		// 	// Always move non layer nodes to the chain of the new export layer
		// 	if !previous_root_node.is_layer {
		// 		self.network_interface.move_node_to_chain(root_node.id, id)
		// 	}
		// 	// If the new layer is an artboard and the previous export is not an artboard, move it to be a child
		// 	if new_node.is_artboard() && !previous_root_node.is_artboard() {
		// 		// If that node is a layer, move it to be a child of the artboard.
		// 		if previous_root_node.is_layer {
		// 			self.network_interface.move_node_to_child(root_node.id, id)
		// 		}
		// 	}
		// }
	}
}

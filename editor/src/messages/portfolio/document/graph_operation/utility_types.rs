use super::transform_utils;
use crate::messages::portfolio::document::node_graph::document_node_types::resolve_document_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{self, InputConnector, NodeNetworkInterface, NodeTemplate, OutputConnector};
use crate::messages::prelude::*;

use bezier_rs::Subpath;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{generate_uuid, NodeId, NodeInput};
use graphene_core::raster::{BlendMode, ImageFrame};
use graphene_core::text::Font;
use graphene_core::vector::brush_stroke::BrushStroke;
use graphene_core::vector::style::{Fill, Stroke};
use graphene_core::vector::{PointId, VectorModificationType};
use graphene_core::{Artboard, Color};

use glam::{DAffine2, DVec2, IVec2};

#[derive(PartialEq, Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub enum TransformIn {
	Local,
	Scope { scope: DAffine2 },
	Viewport,
}

// This struct is helpful to prevent passing the same arguments to multiple functions
// Should only be used by GraphOperationMessage, since it only affects the document network.
pub struct ModifyInputsContext<'a> {
	pub network_interface: &'a mut NodeNetworkInterface,
	pub responses: &'a mut VecDeque<Message>,
	// Cannot be LayerNodeIdentifier::ROOT_PARENT
	pub layer_node: Option<LayerNodeIdentifier>,
}

impl<'a> ModifyInputsContext<'a> {
	/// Get the node network from the document
	pub fn new(network_interface: &'a mut NodeNetworkInterface, responses: &'a mut VecDeque<Message>) -> Self {
		Self {
			network_interface,
			responses,
			layer_node: None,
		}
	}

	pub fn new_with_layer(layer: LayerNodeIdentifier, network_interface: &'a mut NodeNetworkInterface, responses: &'a mut VecDeque<Message>) -> Option<Self> {
		if layer == LayerNodeIdentifier::ROOT_PARENT {
			log::error!("LayerNodeIdentifier::ROOT_PARENT should not be used in ModifyInputsContext::new_with_layer");
			return None;
		}
		let mut document = Self::new(network_interface, responses);
		document.layer_node = Some(layer);
		Some(document)
	}

	/// Starts at any folder, or the output, and skips layer nodes based on insert_index. Non layer nodes are always skipped. Returns the post node InputConnector and pre node OutputConnector
	/// Non layer nodes directly upstream of a layer are treated as part of that layer. See insert_index == 2 in the diagram
	///       -----> Post node
	///      |      if insert_index == 0, return (Post node, Some(Layer1))
	/// -> Layer1   
	///      ↑      if insert_index == 1, return (Layer1, Some(Layer2))
	/// -> Layer2   
	///      ↑
	///	-> NonLayerNode
	///      ↑      if insert_index == 2, return (NonLayerNode, Some(Layer3))
	/// -> Layer3  
	///             if insert_index == 3, return (Layer3, None)
	pub fn get_post_node_with_index(network_interface: &NodeNetworkInterface, parent: LayerNodeIdentifier, insert_index: usize) -> (InputConnector, Option<OutputConnector>) {
		let mut post_node_input_connector = if parent == LayerNodeIdentifier::ROOT_PARENT {
			InputConnector::Export(0)
		} else {
			InputConnector::node(parent.to_node(), 1)
		};
		// Skip layers based on skip_layer_nodes, which inserts the new layer at a certain index of the layer stack.
		let mut current_index = 0;

		// Set the post node to the layer node at insert_index
		loop {
			if current_index == insert_index {
				break;
			}
			let next_node_in_stack_id =
				network_interface
					.get_input(&post_node_input_connector, true)
					.and_then(|input_from_connector| if let NodeInput::Node { node_id, .. } = input_from_connector { Some(node_id) } else { None });

			if let Some(next_node_in_stack_id) = next_node_in_stack_id {
				// Only increment index for layer nodes
				if network_interface.is_layer(next_node_in_stack_id) {
					current_index += 1;
				}
				// Input as a sibling to the Layer node above
				post_node_input_connector = InputConnector::node(*next_node_in_stack_id, 0);
			} else {
				log::error!("Error creating layer: insert_index out of bounds");
				break;
			};
		}

		let mut pre_node_output_connector = None;

		// Sink post_node down to the end of the non layer chain that feeds into post_node, such that pre_node is the layer node at insert_index + 1, or None if insert_index is the last layer
		loop {
			pre_node_output_connector = network_interface.get_upstream_output_connector(&post_node_input_connector);

			match pre_node_output_connector {
				Some(OutputConnector::Node { node_id: pre_node_id, .. }) if !network_interface.is_layer(&pre_node_id) => {
					// Update post_node_input_connector for the next iteration
					post_node_input_connector = InputConnector::node(pre_node_id, 0);
					// Reset pre_node_output_connector to None to fetch new input in the next iteration
					pre_node_output_connector = None;
				}
				_ => break, // Break if pre_node_output_connector is None or if pre_node_id is a layer
			}
		}

		(post_node_input_connector, pre_node_output_connector)
	}

	/// Creates a new layer and adds it to the document network. network_interface.move_layer_to_stack should be called after
	pub fn create_layer(&mut self, new_id: NodeId, parent: LayerNodeIdentifier) -> LayerNodeIdentifier {
		let mut new_merge_node = resolve_document_node_type("Merge").expect("Merge node").default_node_template();
		self.network_interface.insert_node(new_id, new_merge_node, true);
		LayerNodeIdentifier::new(new_id, &self.network_interface)
	}

	/// Creates an artboard as the primary export for the document network
	pub fn create_artboard(&mut self, new_id: NodeId, artboard: Artboard) {
		let mut artboard_node_template = resolve_document_node_type("Artboard").expect("Node").node_template_input_override([
			Some(NodeInput::value(TaggedValue::ArtboardGroup(graphene_std::ArtboardGroup::EMPTY), true)),
			Some(NodeInput::value(TaggedValue::GraphicGroup(graphene_core::GraphicGroup::EMPTY), true)),
			Some(NodeInput::value(TaggedValue::IVec2(artboard.location), false)),
			Some(NodeInput::value(TaggedValue::IVec2(artboard.dimensions), false)),
			Some(NodeInput::value(TaggedValue::Color(artboard.background), false)),
			Some(NodeInput::value(TaggedValue::Bool(artboard.clip), false)),
		]);

		self.network_interface.insert_node(new_id, artboard_node_template, true);
		self.network_interface
			.move_layer_to_stack(LayerNodeIdentifier::new_unchecked(new_id), LayerNodeIdentifier::ROOT_PARENT, 0);

		// If there is a non artboard feeding into the primary input of the artboard, move it to the secondary input
		let Some(artboard) = self.network_interface.document_network().nodes.get(&new_id) else {
			log::error!("Artboard not created");
			return;
		};
		let primary_input = artboard.inputs.get(0).expect("Artboard should have a primary input");
		if let NodeInput::Node { node_id, .. } = primary_input.clone() {
			let artboard_layer = LayerNodeIdentifier::new(new_id, &self.network_interface);
			if self.network_interface.is_layer(&node_id) {
				self.network_interface.move_node_to_chain(&node_id, artboard_layer)
			} else {
				self.network_interface
					.move_layer_to_stack(LayerNodeIdentifier::new(node_id, &self.network_interface), artboard_layer, 0);
			}
		}
	}
	pub fn insert_vector_data(&mut self, subpaths: Vec<Subpath<PointId>>, layer: LayerNodeIdentifier) {
		let shape = resolve_document_node_type("Shape")
			.expect("Shape node does not exist")
			.node_template_input_override([Some(NodeInput::value(TaggedValue::Subpaths(subpaths), false))]);

		let transform = resolve_document_node_type("Transform").expect("Transform node does not exist").default_node_template();
		let fill = resolve_document_node_type("Fill").expect("Fill node does not exist").default_node_template();
		let stroke = resolve_document_node_type("Stroke").expect("Stroke node does not exist").default_node_template();

		let stroke_id = NodeId(generate_uuid());
		self.insert_node_to_chain(stroke_id, layer, stroke);
		let fill_id = NodeId(generate_uuid());
		self.insert_node_to_chain(fill_id, layer, fill);
		let transform_id = NodeId(generate_uuid());
		self.insert_node_to_chain(transform_id, layer, transform);
		let shape_id = NodeId(generate_uuid());
		self.insert_node_to_chain(shape_id, layer, shape);
	}

	pub fn insert_text(&mut self, text: String, font: Font, size: f64, layer: LayerNodeIdentifier) {
		let stroke = resolve_document_node_type("Stroke").expect("Stroke node does not exist").default_node_template();
		let fill = resolve_document_node_type("Fill").expect("Fill node does not exist").default_node_template();
		let transform = resolve_document_node_type("Transform").expect("Transform node does not exist").default_node_template();
		let text = resolve_document_node_type("Text").expect("Text node does not exist").node_template_input_override([
			Some(NodeInput::network(graph_craft::concrete!(graphene_std::wasm_application_io::WasmEditorApi), 0)),
			Some(NodeInput::value(TaggedValue::String(text), false)),
			Some(NodeInput::value(TaggedValue::Font(font), false)),
			Some(NodeInput::value(TaggedValue::F64(size), false)),
		]);

		let stroke_id = NodeId(generate_uuid());
		self.insert_node_to_chain(stroke_id, layer, stroke);
		let fill_id = NodeId(generate_uuid());
		self.insert_node_to_chain(fill_id, layer, fill);
		let transform_id = NodeId(generate_uuid());
		self.insert_node_to_chain(transform_id, layer, transform);
		let text_id = NodeId(generate_uuid());
		self.insert_node_to_chain(text_id, layer, text);
		self.responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	pub fn insert_image_data(&mut self, image_frame: ImageFrame<Color>, layer: LayerNodeIdentifier) {
		let transform = resolve_document_node_type("Transform").expect("Transform node does not exist").default_node_template();
		let image = resolve_document_node_type("Image")
			.expect("Image node does not exist")
			.node_template_input_override([Some(NodeInput::value(TaggedValue::ImageFrame(image_frame), false))]);

		let transform_id = NodeId(generate_uuid());
		self.insert_node_to_chain(transform_id, layer, transform);
		let image_id = NodeId(generate_uuid());
		self.insert_node_to_chain(image_id, layer, image);
	}

	pub fn get_existing_node_id(&self, reference: &'static str) -> Option<NodeId> {
		self.network_interface
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
					|layer| vec![layer.to_node()],
				),
				network_interface::FlowType::HorizontalFlow,
			)
			.find(|(_, node_id)| self.network_interface.get_reference(node_id).is_some_and(|node_reference| node_reference == reference))
			.map(|(_, id)| id)
		// Create a new node if the node does not exist and update its inputs
		// TODO: Is this necessary?
	}

	pub fn fill_set(&mut self, fill: Fill) {
		let fill_index = 1;
		let backup_color_index = 2;
		let backup_gradient_index = 3;

		let Some(fill_node_id) = self.get_existing_node_id("Fill") else {
			return;
		};
		match &fill {
			Fill::None => {
				let input_connector = InputConnector::node(fill_node_id, backup_color_index);
				self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::OptionalColor(None), false), true);
			}
			Fill::Solid(color) => {
				let input_connector = InputConnector::node(fill_node_id, backup_color_index);
				self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::OptionalColor(Some(*color)), false), true);
			}
			Fill::Gradient(gradient) => {
				let input_connector = InputConnector::node(fill_node_id, backup_gradient_index);
				self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::Gradient(gradient.clone()), false), true);
			}
		}
		let input_connector = InputConnector::node(fill_node_id, fill_index);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::Fill(fill), false), false);
	}

	pub fn opacity_set(&mut self, opacity: f64) {
		let Some(opacity_node_id) = self.get_existing_node_id("Opacity") else {
			return;
		};
		let input_connector = InputConnector::node(opacity_node_id, 1);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::F64(opacity * 100.), false), false);
	}

	pub fn blend_mode_set(&mut self, blend_mode: BlendMode) {
		let Some(blend_mode_node_id) = self.get_existing_node_id("Blend Mode") else {
			return;
		};
		let input_connector = InputConnector::node(blend_mode_node_id, 1);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::BlendMode(blend_mode), false), false);
	}

	pub fn stroke_set(&mut self, stroke: Stroke) {
		let Some(stroke_node_id) = self.get_existing_node_id("Stroke") else {
			return;
		};

		let input_connector = InputConnector::node(stroke_node_id, 1);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::OptionalColor(stroke.color), false), true);
		let input_connector = InputConnector::node(stroke_node_id, 2);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::F64(stroke.weight), false), true);
		let input_connector = InputConnector::node(stroke_node_id, 3);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::VecF64(stroke.dash_lengths), false), true);
		let input_connector = InputConnector::node(stroke_node_id, 4);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::F64(stroke.dash_offset), false), true);
		let input_connector = InputConnector::node(stroke_node_id, 5);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::LineCap(stroke.line_cap), false), true);
		let input_connector = InputConnector::node(stroke_node_id, 6);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::LineJoin(stroke.line_join), false), true);
		let input_connector = InputConnector::node(stroke_node_id, 7);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::F64(stroke.line_join_miter_limit), false), false);
	}

	pub fn transform_change(&mut self, transform: DAffine2, transform_in: TransformIn, parent_transform: DAffine2, skip_rerender: bool) {
		let Some(transform_node_id) = self.get_existing_node_id("Transform") else {
			return;
		};
		let document_node = self.network_interface.document_network().nodes.get(&transform_node_id).unwrap();
		let layer_transform = transform_utils::get_current_transform(&document_node.inputs);
		let to = match transform_in {
			TransformIn::Local => DAffine2::IDENTITY,
			TransformIn::Scope { scope } => scope * parent_transform,
			TransformIn::Viewport => parent_transform,
		};
		let transform = to.inverse() * transform * to * layer_transform;
		transform_utils::update_transform(&mut self.network_interface, &transform_node_id, transform);

		self.responses.add(PropertiesPanelMessage::Refresh);

		if !skip_rerender {
			self.responses.add(NodeGraphMessage::RunDocumentGraph);
		}
	}

	pub fn transform_set(&mut self, mut transform: DAffine2, transform_in: TransformIn, parent_transform: DAffine2, current_transform: Option<DAffine2>, skip_rerender: bool) {
		let Some(transform_node_id) = self.get_existing_node_id("Transform") else {
			return;
		};
		let upstream_transform = self.network_interface.document_metadata().upstream_transform(transform_node_id);
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
		transform_utils::update_transform(&mut self.network_interface, &transform_node_id, final_transform);

		self.responses.add(PropertiesPanelMessage::Refresh);
		if !skip_rerender {
			self.responses.add(NodeGraphMessage::RunDocumentGraph);
		}
	}

	pub fn pivot_set(&mut self, new_pivot: DVec2) {
		let Some(transform_node_id) = self.get_existing_node_id("Transform") else {
			return;
		};

		self.network_interface
			.set_input(InputConnector::node(transform_node_id, 5), NodeInput::value(TaggedValue::DVec2(new_pivot), false), true);

		self.responses.add(PropertiesPanelMessage::Refresh);
		self.responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	pub fn vector_modify(&mut self, modification_type: VectorModificationType) {
		let Some(path_node_id) = self.get_existing_node_id("Path") else {
			return;
		};
		self.network_interface.vector_modify(&path_node_id, modification_type);
		self.responses.add(PropertiesPanelMessage::Refresh);
		self.responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	pub fn brush_modify(&mut self, strokes: Vec<BrushStroke>) {
		let Some(brush_node_id) = self.get_existing_node_id("Brush") else {
			return;
		};
		self.network_interface
			.set_input(InputConnector::node(brush_node_id, 2), NodeInput::value(TaggedValue::BrushStrokes(strokes), false), true);

		self.responses.add(PropertiesPanelMessage::Refresh);
		self.responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	pub fn resize_artboard(&mut self, location: IVec2, dimensions: IVec2) {
		let Some(artboard_node_id) = self.get_existing_node_id("Artboard") else {
			return;
		};

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
		self.network_interface
			.set_input(InputConnector::node(artboard_node_id, 2), NodeInput::value(TaggedValue::IVec2(location), false), true);
		self.network_interface
			.set_input(InputConnector::node(artboard_node_id, 3), NodeInput::value(TaggedValue::IVec2(dimensions), false), true);
	}

	/// Set the input, refresh the properties panel, and run the document graph if skip_rerender is false
	pub fn set_input_with_refresh(&mut self, input_connector: InputConnector, input: NodeInput, skip_rerender: bool) {
		self.network_interface.set_input(input_connector, input, true);
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
	pub fn insert_node_to_chain(&mut self, new_id: NodeId, parent: LayerNodeIdentifier, mut node_template: NodeTemplate) {
		assert!(
			self.network_interface.document_network().nodes.contains_key(&new_id),
			"add_node_to_chain only works in the document network"
		);
		// TODO: node layout system and implementation
	}

	/// Inserts a node as a child of a layer at a certain stack index. The position will be `Position::Stack(calculated y position)`
	pub fn insert_layer_to_stack(&mut self, new_id: NodeId, mut node_template: NodeTemplate, parent: LayerNodeIdentifier, insert_index: usize) {
		assert!(
			self.network_interface.document_network().nodes.contains_key(&new_id),
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

use super::transform_utils;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{self, InputConnector, NodeNetworkInterface, OutputConnector};
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
use graphene_std::vector::VectorData;

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
	/// -> NonLayerNode
	///      ↑      if insert_index == 2, return (NonLayerNode, Some(Layer3))
	/// -> Layer3  
	///             if insert_index == 3, return (Layer3, None)
	pub fn get_post_node_with_index(network_interface: &NodeNetworkInterface, parent: LayerNodeIdentifier, insert_index: usize) -> InputConnector {
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
					.input_from_connector(&post_node_input_connector, &[])
					.and_then(|input_from_connector| if let NodeInput::Node { node_id, .. } = input_from_connector { Some(node_id) } else { None });

			if let Some(next_node_in_stack_id) = next_node_in_stack_id {
				// Only increment index for layer nodes
				if network_interface.is_layer(next_node_in_stack_id, &[]) {
					current_index += 1;
				}
				// Input as a sibling to the Layer node above
				post_node_input_connector = InputConnector::node(*next_node_in_stack_id, 0);
			} else {
				log::error!("Error getting post node: insert_index out of bounds");
				break;
			};
		}

		// Sink post_node down to the end of the non layer chain that feeds into post_node, such that pre_node is the layer node at insert_index + 1, or None if insert_index is the last layer
		loop {
			let pre_node_output_connector = network_interface.upstream_output_connector(&post_node_input_connector, &[]);

			match pre_node_output_connector {
				Some(OutputConnector::Node { node_id: pre_node_id, .. }) if !network_interface.is_layer(&pre_node_id, &[]) => {
					// Update post_node_input_connector for the next iteration
					post_node_input_connector = InputConnector::node(pre_node_id, 0);
				}
				_ => break, // Break if pre_node_output_connector is None or if pre_node_id is a layer
			}
		}

		post_node_input_connector
	}

	/// Creates a new layer and adds it to the document network. network_interface.move_layer_to_stack should be called after
	pub fn create_layer(&mut self, new_id: NodeId) -> LayerNodeIdentifier {
		let new_merge_node = resolve_document_node_type("Merge").expect("Merge node").default_node_template();
		self.network_interface.insert_node(new_id, new_merge_node, &[]);
		LayerNodeIdentifier::new(new_id, self.network_interface, &[])
	}

	/// Creates an artboard as the primary export for the document network
	pub fn create_artboard(&mut self, new_id: NodeId, artboard: Artboard) -> LayerNodeIdentifier {
		let artboard_node_template = resolve_document_node_type("Artboard").expect("Node").node_template_input_override([
			Some(NodeInput::value(TaggedValue::ArtboardGroup(graphene_std::ArtboardGroup::EMPTY), true)),
			Some(NodeInput::value(TaggedValue::GraphicGroup(graphene_core::GraphicGroup::EMPTY), true)),
			Some(NodeInput::value(TaggedValue::IVec2(artboard.location), false)),
			Some(NodeInput::value(TaggedValue::IVec2(artboard.dimensions), false)),
			Some(NodeInput::value(TaggedValue::Color(artboard.background), false)),
			Some(NodeInput::value(TaggedValue::Bool(artboard.clip), false)),
		]);
		self.network_interface.insert_node(new_id, artboard_node_template, &[]);
		LayerNodeIdentifier::new(new_id, self.network_interface, &[])
	}

	pub fn insert_boolean_data(&mut self, operation: graphene_std::vector::misc::BooleanOperation, layer: LayerNodeIdentifier) {
		let boolean = resolve_document_node_type("Boolean Operation").expect("Boolean node does not exist").node_template_input_override([
			Some(NodeInput::value(TaggedValue::GraphicGroup(graphene_std::GraphicGroup::EMPTY), true)),
			Some(NodeInput::value(TaggedValue::BooleanOperation(operation), false)),
		]);

		let boolean_id = NodeId(generate_uuid());
		self.network_interface.insert_node(boolean_id, boolean, &[]);
		self.network_interface.move_node_to_chain_start(&boolean_id, layer, &[]);
	}

	pub fn insert_vector_data(&mut self, subpaths: Vec<Subpath<PointId>>, layer: LayerNodeIdentifier, include_transform: bool, include_fill: bool, include_stroke: bool) {
		let vector_data = VectorData::from_subpaths(subpaths, true);

		let shape = resolve_document_node_type("Path")
			.expect("Path node does not exist")
			.node_template_input_override([Some(NodeInput::value(TaggedValue::VectorData(vector_data), false))]);
		let shape_id = NodeId(generate_uuid());
		self.network_interface.insert_node(shape_id, shape, &[]);
		self.network_interface.move_node_to_chain_start(&shape_id, layer, &[]);

		if include_transform {
			let transform = resolve_document_node_type("Transform").expect("Transform node does not exist").default_node_template();
			let transform_id = NodeId(generate_uuid());
			self.network_interface.insert_node(transform_id, transform, &[]);
			self.network_interface.move_node_to_chain_start(&transform_id, layer, &[]);
		}

		if include_fill {
			let fill = resolve_document_node_type("Fill").expect("Fill node does not exist").default_node_template();
			let fill_id = NodeId(generate_uuid());
			self.network_interface.insert_node(fill_id, fill, &[]);
			self.network_interface.move_node_to_chain_start(&fill_id, layer, &[]);
		}

		if include_stroke {
			let stroke = resolve_document_node_type("Stroke").expect("Stroke node does not exist").default_node_template();
			let stroke_id = NodeId(generate_uuid());
			self.network_interface.insert_node(stroke_id, stroke, &[]);
			self.network_interface.move_node_to_chain_start(&stroke_id, layer, &[]);
		}
	}

	pub fn insert_text(&mut self, text: String, font: Font, size: f64, line_height_ratio: f64, character_spacing: f64, layer: LayerNodeIdentifier) {
		let stroke = resolve_document_node_type("Stroke").expect("Stroke node does not exist").default_node_template();
		let fill = resolve_document_node_type("Fill").expect("Fill node does not exist").default_node_template();
		let transform = resolve_document_node_type("Transform").expect("Transform node does not exist").default_node_template();
		let text = resolve_document_node_type("Text").expect("Text node does not exist").node_template_input_override([
			Some(NodeInput::scope("editor-api")),
			Some(NodeInput::value(TaggedValue::String(text), false)),
			Some(NodeInput::value(TaggedValue::Font(font), false)),
			Some(NodeInput::value(TaggedValue::F64(size), false)),
			Some(NodeInput::value(TaggedValue::F64(line_height_ratio), false)),
			Some(NodeInput::value(TaggedValue::F64(character_spacing), false)),
		]);

		let text_id = NodeId(generate_uuid());
		self.network_interface.insert_node(text_id, text, &[]);
		self.network_interface.move_node_to_chain_start(&text_id, layer, &[]);

		let transform_id = NodeId(generate_uuid());
		self.network_interface.insert_node(transform_id, transform, &[]);
		self.network_interface.move_node_to_chain_start(&transform_id, layer, &[]);

		let fill_id = NodeId(generate_uuid());
		self.network_interface.insert_node(fill_id, fill, &[]);
		self.network_interface.move_node_to_chain_start(&fill_id, layer, &[]);

		let stroke_id = NodeId(generate_uuid());
		self.network_interface.insert_node(stroke_id, stroke, &[]);
		self.network_interface.move_node_to_chain_start(&stroke_id, layer, &[]);
	}

	pub fn insert_image_data(&mut self, image_frame: ImageFrame<Color>, layer: LayerNodeIdentifier) {
		let transform = resolve_document_node_type("Transform").expect("Transform node does not exist").default_node_template();
		let image = resolve_document_node_type("Image")
			.expect("Image node does not exist")
			.node_template_input_override([Some(NodeInput::value(TaggedValue::ImageFrame(image_frame), false))]);

		let image_id = NodeId(generate_uuid());
		self.network_interface.insert_node(image_id, image, &[]);
		self.network_interface.move_node_to_chain_start(&image_id, layer, &[]);

		let transform_id = NodeId(generate_uuid());
		self.network_interface.insert_node(transform_id, transform, &[]);
		self.network_interface.move_node_to_chain_start(&transform_id, layer, &[]);
	}

	fn get_output_layer(&self) -> Option<LayerNodeIdentifier> {
		self.layer_node.or_else(|| {
			let Some(network) = self.network_interface.network(&[]) else {
				log::error!("Document network does not exist in ModifyInputsContext::get_output_node");
				return None;
			};
			let export_node = network.exports.first().and_then(|export| export.as_node())?;
			if self.network_interface.is_layer(&export_node, &[]) {
				Some(LayerNodeIdentifier::new(export_node, self.network_interface, &[]))
			} else {
				None
			}
		})
	}
	// Gets the node id of a node with a specific reference that is upstream from the layer node, and creates it if it does not exist
	pub fn existing_node_id(&mut self, reference: &'static str) -> Option<NodeId> {
		// Start from the layer node or export
		let output_layer = self.get_output_layer()?;
		let layer_input_type = self.network_interface.input_type(&InputConnector::node(output_layer.to_node(), 1), &[]).0.nested_type();

		let upstream = self
			.network_interface
			.upstream_flow_back_from_nodes(vec![output_layer.to_node()], &[], network_interface::FlowType::HorizontalFlow);

		// Take until another layer node is found (but not the first layer node)
		let mut existing_node_id = None;
		for upstream_node in upstream.collect::<Vec<_>>() {
			let upstream_node_input_type = self.network_interface.input_type(&InputConnector::node(upstream_node, 0), &[]).0.nested_type();

			// Check if this is the node we have been searching for.
			if self.network_interface.reference(&upstream_node, &[]).is_some_and(|node_reference| node_reference == reference) {
				existing_node_id = Some(upstream_node);
				break;
			}

			let is_traversal_start = |node_id: NodeId| {
				self.layer_node.map(|layer| layer.to_node()) == Some(node_id) || self.network_interface.network(&[]).unwrap().exports.iter().any(|export| export.as_node() == Some(node_id))
			};

			// If the type changes then break?? This should at least be after checking if the node is correct (otherwise the brush tool breaks.)
			if !is_traversal_start(upstream_node) && (self.network_interface.is_layer(&upstream_node, &[]) || upstream_node_input_type != layer_input_type) {
				break;
			}
		}

		// Create a new node if the node does not exist and update its inputs
		existing_node_id.or_else(|| {
			let output_layer = self.get_output_layer()?;
			let Some(node_definition) = resolve_document_node_type(reference) else {
				log::error!("Node type {} does not exist in ModifyInputsContext::existing_node_id", reference);
				return None;
			};
			let node_id = NodeId(generate_uuid());
			self.network_interface.insert_node(node_id, node_definition.default_node_template(), &[]);
			self.network_interface.move_node_to_chain_start(&node_id, output_layer, &[]);
			Some(node_id)
		})
	}

	pub fn fill_set(&mut self, fill: Fill) {
		let fill_index = 1;
		let backup_color_index = 2;
		let backup_gradient_index = 3;

		let Some(fill_node_id) = self.existing_node_id("Fill") else { return };
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
		let Some(opacity_node_id) = self.existing_node_id("Opacity") else { return };
		let input_connector = InputConnector::node(opacity_node_id, 1);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::F64(opacity * 100.), false), false);
	}

	pub fn blend_mode_set(&mut self, blend_mode: BlendMode) {
		let Some(blend_mode_node_id) = self.existing_node_id("Blend Mode") else {
			return;
		};
		let input_connector = InputConnector::node(blend_mode_node_id, 1);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::BlendMode(blend_mode), false), false);
	}

	pub fn stroke_set(&mut self, stroke: Stroke) {
		let Some(stroke_node_id) = self.existing_node_id("Stroke") else { return };

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
		let Some(transform_node_id) = self.existing_node_id("Transform") else { return };
		let document_node = self.network_interface.network(&[]).unwrap().nodes.get(&transform_node_id).unwrap();
		let layer_transform = transform_utils::get_current_transform(&document_node.inputs);
		let to = match transform_in {
			TransformIn::Local => DAffine2::IDENTITY,
			TransformIn::Scope { scope } => scope * parent_transform,
			TransformIn::Viewport => parent_transform,
		};
		let transform = to.inverse() * transform * to * layer_transform;
		transform_utils::update_transform(self.network_interface, &transform_node_id, transform);

		self.responses.add(PropertiesPanelMessage::Refresh);

		if !skip_rerender {
			self.responses.add(NodeGraphMessage::RunDocumentGraph);
		}
	}

	pub fn transform_set(&mut self, transform: DAffine2, transform_in: TransformIn, skip_rerender: bool) {
		let final_transform = match transform_in {
			TransformIn::Local => DAffine2::IDENTITY * transform,
			TransformIn::Scope { scope } => scope * transform,
			TransformIn::Viewport => self.network_interface.document_metadata().downstream_transform_to_viewport(self.layer_node.unwrap()).inverse() * transform,
		};

		let Some(transform_node_id) = self.existing_node_id("Transform") else { return };

		transform_utils::update_transform(self.network_interface, &transform_node_id, final_transform);

		self.responses.add(PropertiesPanelMessage::Refresh);
		if !skip_rerender {
			self.responses.add(NodeGraphMessage::RunDocumentGraph);
		}
	}

	pub fn pivot_set(&mut self, new_pivot: DVec2) {
		let Some(transform_node_id) = self.existing_node_id("Transform") else { return };

		self.set_input_with_refresh(InputConnector::node(transform_node_id, 5), NodeInput::value(TaggedValue::DVec2(new_pivot), false), false);
	}

	pub fn vector_modify(&mut self, modification_type: VectorModificationType) {
		let Some(path_node_id) = self.existing_node_id("Path") else { return };
		self.network_interface.vector_modify(&path_node_id, modification_type);
		self.responses.add(PropertiesPanelMessage::Refresh);
		self.responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	pub fn brush_modify(&mut self, strokes: Vec<BrushStroke>) {
		let Some(brush_node_id) = self.existing_node_id("Brush") else { return };
		self.set_input_with_refresh(InputConnector::node(brush_node_id, 2), NodeInput::value(TaggedValue::BrushStrokes(strokes), false), false);
	}

	pub fn resize_artboard(&mut self, location: IVec2, dimensions: IVec2) {
		let Some(artboard_node_id) = self.existing_node_id("Artboard") else {
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
		self.set_input_with_refresh(InputConnector::node(artboard_node_id, 2), NodeInput::value(TaggedValue::IVec2(location), false), false);
		self.set_input_with_refresh(InputConnector::node(artboard_node_id, 3), NodeInput::value(TaggedValue::IVec2(dimensions), false), false);
	}

	/// Set the input, refresh the properties panel, and run the document graph if skip_rerender is false
	pub fn set_input_with_refresh(&mut self, input_connector: InputConnector, input: NodeInput, skip_rerender: bool) {
		self.network_interface.set_input(&input_connector, input, &[]);
		self.responses.add(PropertiesPanelMessage::Refresh);
		if !skip_rerender {
			self.responses.add(NodeGraphMessage::RunDocumentGraph);
		}
	}
}

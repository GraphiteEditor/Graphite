use super::transform_utils;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{self, InputConnector, NodeNetworkInterface, OutputConnector};
use crate::messages::prelude::*;
use bezier_rs::Subpath;
use glam::{DAffine2, IVec2};
use graph_craft::concrete;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeId, NodeInput};
use graphene_std::Artboard;
use graphene_std::brush::brush_stroke::BrushStroke;
use graphene_std::raster::BlendMode;
use graphene_std::raster_types::{CPU, Raster};
use graphene_std::table::Table;
use graphene_std::text::{Font, TypesettingConfig};
use graphene_std::vector::Vector;
use graphene_std::vector::style::{Fill, Stroke};
use graphene_std::vector::{PointId, VectorModificationType};
use graphene_std::{Graphic, NodeInputDecleration};

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
			let next_node_in_stack_id = network_interface
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

		let layer_input_connector = post_node_input_connector;

		// Sink post_node down to the end of the non layer chain that feeds into post_node, such that pre_node is the layer node at insert_index + 1, or None if insert_index is the last layer
		loop {
			let pre_node_output_connector = network_interface.upstream_output_connector(&post_node_input_connector, &[]);

			match pre_node_output_connector {
				Some(OutputConnector::Node { node_id: pre_node_id, .. }) if !network_interface.is_layer(&pre_node_id, &[]) => {
					// Update post_node_input_connector for the next iteration
					post_node_input_connector = InputConnector::node(pre_node_id, 0);
					// Insert directly under layer if moving to the end of a layer stack that ends with a non layer node that does not have an exposed primary input
					let primary_is_exposed = network_interface.input_from_connector(&post_node_input_connector, &[]).is_some_and(|input| input.is_exposed());
					if !primary_is_exposed {
						return layer_input_connector;
					}
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
		LayerNodeIdentifier::new(new_id, self.network_interface)
	}

	/// Creates an artboard as the primary export for the document network
	pub fn create_artboard(&mut self, new_id: NodeId, artboard: Artboard) -> LayerNodeIdentifier {
		let artboard_node_template = resolve_document_node_type("Artboard").expect("Node").node_template_input_override([
			Some(NodeInput::value(TaggedValue::Artboard(Default::default()), true)),
			Some(NodeInput::value(TaggedValue::Group(Default::default()), true)),
			Some(NodeInput::value(TaggedValue::DVec2(artboard.location.into()), false)),
			Some(NodeInput::value(TaggedValue::DVec2(artboard.dimensions.into()), false)),
			Some(NodeInput::value(TaggedValue::Color(artboard.background), false)),
			Some(NodeInput::value(TaggedValue::Bool(artboard.clip), false)),
		]);
		self.network_interface.insert_node(new_id, artboard_node_template, &[]);
		LayerNodeIdentifier::new(new_id, self.network_interface)
	}

	pub fn insert_boolean_data(&mut self, operation: graphene_std::path_bool::BooleanOperation, layer: LayerNodeIdentifier) {
		let boolean = resolve_document_node_type("Boolean Operation").expect("Boolean node does not exist").node_template_input_override([
			Some(NodeInput::value(TaggedValue::Group(Default::default()), true)),
			Some(NodeInput::value(TaggedValue::BooleanOperation(operation), false)),
		]);

		let boolean_id = NodeId::new();
		self.network_interface.insert_node(boolean_id, boolean, &[]);
		self.network_interface.move_node_to_chain_start(&boolean_id, layer, &[]);
	}

	pub fn insert_vector(&mut self, subpaths: Vec<Subpath<PointId>>, layer: LayerNodeIdentifier, include_transform: bool, include_fill: bool, include_stroke: bool) {
		let vector = Table::new_from_element(Vector::from_subpaths(subpaths, true));

		let shape = resolve_document_node_type("Path")
			.expect("Path node does not exist")
			.node_template_input_override([Some(NodeInput::value(TaggedValue::Vector(vector), false))]);
		let shape_id = NodeId::new();
		self.network_interface.insert_node(shape_id, shape, &[]);
		self.network_interface.move_node_to_chain_start(&shape_id, layer, &[]);

		if include_transform {
			let transform = resolve_document_node_type("Transform").expect("Transform node does not exist").default_node_template();
			let transform_id = NodeId::new();
			self.network_interface.insert_node(transform_id, transform, &[]);
			self.network_interface.move_node_to_chain_start(&transform_id, layer, &[]);
		}

		if include_fill {
			let fill = resolve_document_node_type("Fill").expect("Fill node does not exist").default_node_template();
			let fill_id = NodeId::new();
			self.network_interface.insert_node(fill_id, fill, &[]);
			self.network_interface.move_node_to_chain_start(&fill_id, layer, &[]);
		}

		if include_stroke {
			let stroke = resolve_document_node_type("Stroke").expect("Stroke node does not exist").default_node_template();
			let stroke_id = NodeId::new();
			self.network_interface.insert_node(stroke_id, stroke, &[]);
			self.network_interface.move_node_to_chain_start(&stroke_id, layer, &[]);
		}
	}

	pub fn insert_text(&mut self, text: String, font: Font, typesetting: TypesettingConfig, layer: LayerNodeIdentifier) {
		let stroke = resolve_document_node_type("Stroke").expect("Stroke node does not exist").default_node_template();
		let fill = resolve_document_node_type("Fill").expect("Fill node does not exist").default_node_template();
		let transform = resolve_document_node_type("Transform").expect("Transform node does not exist").default_node_template();
		let text = resolve_document_node_type("Text").expect("Text node does not exist").node_template_input_override([
			Some(NodeInput::scope("editor-api")),
			Some(NodeInput::value(TaggedValue::String(text), false)),
			Some(NodeInput::value(TaggedValue::Font(font), false)),
			Some(NodeInput::value(TaggedValue::F64(typesetting.font_size), false)),
			Some(NodeInput::value(TaggedValue::F64(typesetting.line_height_ratio), false)),
			Some(NodeInput::value(TaggedValue::F64(typesetting.character_spacing), false)),
			Some(NodeInput::value(TaggedValue::OptionalF64(typesetting.max_width), false)),
			Some(NodeInput::value(TaggedValue::OptionalF64(typesetting.max_height), false)),
			Some(NodeInput::value(TaggedValue::F64(typesetting.tilt), false)),
			Some(NodeInput::value(TaggedValue::TextAlign(typesetting.align), false)),
		]);

		let text_id = NodeId::new();
		self.network_interface.insert_node(text_id, text, &[]);
		self.network_interface.move_node_to_chain_start(&text_id, layer, &[]);

		let transform_id = NodeId::new();
		self.network_interface.insert_node(transform_id, transform, &[]);
		self.network_interface.move_node_to_chain_start(&transform_id, layer, &[]);

		let fill_id = NodeId::new();
		self.network_interface.insert_node(fill_id, fill, &[]);
		self.network_interface.move_node_to_chain_start(&fill_id, layer, &[]);

		let stroke_id = NodeId::new();
		self.network_interface.insert_node(stroke_id, stroke, &[]);
		self.network_interface.move_node_to_chain_start(&stroke_id, layer, &[]);
	}

	pub fn insert_image_data(&mut self, image_frame: Table<Raster<CPU>>, layer: LayerNodeIdentifier) {
		let transform = resolve_document_node_type("Transform").expect("Transform node does not exist").default_node_template();
		let image = resolve_document_node_type("Image Value")
			.expect("ImageValue node does not exist")
			.node_template_input_override([Some(NodeInput::value(TaggedValue::None, false)), Some(NodeInput::value(TaggedValue::Raster(image_frame), false))]);

		let image_id = NodeId::new();
		self.network_interface.insert_node(image_id, image, &[]);
		self.network_interface.move_node_to_chain_start(&image_id, layer, &[]);

		let transform_id = NodeId::new();
		self.network_interface.insert_node(transform_id, transform, &[]);
		self.network_interface.move_node_to_chain_start(&transform_id, layer, &[]);
	}

	fn get_output_layer(&self) -> Option<LayerNodeIdentifier> {
		self.layer_node.or_else(|| {
			let export_node = self.network_interface.document_network().exports.first().and_then(|export| export.as_node())?;
			if self.network_interface.is_layer(&export_node, &[]) {
				Some(LayerNodeIdentifier::new(export_node, self.network_interface))
			} else {
				None
			}
		})
	}

	/// Gets the node id of a node with a specific reference that is upstream from the layer node, and optionally creates it if it does not exist.
	/// The returned node is based on the selection dots in the layer. The right most dot will always insert/access the path that flows directly into the layer.
	/// Each dot after that represents an existing path node. If there is an existing upstream node, then it will always be returned first.
	pub fn existing_node_id(&mut self, reference_name: &'static str, create_if_nonexistent: bool) -> Option<NodeId> {
		// Start from the layer node or export
		let output_layer = self.get_output_layer()?;

		let existing_node_id = Self::locate_node_in_layer_chain(reference_name, output_layer, self.network_interface);

		// Create a new node if the node does not exist and update its inputs
		if create_if_nonexistent {
			return existing_node_id.or_else(|| self.create_node(reference_name));
		}

		existing_node_id
	}

	/// Gets the node id of a node with a specific reference (name) that is upstream (leftward) from the layer node, but before reaching another upstream layer stack.
	/// For example, if given a group layer, this would find a requested "Transform" or "Boolean Operation" node in its chain, between the group layer and its layer stack child contents.
	/// It would also travel up an entire layer that's not fed by a stack until reaching the generator node, such as a "Rectangle" or "Path" layer.
	pub fn locate_node_in_layer_chain(reference_name: &str, left_of_layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<NodeId> {
		let upstream = network_interface.upstream_flow_back_from_nodes(vec![left_of_layer.to_node()], &[], network_interface::FlowType::HorizontalFlow);

		// Look at all of the upstream nodes
		for upstream_node in upstream {
			// Check if this is the node we have been searching for.
			if network_interface
				.reference(&upstream_node, &[])
				.is_some_and(|node_reference| *node_reference == Some(reference_name.to_string()))
			{
				if !network_interface.is_visible(&upstream_node, &[]) {
					continue;
				}

				return Some(upstream_node);
			}

			// Take until another layer node is found (but not the first layer node)
			let is_traversal_start = |node_id: NodeId| left_of_layer.to_node() == node_id || network_interface.document_network().exports.iter().any(|export| export.as_node() == Some(node_id));
			if !is_traversal_start(upstream_node) && (network_interface.is_layer(&upstream_node, &[])) {
				return None;
			}
		}

		None
	}

	/// Create a new node inside the layer
	pub fn create_node(&mut self, reference: &str) -> Option<NodeId> {
		let output_layer = self.get_output_layer()?;
		let Some(node_definition) = resolve_document_node_type(reference) else {
			log::error!("Node type {reference} does not exist in ModifyInputsContext::existing_node_id");
			return None;
		};

		// If inserting a path node, insert a Flatten Path if the type is Group.
		// TODO: Allow the path node to operate on Group data by utilizing the reference for each Vector in a group.
		if node_definition.identifier == "Path" {
			let layer_input_type = self.network_interface.input_type(&InputConnector::node(output_layer.to_node(), 1), &[]).0.nested_type().clone();
			if layer_input_type == concrete!(Table<Graphic>) {
				let Some(flatten_path_definition) = resolve_document_node_type("Flatten Path") else {
					log::error!("Flatten Path does not exist in ModifyInputsContext::existing_node_id");
					return None;
				};
				let node_id = NodeId::new();
				self.network_interface.insert_node(node_id, flatten_path_definition.default_node_template(), &[]);
				self.network_interface.move_node_to_chain_start(&node_id, output_layer, &[]);
			}
		}
		let node_id = NodeId::new();
		self.network_interface.insert_node(node_id, node_definition.default_node_template(), &[]);
		self.network_interface.move_node_to_chain_start(&node_id, output_layer, &[]);
		Some(node_id)
	}

	pub fn fill_set(&mut self, fill: Fill) {
		let fill_index = 1;
		let backup_color_index = 2;
		let backup_gradient_index = 3;

		let Some(fill_node_id) = self.existing_node_id("Fill", true) else { return };
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

	pub fn blend_mode_set(&mut self, blend_mode: BlendMode) {
		let Some(blend_node_id) = self.existing_node_id("Blending", true) else { return };
		let input_connector = InputConnector::node(blend_node_id, 1);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::BlendMode(blend_mode), false), false);
	}

	pub fn opacity_set(&mut self, opacity: f64) {
		let Some(blend_node_id) = self.existing_node_id("Blending", true) else { return };
		let input_connector = InputConnector::node(blend_node_id, 2);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::F64(opacity * 100.), false), false);
	}

	pub fn blending_fill_set(&mut self, fill: f64) {
		let Some(blend_node_id) = self.existing_node_id("Blending", true) else { return };
		let input_connector = InputConnector::node(blend_node_id, 3);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::F64(fill * 100.), false), false);
	}

	pub fn clip_mode_toggle(&mut self, clip_mode: Option<bool>) {
		let clip = !clip_mode.unwrap_or(false);
		let Some(clip_node_id) = self.existing_node_id("Blending", true) else { return };
		let input_connector = InputConnector::node(clip_node_id, 4);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::Bool(clip), false), false);
	}

	pub fn stroke_set(&mut self, stroke: Stroke) {
		let Some(stroke_node_id) = self.existing_node_id("Stroke", true) else { return };

		let input_connector = InputConnector::node(stroke_node_id, graphene_std::vector::stroke::ColorInput::<Option<graphene_std::Color>>::INDEX);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::OptionalColor(stroke.color), false), true);
		let input_connector = InputConnector::node(stroke_node_id, graphene_std::vector::stroke::WeightInput::INDEX);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::F64(stroke.weight), false), true);
		let input_connector = InputConnector::node(stroke_node_id, graphene_std::vector::stroke::AlignInput::INDEX);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::StrokeAlign(stroke.align), false), false);
		let input_connector = InputConnector::node(stroke_node_id, graphene_std::vector::stroke::CapInput::INDEX);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::StrokeCap(stroke.cap), false), true);
		let input_connector = InputConnector::node(stroke_node_id, graphene_std::vector::stroke::JoinInput::INDEX);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::StrokeJoin(stroke.join), false), true);
		let input_connector = InputConnector::node(stroke_node_id, graphene_std::vector::stroke::MiterLimitInput::INDEX);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::F64(stroke.join_miter_limit), false), false);
		let input_connector = InputConnector::node(stroke_node_id, graphene_std::vector::stroke::PaintOrderInput::INDEX);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::PaintOrder(stroke.paint_order), false), false);
		let input_connector = InputConnector::node(stroke_node_id, graphene_std::vector::stroke::DashLengthsInput::INDEX);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::VecF64(stroke.dash_lengths), false), true);
		let input_connector = InputConnector::node(stroke_node_id, graphene_std::vector::stroke::DashOffsetInput::INDEX);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::F64(stroke.dash_offset), false), true);
	}

	/// Update the transform value of the upstream Transform node based a change to its existing value and the given parent transform.
	/// A new Transform node is created if one does not exist, unless it would be given the identity transform.
	pub fn transform_change_with_parent(&mut self, transform: DAffine2, transform_in: TransformIn, parent_transform: DAffine2, skip_rerender: bool) {
		// Get the existing upstream Transform node and its transform, if present, otherwise use the identity transform
		let (layer_transform, transform_node_id) = self
			.existing_node_id("Transform", false)
			.and_then(|transform_node_id| {
				let document_node = self.network_interface.document_network().nodes.get(&transform_node_id)?;
				Some((transform_utils::get_current_transform(&document_node.inputs), transform_node_id))
			})
			.unzip();
		let layer_transform = layer_transform.unwrap_or_default();

		// Get a transform appropriate for the requested space
		let to_transform = match transform_in {
			TransformIn::Local => DAffine2::IDENTITY,
			TransformIn::Scope { scope } => scope * parent_transform,
			TransformIn::Viewport => parent_transform,
		};

		// Set the transform value to the Transform node
		let final_transform = to_transform.inverse() * transform * to_transform * layer_transform;
		self.transform_set_direct(final_transform, skip_rerender, transform_node_id);
	}

	/// Set the transform value to the upstream Transform node, replacing the existing value.
	/// A new Transform node is created if one does not exist, unless it would be given the identity transform.
	pub fn transform_set(&mut self, transform: DAffine2, transform_in: TransformIn, skip_rerender: bool) {
		// Get the existing upstream Transform node, if present
		let transform_node_id = self.existing_node_id("Transform", false);

		// Get a transform appropriate for the requested space
		let to_transform = match transform_in {
			TransformIn::Local => DAffine2::IDENTITY,
			TransformIn::Scope { scope } => scope,
			TransformIn::Viewport => self.network_interface.document_metadata().downstream_transform_to_viewport(self.layer_node.unwrap()).inverse(),
		};

		// Set the transform value to the Transform node
		let final_transform = to_transform * transform;
		self.transform_set_direct(final_transform, skip_rerender, transform_node_id);
	}

	/// Write the given transform value to the upstream Transform node, if one is supplied. If one doesn't exist, it will be created unless the given transform is the identity.
	pub fn transform_set_direct(&mut self, transform: DAffine2, skip_rerender: bool, transform_node_id: Option<NodeId>) {
		// If the Transform node didn't exist yet, create it now
		let Some(transform_node_id) = transform_node_id.or_else(|| {
			// Check if the transform is the identity transform (within an epsilon) and if so, don't create a new Transform node
			if transform.abs_diff_eq(DAffine2::IDENTITY, 1e-6) {
				// We don't want to pollute the graph with an unnecessary Transform node, so we avoid creating and setting it by returning None
				return None;
			}

			// Create the Transform node
			self.existing_node_id("Transform", true)
		}) else {
			return;
		};

		// Update the transform value of the Transform node
		transform_utils::update_transform(self.network_interface, &transform_node_id, transform);

		// Refresh the render and editor UI
		self.responses.add(PropertiesPanelMessage::Refresh);
		if !skip_rerender {
			self.responses.add(NodeGraphMessage::RunDocumentGraph);
		}
	}

	pub fn vector_modify(&mut self, modification_type: VectorModificationType) {
		let Some(path_node_id) = self.existing_node_id("Path", true) else { return };
		self.network_interface.vector_modify(&path_node_id, modification_type);
		self.responses.add(PropertiesPanelMessage::Refresh);
		self.responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	pub fn brush_modify(&mut self, strokes: Vec<BrushStroke>) {
		let Some(brush_node_id) = self.existing_node_id("Brush", true) else { return };
		self.set_input_with_refresh(InputConnector::node(brush_node_id, 1), NodeInput::value(TaggedValue::BrushStrokes(strokes), false), false);
	}

	pub fn resize_artboard(&mut self, location: IVec2, dimensions: IVec2) {
		let Some(artboard_node_id) = self.existing_node_id("Artboard", true) else {
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
		self.set_input_with_refresh(InputConnector::node(artboard_node_id, 2), NodeInput::value(TaggedValue::DVec2(location.into()), false), false);
		self.set_input_with_refresh(InputConnector::node(artboard_node_id, 3), NodeInput::value(TaggedValue::DVec2(dimensions.into()), false), false);
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

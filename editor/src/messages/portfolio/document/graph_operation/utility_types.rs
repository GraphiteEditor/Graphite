use super::transform_utils;
use crate::messages::portfolio::document::node_graph::document_node_definitions::{
	ARTBOARD_DIMENSIONS_INPUT_INDEX, ARTBOARD_LOCATION_INPUT_INDEX, DefinitionIdentifier, resolve_document_node_type, resolve_network_node_type, resolve_proto_node_type,
};
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{self, FlowType, InputConnector, NodeNetworkInterface};
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils::{
	ReplaceablePaintChain, get_fill_input_node_id, get_upstream_gradient_value_node_id, gradient_chain_target_input, replaceable_paint_chain,
};
use glam::{DAffine2, DVec2, IVec2};
use graph_craft::application_io::resource::ResourceId;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeId, NodeInput};
use graph_craft::{ProtoNodeIdentifier, list};
use graphene_std::brush::brush_stroke::BrushStroke;
use graphene_std::raster::BlendMode;
use graphene_std::raster_types::Image;
use graphene_std::subpath::Subpath;
use graphene_std::text::{Font, TypesettingConfig};
use graphene_std::vector::style::{GradientForm, GradientHueDirection, GradientInterpolation, GradientSpace, GradientSpread, Stroke};
use graphene_std::vector::{Gradient, GradientRamp, PointId, Vector, VectorModification, VectorModificationType};
use graphene_std::{Artboard, Color, Graphic};

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
	/// When true, uses lightweight import paths that skip expensive checks during bulk import.
	pub import: bool,
}

impl<'a> ModifyInputsContext<'a> {
	/// Get the node network from the document
	pub fn new(network_interface: &'a mut NodeNetworkInterface, responses: &'a mut VecDeque<Message>) -> Self {
		Self {
			network_interface,
			responses,
			layer_node: None,
			import: false,
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

	/// Creates a new layer and adds it to the document network. network_interface.move_layer_to_stack should be called after
	pub fn create_layer(&mut self, new_id: NodeId) -> LayerNodeIdentifier {
		let new_merge_node = resolve_network_node_type("Merge").expect("Merge node").default_node_template();
		self.network_interface.insert_node(new_id, new_merge_node, &[]);
		LayerNodeIdentifier::new(new_id, self.network_interface)
	}

	/// Creates an artboard as the primary export for the document network.
	pub fn create_artboard(&mut self, new_id: NodeId, location: DVec2, dimensions: DVec2, background: Color, clip: bool) -> LayerNodeIdentifier {
		let artboard_node_template = resolve_network_node_type("Artboard").expect("Node").node_template_input_override([
			Some(NodeInput::type_default(list!(Artboard), true)),
			Some(NodeInput::type_default(list!(Graphic), true)),
			Some(NodeInput::value(TaggedValue::DVec2(location), false)),
			Some(NodeInput::value(TaggedValue::DVec2(dimensions), false)),
			Some(NodeInput::value(TaggedValue::Color(background), false)),
			Some(NodeInput::value(TaggedValue::Bool(clip), false)),
		]);
		self.network_interface.insert_node(new_id, artboard_node_template, &[]);
		LayerNodeIdentifier::new(new_id, self.network_interface)
	}

	pub fn insert_boolean_data(&mut self, operation: graphene_std::vector::misc::BooleanOperation, layer: LayerNodeIdentifier) {
		let boolean = resolve_proto_node_type(graphene_std::path_bool_nodes::boolean_operation::IDENTIFIER)
			.expect("Boolean node does not exist")
			.node_template_input_override([
				Some(NodeInput::type_default(list!(Graphic), true)),
				Some(NodeInput::value(TaggedValue::BooleanOperation(operation), false)),
			]);

		let boolean_id = NodeId::new();
		self.network_interface.insert_node(boolean_id, boolean, &[]);
		self.network_interface.move_node_to_chain_start(&boolean_id, layer, &[], self.import);
	}

	pub fn insert_blend_data(&mut self, layer: LayerNodeIdentifier, count: f64) -> NodeId {
		let blend = resolve_network_node_type("Blend")
			.expect("Blend node does not exist")
			.node_template_input_override([Some(NodeInput::type_default(list!(Graphic), true)), Some(NodeInput::value(TaggedValue::F64(count), false))]);

		let blend_id = NodeId::new();
		self.network_interface.insert_node(blend_id, blend, &[]);
		self.network_interface.move_node_to_chain_start(&blend_id, layer, &[], self.import);

		blend_id
	}

	pub fn insert_morph_data(&mut self, layer: LayerNodeIdentifier) -> NodeId {
		let morph = resolve_proto_node_type(graphene_std::vector::morph::IDENTIFIER)
			.expect("Morph node does not exist")
			.node_template_input_override([Some(NodeInput::type_default(list!(Graphic), true)), Some(NodeInput::value(TaggedValue::F64(0.5), false))]);

		let morph_id = NodeId::new();
		self.network_interface.insert_node(morph_id, morph, &[]);
		self.network_interface.move_node_to_chain_start(&morph_id, layer, &[], self.import);

		morph_id
	}

	/// Returns the Path node ID (the node closest to the layer's merge node in the chain).
	pub fn insert_control_path_data(&mut self, layer: LayerNodeIdentifier) -> NodeId {
		// Add Origins to Polyline node first (will be pushed deepest in the chain)
		let origins_to_polyline = resolve_network_node_type("Origins to Polyline")
			.expect("Origins to Polyline node does not exist")
			.default_node_template();
		let origins_to_polyline_id = NodeId::new();
		self.network_interface.insert_node(origins_to_polyline_id, origins_to_polyline, &[]);
		self.network_interface.move_node_to_chain_start(&origins_to_polyline_id, layer, &[], self.import);

		// Add Auto-Tangents node (between Origins to Polyline and Path), with spread=1 and preserve_existing=false
		let auto_tangents = resolve_proto_node_type(graphene_std::vector::auto_tangents::IDENTIFIER)
			.expect("Auto-Tangents node does not exist")
			.node_template_input_override([None, Some(NodeInput::value(TaggedValue::F64(1.), false)), Some(NodeInput::value(TaggedValue::Bool(false), false))]);
		let auto_tangents_id = NodeId::new();
		self.network_interface.insert_node(auto_tangents_id, auto_tangents, &[]);
		self.network_interface.move_node_to_chain_start(&auto_tangents_id, layer, &[], self.import);

		// Add Path node to chain start (closest to the Merge node)
		let path = resolve_network_node_type("Path").expect("Path node does not exist").default_node_template();
		let path_id = NodeId::new();
		self.network_interface.insert_node(path_id, path, &[]);
		self.network_interface.move_node_to_chain_start(&path_id, layer, &[], self.import);

		path_id
	}

	pub fn insert_vector(&mut self, subpaths: Vec<Subpath<PointId>>, layer: LayerNodeIdentifier, include_transform: bool, include_fill: bool, include_stroke: bool) {
		// Build a VectorModification that reproduces the geometry (same format the Pen tool uses)
		let vector = Vector::from_subpaths(subpaths, true);
		let modification = Box::new(VectorModification::create_from_vector(&vector));

		let shape = resolve_network_node_type("Path")
			.expect("Path node does not exist")
			.node_template_input_override([None, Some(NodeInput::value(TaggedValue::VectorModification(modification), false))]);
		let shape_id = NodeId::new();
		self.network_interface.insert_node(shape_id, shape, &[]);
		self.network_interface.move_node_to_chain_start(&shape_id, layer, &[], self.import);

		if include_transform {
			let transform = resolve_proto_node_type(graphene_std::transform_nodes::transform::IDENTIFIER)
				.expect("Transform node does not exist")
				.default_node_template();
			let transform_id = NodeId::new();
			self.network_interface.insert_node(transform_id, transform, &[]);
			self.network_interface.move_node_to_chain_start(&transform_id, layer, &[], self.import);
		}

		if include_stroke {
			let stroke = resolve_proto_node_type(graphene_std::vector_nodes::stroke::IDENTIFIER)
				.expect("Stroke node does not exist")
				.default_node_template();
			let stroke_id = NodeId::new();
			self.network_interface.insert_node(stroke_id, stroke, &[]);
			self.network_interface.move_node_to_chain_start(&stroke_id, layer, &[], self.import);
		}

		if include_fill {
			let fill = resolve_proto_node_type(graphene_std::vector_nodes::fill::IDENTIFIER)
				.expect("Fill node does not exist")
				.default_node_template();
			let fill_id = NodeId::new();
			self.network_interface.insert_node(fill_id, fill, &[]);
			self.network_interface.move_node_to_chain_start(&fill_id, layer, &[], self.import);
		}
	}

	pub fn insert_text(&mut self, text: String, font: Font, typesetting: TypesettingConfig, layer: LayerNodeIdentifier) {
		let font_resource_id = ResourceId::new();
		let text = resolve_proto_node_type(graphene_std::text::text::IDENTIFIER)
			.expect("Text node does not exist")
			.node_template_input_override([
				Some(NodeInput::value(TaggedValue::None, false)),
				Some(NodeInput::value(TaggedValue::String(text), false)),
				Some(NodeInput::value(TaggedValue::Resource(font_resource_id), false)),
				Some(NodeInput::value(TaggedValue::F64(typesetting.font_size), false)),
				Some(NodeInput::value(TaggedValue::F64(typesetting.line_height_ratio), false)),
				Some(NodeInput::value(TaggedValue::F64(typesetting.letter_spacing), false)),
				Some(NodeInput::value(TaggedValue::F64(typesetting.letter_tilt), false)),
				Some(NodeInput::value(TaggedValue::Bool(typesetting.max_width.is_some()), false)),
				Some(NodeInput::value(TaggedValue::F64(typesetting.max_width.unwrap_or(100.)), false)),
				Some(NodeInput::value(TaggedValue::Bool(typesetting.max_height.is_some()), false)),
				Some(NodeInput::value(TaggedValue::F64(typesetting.max_height.unwrap_or(100.)), false)),
				Some(NodeInput::value(TaggedValue::TextAlign(typesetting.align), false)),
			]);
		let text_to_vector = resolve_proto_node_type(graphene_std::text::text_to_vector::IDENTIFIER)
			.expect("Text to Vector node does not exist")
			.default_node_template();
		let transform = resolve_proto_node_type(graphene_std::transform_nodes::transform::IDENTIFIER)
			.expect("Transform node does not exist")
			.default_node_template();
		let fill = resolve_proto_node_type(graphene_std::vector_nodes::fill::IDENTIFIER)
			.expect("Fill node does not exist")
			.default_node_template();

		// Build the chain `Text -> Text to Vector -> Transform -> Fill -> layer`
		let text_id = NodeId::new();
		self.network_interface.insert_node(text_id, text, &[]);
		self.network_interface.move_node_to_chain_start(&text_id, layer, &[], self.import);

		self.responses.add(DocumentMessage::Resource(ResourceMessage::AddFont { resource_id: font_resource_id, font }));

		let text_to_vector_id = NodeId::new();
		self.network_interface.insert_node(text_to_vector_id, text_to_vector, &[]);
		self.network_interface.move_node_to_chain_start(&text_to_vector_id, layer, &[], self.import);

		let transform_id = NodeId::new();
		self.network_interface.insert_node(transform_id, transform, &[]);
		self.network_interface.move_node_to_chain_start(&transform_id, layer, &[], self.import);

		let fill_id = NodeId::new();
		self.network_interface.insert_node(fill_id, fill, &[]);
		self.network_interface.move_node_to_chain_start(&fill_id, layer, &[], self.import);
	}

	pub fn insert_color_value(&mut self, color: Color, layer: LayerNodeIdentifier, attachment_input: InputConnector) -> NodeId {
		let color_value = resolve_proto_node_type(graphene_std::math_nodes::color_value::IDENTIFIER)
			.expect("Color Value node does not exist")
			.node_template_input_override([Some(NodeInput::value(TaggedValue::None, false)), Some(NodeInput::value(TaggedValue::Color(color), false))]);

		let color_value_id = NodeId::new();
		self.network_interface.insert_node(color_value_id, color_value, &[]);
		self.start_paint_chain(&color_value_id, layer, attachment_input);

		color_value_id
	}

	/// Clear the whole-expanse paint one tool left on a layer so the other can start its own chain there.
	/// Severing at the attachment detaches the layer from whatever the walk stopped at, which is the only part a node
	/// the rest of the graph also draws from is subjected to, since such a node is never among those deleted.
	fn clear_paint_chain(&mut self, paint_chain: &ReplaceablePaintChain) {
		self.network_interface.disconnect_input(&paint_chain.attachment_input, &[]);

		if !paint_chain.nodes.is_empty() {
			self.network_interface.delete_nodes(paint_chain.nodes.clone(), false, &[]);
		}
	}

	/// Wire a node that paints the layer's whole expanse into the start of its chain,
	/// or past the 'Transform' nodes a blank layer already carries so those go on applying to the paint.
	fn start_paint_chain(&mut self, node_id: &NodeId, layer: LayerNodeIdentifier, attachment_input: InputConnector) {
		let layer_content_input = InputConnector::layer_secondary_input(layer.to_node());

		if attachment_input == layer_content_input {
			self.network_interface.move_node_to_chain_start(node_id, layer, &[], self.import);
			return;
		}

		self.network_interface.set_input(&attachment_input, NodeInput::node(*node_id, 0), &[]);
		self.network_interface.set_chain_position(node_id, &[]);
	}

	pub fn insert_image_data(&mut self, image: Image<Color>, layer: LayerNodeIdentifier) {
		let transform = resolve_proto_node_type(graphene_std::transform_nodes::transform::IDENTIFIER)
			.expect("Transform node does not exist")
			.default_node_template();

		let resource_id = ResourceId::new();
		self.responses.add(ResourceMessage::StoreEmbedded {
			resource_id,
			data: image.to_png().into(),
		});

		let image_node = resolve_proto_node_type(graphene_std::raster_nodes::std_nodes::image::IDENTIFIER)
			.expect("Image node does not exist")
			.node_template_input_override([Some(NodeInput::value(TaggedValue::Resource(resource_id), false))]);

		let image_node_id = NodeId::new();
		self.network_interface.insert_node(image_node_id, image_node, &[]);
		self.network_interface.move_node_to_chain_start(&image_node_id, layer, &[], self.import);

		let transform_id = NodeId::new();
		self.network_interface.insert_node(transform_id, transform, &[]);
		self.network_interface.move_node_to_chain_start(&transform_id, layer, &[], self.import);
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

	/// Gets the node id of a network node with a specific reference that is upstream from the layer node, and optionally creates it if it does not exist.
	pub fn existing_network_node_id(&mut self, reference: &str, create_if_nonexistent: bool) -> Option<NodeId> {
		self.existing_node_id(&DefinitionIdentifier::Network(reference.into()), create_if_nonexistent)
	}

	/// Like `existing_proto_node_id`, but walks/inserts at `target_input` instead of the layer's content input.
	/// Used when a chain lives on a non-layer input.
	pub fn existing_proto_node_id_at(&mut self, target_input: &InputConnector, reference: ProtoNodeIdentifier, create_if_nonexistent: bool) -> Option<NodeId> {
		let identifier = DefinitionIdentifier::ProtoNode(reference.clone());

		// Walk upstream from whatever is currently connected to target_input
		let walk_start = self.network_interface.upstream_output_connector(target_input, &[]).and_then(|out| out.node_id());

		let existing = walk_start.and_then(|start| {
			self.network_interface
				.upstream_flow_back_from_nodes(vec![start], &[], FlowType::HorizontalFlow)
				.take_while(|id| !self.network_interface.is_layer(id, &[]))
				.find(|id| self.network_interface.reference(id, &[]).as_ref() == Some(&identifier) && self.network_interface.is_visible(id, &[]))
		});

		if let Some(id) = existing {
			return Some(id);
		}
		if !create_if_nonexistent {
			return None;
		}

		// Splice a new node onto the wire feeding `target_input`, positioning it sensibly within the chain.
		let node_definition = resolve_proto_node_type(reference)?;
		let node_id = NodeId::new();
		self.network_interface.insert_node(node_id, node_definition.default_node_template(), &[]);
		self.network_interface.insert_node_before_input(&node_id, target_input, &[]);

		Some(node_id)
	}

	/// Gets the node id of a proto node with a specific reference that is upstream from the layer node, and optionally creates it if it does not exist.
	pub fn existing_proto_node_id(&mut self, reference: ProtoNodeIdentifier, create_if_nonexistent: bool) -> Option<NodeId> {
		self.existing_node_id(&DefinitionIdentifier::ProtoNode(reference), create_if_nonexistent)
	}

	/// Gets the node id of a document node with a specific reference that is upstream from the layer node, and optionally creates it if it does not exist.
	fn existing_node_id(&mut self, reference: &DefinitionIdentifier, create_if_nonexistent: bool) -> Option<NodeId> {
		// Start from the layer node or export
		let output_layer = self.get_output_layer()?;

		let existing_node_id = Self::locate_node_in_layer_chain(reference, output_layer, self.network_interface);

		// Create a new node if the node does not exist and update its inputs
		if create_if_nonexistent {
			return existing_node_id.or_else(|| self.create_node(reference));
		}

		existing_node_id
	}

	/// Gets the node id of a node with a specific reference (name) that is upstream (leftward) from the layer node, but before reaching another upstream layer stack.
	/// For example, if given a parent layer, this would find a requested "Transform" or "Boolean Operation" node in its chain, between the parent layer and its layer stack child contents.
	/// It would also travel up an entire layer that's not fed by a stack until reaching the generator node, such as a "Rectangle" or "Path" layer.
	pub fn locate_node_in_layer_chain(reference: &DefinitionIdentifier, left_of_layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<NodeId> {
		let upstream = network_interface.upstream_flow_back_from_nodes(vec![left_of_layer.to_node()], &[], network_interface::FlowType::HorizontalFlow);

		// Look at all of the upstream nodes
		for upstream_node in upstream {
			// Check if this is the node we have been searching for.
			if network_interface.reference(&upstream_node, &[]).is_some_and(|node_reference| node_reference == *reference) {
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
	pub fn create_node(&mut self, reference: &DefinitionIdentifier) -> Option<NodeId> {
		let output_layer = self.get_output_layer()?;
		let Some(node_definition) = resolve_document_node_type(reference) else {
			log::error!("Node {reference:?} does not exist in ModifyInputsContext::existing_node_id");
			return None;
		};

		// If inserting a 'Path' node, insert a 'Combine Paths' node if the type is `Graphic`.
		// TODO: Allow the 'Path' node to operate on `List` data by utilizing the reference (index or ID?) for each item.
		if node_definition.identifier == "Path" {
			let layer_input_type = self.network_interface.input_type(&InputConnector::layer_secondary_input(output_layer.to_node()), &[]);
			if layer_input_type.compiled_element_name().as_deref() == Some("Graphic") {
				let Some(combine_paths_definition) = resolve_proto_node_type(graphene_std::vector_nodes::combine_paths::IDENTIFIER) else {
					log::error!("Combine Paths does not exist in ModifyInputsContext::existing_node_id");
					return None;
				};
				let node_id = NodeId::new();
				self.network_interface.insert_node(node_id, combine_paths_definition.default_node_template(), &[]);
				self.network_interface.move_node_to_chain_start(&node_id, output_layer, &[], self.import);
			}
		}
		let node_id = NodeId::new();
		self.network_interface.insert_node(node_id, node_definition.default_node_template(), &[]);
		self.network_interface.move_node_to_chain_start(&node_id, output_layer, &[], self.import);
		Some(node_id)
	}

	pub fn fill_color_set(&mut self, color: Option<Color>) {
		let Some(fill_node_id) = self.existing_proto_node_id(graphene_std::vector_nodes::fill::IDENTIFIER, true) else {
			return;
		};
		let input_connector = InputConnector::node(fill_node_id, graphene_std::vector::fill::FillInput);
		let backup_input_connector = InputConnector::node(fill_node_id, graphene_std::vector::fill::BackupColorInput);

		// The backup remembers the last solid color, so the red-slash "none" choice leaves it untouched
		if let Some(color) = color {
			self.set_input_with_refresh(backup_input_connector, NodeInput::value(TaggedValue::Color(color), false), true);
		}
		let fill_value = color.map_or_else(TaggedValue::no_paint, TaggedValue::Color);
		self.set_input_with_refresh(input_connector, NodeInput::value(fill_value, false), false);
	}

	#[allow(clippy::too_many_arguments)]
	pub fn fill_gradient_set(
		&mut self,
		gradient: Gradient,
		gradient_form: GradientForm,
		gradient_spread: GradientSpread,
		gradient_space: GradientSpace,
		gradient_cyclic: bool,
		gradient_hue_direction: GradientHueDirection,
		gradient_interpolation: GradientInterpolation,
		transform: DAffine2,
	) {
		let Some(fill_node_id) = self.existing_proto_node_id(graphene_std::vector_nodes::fill::IDENTIFIER, true) else {
			return;
		};
		let backup_input_connector = InputConnector::node(fill_node_id, graphene_std::vector::fill::BackupGradientInput);

		let ramp = GradientRamp::from(gradient);
		let ramp = GradientRamp {
			gradient_spread,
			gradient_space,
			gradient_cyclic,
			gradient_hue_direction,
			gradient_interpolation,
			..ramp
		};
		self.set_input_with_refresh(backup_input_connector, NodeInput::value(TaggedValue::GradientRamp(ramp.clone()), false), true);

		// Skip the rerender on all but the last input so the whole update triggers a single graph run
		self.set_input_with_refresh(
			InputConnector::node(fill_node_id, graphene_std::vector::fill::FillInput),
			NodeInput::value(TaggedValue::GradientRamp(ramp), false),
			true,
		);

		// Reposition the gradient only when the transform is a plain value, leaving a wired transform source connected
		let transform_is_value = self
			.network_interface
			.document_network()
			.nodes
			.get(&fill_node_id)
			.and_then(|node| node.input(graphene_std::vector::fill::TransformInput))
			.is_some_and(|input| input.as_value().is_some());
		if transform_is_value {
			self.set_input_with_refresh(
				InputConnector::node(fill_node_id, graphene_std::vector::fill::HasTransformInput),
				NodeInput::value(TaggedValue::Bool(true), false),
				true,
			);
			self.set_input_with_refresh(
				InputConnector::node(fill_node_id, graphene_std::vector::fill::TransformInput),
				NodeInput::value(TaggedValue::DAffine2(transform), false),
				true,
			);
		}

		self.set_input_with_refresh(
			InputConnector::node(fill_node_id, graphene_std::vector::fill::GradientFormInput),
			NodeInput::value(TaggedValue::GradientForm(gradient_form), false),
			false,
		);
	}

	pub fn blend_mode_set(&mut self, blend_mode: BlendMode) {
		let Some(blend_node_id) = self.existing_proto_node_id(graphene_std::blending_nodes::blend_mode::IDENTIFIER, true) else {
			return;
		};
		let input_connector = InputConnector::node(blend_node_id, graphene_std::blending_nodes::blend_mode::BlendModeInput);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::BlendMode(blend_mode), false), false);
	}

	pub fn opacity_set(&mut self, opacity: f64) {
		let Some(opacity_node_id) = self.existing_proto_node_id(graphene_std::blending_nodes::opacity::IDENTIFIER, true) else {
			return;
		};
		// Enable the `has_opacity` checkbox so the value is applied
		self.set_input_with_refresh(
			InputConnector::node(opacity_node_id, graphene_std::blending_nodes::opacity::HasOpacityInput),
			NodeInput::value(TaggedValue::Bool(true), false),
			false,
		);
		self.set_input_with_refresh(
			InputConnector::node(opacity_node_id, graphene_std::blending_nodes::opacity::OpacityInput),
			NodeInput::value(TaggedValue::F64(opacity * 100.), false),
			false,
		);
	}

	pub fn opacity_fill_set(&mut self, fill: f64) {
		// Reuse an existing Opacity node to avoid a redundant chain walk on slider drags
		let identifier = graphene_std::blending_nodes::opacity::IDENTIFIER;
		let existing = self.existing_proto_node_id(identifier.clone(), false);
		let existed = existing.is_some();
		let Some(opacity_node_id) = existing.or_else(|| self.existing_proto_node_id(identifier, true)) else {
			return;
		};
		// Freshly-created node defaults to opacity enabled; disable it so the fill slider works independently
		if !existed {
			self.set_input_with_refresh(
				InputConnector::node(opacity_node_id, graphene_std::blending_nodes::opacity::HasOpacityInput),
				NodeInput::value(TaggedValue::Bool(false), false),
				false,
			);
		}
		// Enable the `has_fill` checkbox so the value is applied
		self.set_input_with_refresh(
			InputConnector::node(opacity_node_id, graphene_std::blending_nodes::opacity::HasFillInput),
			NodeInput::value(TaggedValue::Bool(true), false),
			false,
		);
		self.set_input_with_refresh(
			InputConnector::node(opacity_node_id, graphene_std::blending_nodes::opacity::FillInput),
			NodeInput::value(TaggedValue::F64(fill * 100.), false),
			false,
		);
	}

	/// Update the chain's 'Color Value' node, or start a chain with one on an empty layer, painting the layer's whole expanse.
	pub fn color_value_set(&mut self, color: Color) {
		let Some(output_layer) = self.get_output_layer() else { return };

		let target_input = gradient_chain_target_input(output_layer, self.network_interface);
		if let Some(node_id) = self.existing_proto_node_id_at(&target_input, graphene_std::math_nodes::color_value::IDENTIFIER, false) {
			let input_connector = InputConnector::node(node_id, graphene_std::math_nodes::color_value::ColorInput);
			self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::Color(color), false), false);
			return;
		}

		// The 'Color Value' node discards its primary input, so only a blank 'Merge' layer may start a chain with one,
		// which any whole-expanse paint the other tool left behind is cleared off to become
		let Some(paint_chain) = replaceable_paint_chain(output_layer, self.network_interface) else {
			return;
		};
		self.clear_paint_chain(&paint_chain);

		let color_value_id = self.insert_color_value(color, output_layer, paint_chain.attachment_input);
		let input_connector = InputConnector::node(color_value_id, graphene_std::math_nodes::color_value::ColorInput);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::Color(color), false), false);
	}

	/// Write the gradient stops to the 'Gradient Value' node feeding the layer.
	pub fn gradient_stops_set(&mut self, stops: Gradient) {
		let Some(output_layer) = self.get_output_layer() else { return };

		let gradient_value_id = match get_upstream_gradient_value_node_id(output_layer, self.network_interface) {
			Some(id) => id,
			None => {
				let target = gradient_chain_target_input(output_layer, self.network_interface);
				let starts_layer_chain = target == InputConnector::layer_secondary_input(output_layer.to_node());

				// The 'Gradient Value' node discards its primary input, so only a blank 'Merge' layer may start a chain
				// with one, which any whole-expanse paint the other tool left behind is cleared off to become
				let paint_chain = if starts_layer_chain {
					let Some(paint_chain) = replaceable_paint_chain(output_layer, self.network_interface) else {
						log::error!("Refusing to start a gradient chain on anything but a blank 'Merge' layer");
						return;
					};
					self.clear_paint_chain(&paint_chain);

					Some(paint_chain)
				} else {
					None
				};

				let Some(node_definition) = resolve_proto_node_type(graphene_std::math_nodes::gradient_value::IDENTIFIER) else {
					return;
				};
				let node_id = NodeId::new();
				self.network_interface.insert_node(node_id, node_definition.default_node_template(), &[]);

				if let Some(paint_chain) = paint_chain {
					// No Fill node: the new node starts the layer's chain
					self.start_paint_chain(&node_id, output_layer, paint_chain.attachment_input);
				} else {
					// Feeding a Fill node's paint input: wire it up and place it one chain-width left and a step below the Fill
					self.network_interface.set_input(&target, NodeInput::node(node_id, 0), &[]);
					if let Some(target_node_id) = target.node_id()
						&& let Some(target_position) = self.network_interface.position(&target_node_id, &[])
					{
						let node_position = self.network_interface.position(&node_id, &[]).unwrap_or_default();
						let desired_position = target_position + IVec2::new(-crate::consts::NODE_CHAIN_WIDTH, 2);
						self.network_interface.shift_absolute_node_position(&node_id, desired_position - node_position, &[]);
					}
				}

				node_id
			}
		};

		// Only the stops are being replaced, so the ramp's other settings stay as the value node already holds them
		let ramp = GradientRamp {
			stops: (&stops).into(),
			..self.gradient_value_ramp(gradient_value_id).unwrap_or_default()
		};

		let input_connector = InputConnector::node(gradient_value_id, graphene_std::math_nodes::gradient_value::GradientInput);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::GradientRamp(ramp), false), false);
	}

	/// Update the last 'Gradient Positions' node in the chain when one exists, so on-canvas stop drags stay live even
	/// though that node would otherwise override the stops value's own placement. Never inserts one: the stops value
	/// carries placement itself, and these setter nodes are user-authored procedural overrides. A wired input is
	/// procedural authorship too, so it is likewise left untouched.
	pub fn gradient_positions_set(&mut self, positions: Vec<f64>) {
		let Some(output_layer) = self.get_output_layer() else { return };

		let target_input = gradient_chain_target_input(output_layer, self.network_interface);
		let identifier = graphene_std::math_nodes::gradient_positions::IDENTIFIER;
		let Some(node_id) = self.existing_proto_node_id_at(&target_input, identifier, false) else {
			return;
		};

		let current_input = self
			.network_interface
			.document_network()
			.nodes
			.get(&node_id)
			.and_then(|node| node.input(graphene_std::math_nodes::gradient_positions::PositionsInput));
		if !current_input.is_some_and(|input| input.as_value().is_some()) {
			return;
		}

		let input_connector = InputConnector::node(node_id, graphene_std::math_nodes::gradient_positions::PositionsInput);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::F64Array(positions), false), false);
	}

	/// The 'Gradient Midpoints' counterpart of [`Self::gradient_positions_set`], likewise update-only.
	pub fn gradient_midpoints_set(&mut self, midpoints: Vec<f64>) {
		let Some(output_layer) = self.get_output_layer() else { return };

		let target_input = gradient_chain_target_input(output_layer, self.network_interface);
		let identifier = graphene_std::math_nodes::gradient_midpoints::IDENTIFIER;
		let Some(node_id) = self.existing_proto_node_id_at(&target_input, identifier, false) else {
			return;
		};

		let current_input = self
			.network_interface
			.document_network()
			.nodes
			.get(&node_id)
			.and_then(|node| node.input(graphene_std::math_nodes::gradient_midpoints::MidpointsInput));
		if !current_input.is_some_and(|input| input.as_value().is_some()) {
			return;
		}

		let input_connector = InputConnector::node(node_id, graphene_std::math_nodes::gradient_midpoints::MidpointsInput);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::F64Array(midpoints), false), false);
	}

	/// Update the transform to map the unit gradient ((0,0), (1, 0)) to the geometry's local space.
	/// With multiple `Transform` nodes the last one (closest to the layer) is modified so the chain still composes to the target.
	/// With none, one is inserted unless the target is the identity.
	pub fn gradient_transform_set(&mut self, transform: DAffine2) {
		let Some(output_layer) = self.get_output_layer() else { return };

		let walk_from = if let Some(fill_input_node_id) = get_fill_input_node_id(output_layer, self.network_interface) {
			// Some nodes are connected to a Fill node, this means that the primary path is a `List<Vector>`, so we need to traverse it
			fill_input_node_id
		} else {
			// No Fill node found, we will traverse the primary path to find transforms
			output_layer.to_node()
		};

		let transform_reference = DefinitionIdentifier::ProtoNode(graphene_std::transform_nodes::transform::IDENTIFIER);
		let upstream_transforms: Vec<NodeId> = self
			.network_interface
			.upstream_flow_back_from_nodes(vec![walk_from], &[], FlowType::HorizontalFlow)
			.skip_while(|node_id| self.network_interface.is_layer(node_id, &[]))
			.take_while(|node_id| !self.network_interface.is_layer(node_id, &[]))
			.filter(|id| self.network_interface.reference(id, &[]).as_ref() == Some(&transform_reference))
			.collect();

		// Upstream walk yields downstream-to-upstream order, so the first hit is the chain's last `Transform`
		let (last_transform_node_id, prior_transforms) = match upstream_transforms.split_first() {
			Some((last, prior)) => (Some(*last), prior),
			None => (None, [].as_slice()),
		};

		// `composed_old` = T_n * T_{n-1} * ... * T_1, `prior_combined` = same product without T_n
		let compose = |ids: &[_]| {
			ids.iter().fold(DAffine2::IDENTITY, |acc, transform_id| {
				self.network_interface
					.document_network()
					.nodes
					.get(transform_id)
					.map_or(acc, |document_node| acc * transform_utils::get_current_transform(&document_node.inputs))
			})
		};
		let prior_combined = compose(prior_transforms);

		let last_transform_value = transform * prior_combined.inverse();

		let target_input = gradient_chain_target_input(output_layer, self.network_interface);
		let transform_node_id = if let Some(id) = last_transform_node_id {
			id
		} else {
			// Don't pollute the graph with an identity 'Transform' node
			if last_transform_value.abs_diff_eq(DAffine2::IDENTITY, 1e-6) {
				return;
			}
			let Some(id) = self.existing_proto_node_id_at(&target_input, graphene_std::transform_nodes::transform::IDENTIFIER, true) else {
				return;
			};
			id
		};

		transform_utils::update_transform(self.network_interface, &transform_node_id, last_transform_value);
		self.responses.add(PropertiesPanelMessage::Refresh);
		self.responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	/// Write the Gradient Form to the last 'Gradient Form' node in the chain, inserting one only when the value differs
	/// from the default (`Linear`).
	pub fn gradient_form_set(&mut self, gradient_form: GradientForm) {
		let Some(output_layer) = self.get_output_layer() else { return };

		let target_input = gradient_chain_target_input(output_layer, self.network_interface);
		let identifier = graphene_std::math_nodes::gradient_form::IDENTIFIER;
		let create_if_nonexistent = gradient_form != GradientForm::default();
		let Some(node_id) = self.existing_proto_node_id_at(&target_input, identifier, create_if_nonexistent) else {
			return;
		};

		let input_connector = InputConnector::node(node_id, graphene_std::math_nodes::gradient_form::GradientFormInput);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::GradientForm(gradient_form), false), false);
	}

	/// Set the spread on the chain's gradient value, which is where the ramp carries it.
	pub fn gradient_spread_set(&mut self, gradient_spread: GradientSpread) {
		let Some(output_layer) = self.get_output_layer() else { return };
		let Some(gradient_value_id) = get_upstream_gradient_value_node_id(output_layer, self.network_interface) else {
			return;
		};
		let Some(ramp) = self.gradient_value_ramp(gradient_value_id) else { return };

		let ramp = GradientRamp { gradient_spread, ..ramp };
		let input_connector = InputConnector::node(gradient_value_id, graphene_std::math_nodes::gradient_value::GradientInput);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::GradientRamp(ramp), false), false);
	}

	/// The ramp currently held by a 'Gradient Value' node.
	fn gradient_value_ramp(&self, gradient_value_id: NodeId) -> Option<GradientRamp> {
		let node = self.network_interface.document_network().nodes.get(&gradient_value_id)?;
		let TaggedValue::GradientRamp(ramp) = node.input(graphene_std::math_nodes::gradient_value::GradientInput)?.as_value()? else {
			return None;
		};
		Some(ramp.clone())
	}

	/// Set the space on the chain's gradient value, which is where the ramp carries it. Never touches a
	/// 'Gradient Space' node: that one is a user-authored procedural override, not something the tools manage.
	pub fn gradient_space_set(&mut self, gradient_space: GradientSpace) {
		let Some(output_layer) = self.get_output_layer() else { return };
		let Some(gradient_value_id) = get_upstream_gradient_value_node_id(output_layer, self.network_interface) else {
			return;
		};
		let Some(ramp) = self.gradient_value_ramp(gradient_value_id) else { return };

		let ramp = GradientRamp { gradient_space, ..ramp };
		let input_connector = InputConnector::node(gradient_value_id, graphene_std::math_nodes::gradient_value::GradientInput);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::GradientRamp(ramp), false), false);
	}

	/// Set the cyclic wrap flag on the chain's gradient value, which is where the ramp carries it.
	pub fn gradient_cyclic_set(&mut self, gradient_cyclic: bool) {
		let Some(output_layer) = self.get_output_layer() else { return };
		let Some(gradient_value_id) = get_upstream_gradient_value_node_id(output_layer, self.network_interface) else {
			return;
		};
		let Some(ramp) = self.gradient_value_ramp(gradient_value_id) else { return };

		let ramp = ramp.with_cyclic(gradient_cyclic);
		let input_connector = InputConnector::node(gradient_value_id, graphene_std::math_nodes::gradient_value::GradientInput);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::GradientRamp(ramp), false), false);
	}

	/// Set the hue direction on the chain's gradient value, which is where the ramp carries it.
	pub fn gradient_interpolation_set(&mut self, gradient_interpolation: GradientInterpolation) {
		let Some(output_layer) = self.get_output_layer() else { return };
		let Some(gradient_value_id) = get_upstream_gradient_value_node_id(output_layer, self.network_interface) else {
			return;
		};
		let Some(ramp) = self.gradient_value_ramp(gradient_value_id) else { return };

		let ramp = GradientRamp { gradient_interpolation, ..ramp };
		let input_connector = InputConnector::node(gradient_value_id, graphene_std::math_nodes::gradient_value::GradientInput);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::GradientRamp(ramp), false), false);
	}

	pub fn gradient_hue_direction_set(&mut self, gradient_hue_direction: GradientHueDirection) {
		let Some(output_layer) = self.get_output_layer() else { return };
		let Some(gradient_value_id) = get_upstream_gradient_value_node_id(output_layer, self.network_interface) else {
			return;
		};
		let Some(ramp) = self.gradient_value_ramp(gradient_value_id) else { return };

		let ramp = GradientRamp { gradient_hue_direction, ..ramp };
		let input_connector = InputConnector::node(gradient_value_id, graphene_std::math_nodes::gradient_value::GradientInput);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::GradientRamp(ramp), false), false);
	}

	pub fn clip_mode_toggle(&mut self, clip_mode: Option<bool>) {
		let clip = !clip_mode.unwrap_or(false);
		let Some(clip_node_id) = self.existing_proto_node_id(graphene_std::blending_nodes::clipping_mask::IDENTIFIER, true) else {
			return;
		};
		let input_connector = InputConnector::node(clip_node_id, graphene_std::blending_nodes::clipping_mask::ClipInput);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::Bool(clip), false), false);
	}

	pub fn stroke_set(&mut self, color: Option<Color>, stroke: Stroke) {
		let Some(stroke_node_id) = self.existing_proto_node_id(graphene_std::vector::stroke::IDENTIFIER, true) else {
			return;
		};

		let input_connector = InputConnector::node(stroke_node_id, graphene_std::vector::stroke::PaintInput);
		self.set_input_with_refresh(input_connector, NodeInput::value(color.map_or_else(TaggedValue::no_paint, TaggedValue::Color), false), true);
		let input_connector = InputConnector::node(stroke_node_id, graphene_std::vector::stroke::WeightInput);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::F64(stroke.weight), false), true);
		let input_connector = InputConnector::node(stroke_node_id, graphene_std::vector::stroke::AlignInput);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::StrokeAlign(stroke.align), false), false);
		let input_connector = InputConnector::node(stroke_node_id, graphene_std::vector::stroke::CapInput);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::StrokeCap(stroke.cap), false), true);
		let input_connector = InputConnector::node(stroke_node_id, graphene_std::vector::stroke::JoinInput);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::StrokeJoin(stroke.join), false), true);
		let input_connector = InputConnector::node(stroke_node_id, graphene_std::vector::stroke::MiterLimitInput);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::F64(stroke.join_miter_limit), false), false);
		let input_connector = InputConnector::node(stroke_node_id, graphene_std::vector::stroke::PaintOrderInput);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::PaintOrder(stroke.paint_order), false), false);
		let input_connector = InputConnector::node(stroke_node_id, graphene_std::vector::stroke::DashPatternInput);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::DashPattern(stroke.dash_lengths), false), true);
		let input_connector = InputConnector::node(stroke_node_id, graphene_std::vector::stroke::DashOffsetInput);
		self.set_input_with_refresh(input_connector, NodeInput::value(TaggedValue::F64(stroke.dash_offset), false), true);
	}

	/// Update the transform value of the upstream Transform node based a change to its existing value and the given parent transform.
	/// A new Transform node is created if one does not exist, unless it would be given the identity transform.
	pub fn transform_change_with_parent(&mut self, transform: DAffine2, transform_in: TransformIn, parent_transform: DAffine2, skip_rerender: bool) {
		// Get the existing upstream Transform node and its transform, if present, otherwise use the identity transform
		let (layer_transform, transform_node_id) = self
			.existing_proto_node_id(graphene_std::transform_nodes::transform::IDENTIFIER, false)
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
		let transform_node_id = self.existing_proto_node_id(graphene_std::transform_nodes::transform::IDENTIFIER, false);

		// Compute the Transform node value so `transform_to_viewport` matches the target after re-render
		let final_transform = match transform_in {
			TransformIn::Local => transform,
			TransformIn::Scope { scope } => scope * transform,
			TransformIn::Viewport => {
				let Some(layer) = self.layer_node else { return };
				let metadata = self.network_interface.document_metadata();
				let parent_inverse = metadata.downstream_transform_to_viewport(layer).inverse();

				// Compensate for item 0's baseline offset (multi-item Text only) so the layer doesn't jump by it.
				// Gated on `text_frames` because metadata can be stale mid-handler.
				if metadata.text_frames.contains_key(&layer) {
					let local_transform = metadata.local_transforms.get(&layer.to_node()).copied().unwrap_or(DAffine2::IDENTITY);
					let current_transform_node_value = transform_node_id
						.and_then(|id| self.network_interface.document_network().nodes.get(&id))
						.map(|node| transform_utils::get_current_transform(&node.inputs))
						.unwrap_or(DAffine2::IDENTITY);
					parent_inverse * transform * local_transform.inverse() * current_transform_node_value
				} else {
					parent_inverse * transform
				}
			}
		};

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
			self.existing_proto_node_id(graphene_std::transform_nodes::transform::IDENTIFIER, true)
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
		let Some(path_node_id) = self.existing_network_node_id("Path", true) else {
			return;
		};
		self.network_interface.vector_modify(&path_node_id, modification_type);
		self.responses.add(PropertiesPanelMessage::Refresh);
		self.responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	pub fn brush_modify(&mut self, strokes: Vec<BrushStroke>) {
		let Some(brush_node_id) = self.existing_proto_node_id(graphene_std::brush::brush::brush::IDENTIFIER, true) else {
			return;
		};
		self.set_input_with_refresh(
			InputConnector::node(brush_node_id, graphene_std::brush::brush::brush::TraceInput),
			NodeInput::value(TaggedValue::BrushStrokes(strokes), false),
			false,
		);
	}

	pub fn resize_artboard(&mut self, location: DVec2, dimensions: DVec2) {
		let Some(artboard_node_id) = self.existing_network_node_id("Artboard", true) else {
			return;
		};

		let mut dimensions = dimensions;
		let mut location = location;

		if dimensions.x < 0. {
			dimensions.x = -dimensions.x;
			location.x -= dimensions.x;
		}
		if dimensions.y < 0. {
			dimensions.y = -dimensions.y;
			location.y -= dimensions.y;
		}
		self.set_input_with_refresh(
			InputConnector::node_at_index(artboard_node_id, ARTBOARD_LOCATION_INPUT_INDEX),
			NodeInput::value(TaggedValue::DVec2(location), false),
			false,
		);
		self.set_input_with_refresh(
			InputConnector::node_at_index(artboard_node_id, ARTBOARD_DIMENSIONS_INPUT_INDEX),
			NodeInput::value(TaggedValue::DVec2(dimensions), false),
			false,
		);
	}

	/// Set the input, refresh the Properties panel, and run the document graph if skip_rerender is false
	pub fn set_input_with_refresh(&mut self, input_connector: InputConnector, input: NodeInput, skip_rerender: bool) {
		self.network_interface.set_input(&input_connector, input, &[]);
		self.responses.add(PropertiesPanelMessage::Refresh);
		if !skip_rerender {
			self.responses.add(NodeGraphMessage::RunDocumentGraph);
		}
	}
}

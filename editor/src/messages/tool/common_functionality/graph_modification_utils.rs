use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::{self, DefinitionIdentifier};
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{FlowType, InputConnector, NodeNetworkInterface, NodeTemplate};
use crate::messages::prelude::*;
use glam::DVec2;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeId, NodeInput};
use graph_craft::{ProtoNodeIdentifier, concrete};
use graphene_std::Color;
use graphene_std::NodeInputDecleration;
use graphene_std::list::List;
use graphene_std::raster::BlendMode;
use graphene_std::raster_types::{CPU, GPU, Image, Raster};
use graphene_std::subpath::Subpath;
use graphene_std::text::{Font, TypesettingConfig};
use graphene_std::vector::misc::ManipulatorPointId;
use graphene_std::vector::style::{Fill, FillChoice, Gradient, PaintOrder, StrokeAlign, StrokeCap, StrokeJoin};
use graphene_std::vector::{GradientStops, PointId, SegmentId, VectorModificationType};
use std::collections::VecDeque;

/// Returns the ID of the first Spline node in the horizontal flow which is not followed by a `Path` node, or `None` if none exists.
pub fn find_spline(document: &DocumentMessageHandler, layer: LayerNodeIdentifier) -> Option<NodeId> {
	document
		.network_interface
		.upstream_flow_back_from_nodes([layer.to_node()].to_vec(), &[], FlowType::HorizontalFlow)
		.map(|node_id| (document.network_interface.reference(&node_id, &[]), node_id))
		.take_while(|(reference, _)| reference.as_ref().is_some_and(|node_ref| node_ref != &DefinitionIdentifier::Network("Path".into())))
		.find(|(reference, _)| {
			reference
				.as_ref()
				.is_some_and(|node_ref| *node_ref == DefinitionIdentifier::ProtoNode(graphene_std::vector::spline::IDENTIFIER))
		})
		.map(|node| node.1)
}

/// Merge `second_layer` to the `first_layer`.
pub fn merge_layers(document: &DocumentMessageHandler, first_layer: LayerNodeIdentifier, second_layer: LayerNodeIdentifier, responses: &mut VecDeque<Message>) {
	// Skip layers that are children of each other (or the same)
	if first_layer.ancestors(document.metadata()).any(|l| l == second_layer) || second_layer.ancestors(document.metadata()).any(|l| l == first_layer) {
		return;
	}
	// Calculate the downstream transforms in order to bring the other vector geometry into the same layer space
	let first_layer_transform = document.metadata().downstream_transform_to_document(first_layer);
	let second_layer_transform = document.metadata().downstream_transform_to_document(second_layer);

	// Represents the change in position that would occur if the other layer was moved below the current layer
	let transform_delta = first_layer_transform * second_layer_transform.inverse();
	let offset = transform_delta.inverse();
	responses.add(GraphOperationMessage::TransformChange {
		layer: second_layer,
		transform: offset,
		transform_in: TransformIn::Local,
		skip_rerender: false,
	});

	let mut current_and_other_layer_is_spline = false;

	if let (Some(current_layer_spline), Some(other_layer_spline)) = (find_spline(document, first_layer), find_spline(document, second_layer)) {
		responses.add(NodeGraphMessage::DeleteNodes {
			node_ids: [current_layer_spline, other_layer_spline].to_vec(),
			delete_children: false,
		});
		current_and_other_layer_is_spline = true;
	}

	// Move the `second_layer` below the `first_layer` for positioning purposes
	let Some(first_layer_parent) = first_layer.parent(document.metadata()) else { return };
	let Some(first_layer_index) = first_layer_parent.children(document.metadata()).position(|child| child == first_layer) else {
		return;
	};
	responses.add(NodeGraphMessage::MoveLayerToStack {
		layer: second_layer,
		parent: first_layer_parent,
		insert_index: first_layer_index + 1,
	});

	// Merge the inputs of the two layers
	let merge_node_id = NodeId::new();
	let merge_node = document_node_definitions::resolve_network_node_type("Merge")
		.expect("Failed to create merge node")
		.default_node_template();
	responses.add(NodeGraphMessage::InsertNode {
		node_id: merge_node_id,
		node_template: Box::new(merge_node),
	});
	responses.add(NodeGraphMessage::SetToNodeOrLayer {
		node_id: merge_node_id,
		is_layer: false,
	});
	responses.add(NodeGraphMessage::MoveNodeToChainStart {
		node_id: merge_node_id,
		parent: first_layer,
	});
	responses.add(NodeGraphMessage::ConnectUpstreamOutputToInput {
		downstream_input: InputConnector::node(second_layer.to_node(), 1),
		input_connector: InputConnector::node(merge_node_id, 1),
	});
	responses.add(NodeGraphMessage::DeleteNodes {
		node_ids: vec![second_layer.to_node()],
		delete_children: false,
	});

	// Add a Flatten Path node after the merge
	let flatten_node_id = NodeId::new();
	let flatten_node = document_node_definitions::resolve_proto_node_type(graphene_std::vector::flatten_path::IDENTIFIER)
		.expect("Failed to create flatten node")
		.default_node_template();
	responses.add(NodeGraphMessage::InsertNode {
		node_id: flatten_node_id,
		node_template: Box::new(flatten_node),
	});
	responses.add(NodeGraphMessage::MoveNodeToChainStart {
		node_id: flatten_node_id,
		parent: first_layer,
	});

	// Add a path node after the flatten node
	let path_node_id = NodeId::new();
	let path_node = document_node_definitions::resolve_network_node_type("Path")
		.expect("Failed to create path node")
		.default_node_template();
	responses.add(NodeGraphMessage::InsertNode {
		node_id: path_node_id,
		node_template: Box::new(path_node),
	});
	responses.add(NodeGraphMessage::MoveNodeToChainStart {
		node_id: path_node_id,
		parent: first_layer,
	});

	// Add a Spline node after the Path node if both the layers we are merging is spline.
	if current_and_other_layer_is_spline {
		let spline_node_id = NodeId::new();
		let spline_node = document_node_definitions::resolve_proto_node_type(graphene_std::vector::spline::IDENTIFIER)
			.expect("Failed to create Spline node")
			.default_node_template();
		responses.add(NodeGraphMessage::InsertNode {
			node_id: spline_node_id,
			node_template: Box::new(spline_node),
		});
		responses.add(NodeGraphMessage::MoveNodeToChainStart {
			node_id: spline_node_id,
			parent: first_layer,
		});
	}

	// Add a transform node to ensure correct tooling modifications
	let transform_node_id = NodeId::new();
	let transform_node = document_node_definitions::resolve_proto_node_type(graphene_std::transform_nodes::transform::IDENTIFIER)
		.expect("Failed to create transform node")
		.default_node_template();
	responses.add(NodeGraphMessage::InsertNode {
		node_id: transform_node_id,
		node_template: Box::new(transform_node),
	});
	responses.add(NodeGraphMessage::MoveNodeToChainStart {
		node_id: transform_node_id,
		parent: first_layer,
	});

	responses.add(NodeGraphMessage::RunDocumentGraph);
	responses.add(DeferMessage::AfterGraphRun {
		messages: vec![PenToolMessage::RecalculateLatestPointsPosition.into()],
	});
}

/// Merge the `first_endpoint` with `second_endpoint`.
pub fn merge_points(document: &DocumentMessageHandler, layer: LayerNodeIdentifier, first_endpoint: PointId, second_endpont: PointId, responses: &mut VecDeque<Message>) {
	let transform = document.metadata().transform_to_document(layer);
	let Some(vector) = document.network_interface.compute_modified_vector(layer) else { return };

	let segment = vector.segment_bezier_iter().find(|(_, _, start, end)| *end == second_endpont || *start == second_endpont);
	let Some((segment, _, mut segment_start_point, mut segment_end_point)) = segment else {
		log::error!("Could not get the segment for second_endpoint.");
		return;
	};

	let mut handles = [None; 2];
	if let Some(handle_position) = ManipulatorPointId::PrimaryHandle(segment).get_position(&vector) {
		let anchor_position = ManipulatorPointId::Anchor(segment_start_point).get_position(&vector).unwrap();
		let handle_position = transform.transform_point2(handle_position);
		let anchor_position = transform.transform_point2(anchor_position);
		let anchor_to_handle = handle_position - anchor_position;
		handles[0] = Some(anchor_to_handle);
	}
	if let Some(handle_position) = ManipulatorPointId::EndHandle(segment).get_position(&vector) {
		let anchor_position = ManipulatorPointId::Anchor(segment_end_point).get_position(&vector).unwrap();
		let handle_position = transform.transform_point2(handle_position);
		let anchor_position = transform.transform_point2(anchor_position);
		let anchor_to_handle = handle_position - anchor_position;
		handles[1] = Some(anchor_to_handle);
	}

	if segment_start_point == second_endpont {
		core::mem::swap(&mut segment_start_point, &mut segment_end_point);
		handles.reverse();
	}

	let modification_type = VectorModificationType::RemovePoint { id: second_endpont };
	responses.add(GraphOperationMessage::Vector { layer, modification_type });
	let modification_type = VectorModificationType::RemoveSegment { id: segment };
	responses.add(GraphOperationMessage::Vector { layer, modification_type });

	let points = [segment_start_point, first_endpoint];
	let id = SegmentId::generate();
	let modification_type = VectorModificationType::InsertSegment { id, points, handles };
	responses.add(GraphOperationMessage::Vector { layer, modification_type });
}

/// Create a new vector layer.
pub fn new_vector_layer(subpaths: Vec<Subpath<PointId>>, id: NodeId, parent: LayerNodeIdentifier, responses: &mut VecDeque<Message>) -> LayerNodeIdentifier {
	let insert_index = 0;
	responses.add(GraphOperationMessage::NewVectorLayer { id, subpaths, parent, insert_index });
	responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![id] });

	LayerNodeIdentifier::new_unchecked(id)
}

/// Create a new bitmap layer.
pub fn new_image_layer(image: Image<Color>, id: NodeId, parent: LayerNodeIdentifier, responses: &mut VecDeque<Message>) -> LayerNodeIdentifier {
	let insert_index = 0;
	responses.add(GraphOperationMessage::NewBitmapLayer { id, image, parent, insert_index });
	LayerNodeIdentifier::new_unchecked(id)
}

/// Create a new group layer from an SVG string.
pub fn new_svg_layer(svg: String, transform: glam::DAffine2, center: bool, id: NodeId, parent: LayerNodeIdentifier, responses: &mut VecDeque<Message>) -> LayerNodeIdentifier {
	let insert_index = 0;
	responses.add(GraphOperationMessage::NewSvg {
		id,
		svg,
		transform,
		parent,
		insert_index,
		center,
	});
	LayerNodeIdentifier::new_unchecked(id)
}

pub fn new_custom(id: NodeId, nodes: Vec<(NodeId, NodeTemplate)>, parent: LayerNodeIdentifier, responses: &mut VecDeque<Message>) -> LayerNodeIdentifier {
	responses.add(GraphOperationMessage::NewCustomLayer { id, nodes, parent, insert_index: 0 });
	responses.add(GraphOperationMessage::SetUpstreamToChain {
		layer: LayerNodeIdentifier::new_unchecked(id),
	});
	responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![id] });
	LayerNodeIdentifier::new_unchecked(id)
}

/// Locate the origin of the "Transform" node.
pub fn get_origin(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<DVec2> {
	use graphene_std::transform_nodes::transform::*;

	if let TaggedValue::DVec2(origin) =
		NodeGraphLayer::new(layer, network_interface).find_input(&DefinitionIdentifier::ProtoNode(graphene_std::transform_nodes::transform::IDENTIFIER), TranslationInput::INDEX)?
	{
		Some(*origin)
	} else {
		None
	}
}

pub fn get_viewport_origin(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> DVec2 {
	let origin = get_origin(layer, network_interface).unwrap_or_default();
	network_interface.document_metadata().downstream_transform_to_viewport(layer).transform_point2(origin)
}

pub fn get_viewport_center(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> DVec2 {
	let [min, max] = network_interface.document_metadata().nonzero_bounding_box(layer);
	let center = DVec2::splat(0.5);
	network_interface.document_metadata().transform_to_viewport(layer).transform_point2(min + (max - min) * center)
}

/// Determine the input connector where the gradient chain enters the layer.
/// Returns Fill's fill input if the layer has a "Fill" node, otherwise returns the layer's content input.
pub fn gradient_chain_target_input(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> InputConnector {
	if let Some(fill_node_id) = NodeGraphLayer::new(layer, network_interface).upstream_node_id_from_name(&DefinitionIdentifier::ProtoNode(graphene_std::vector::fill::IDENTIFIER)) {
		InputConnector::node(fill_node_id, graphene_std::vector::fill::FillInput::<Fill>::INDEX)
	} else {
		InputConnector::node(layer.to_node(), 1)
	}
}

/// Try to find a "Gradient Value" node that is connected to a "Fill" node, or to a layer directly.
pub fn get_upstream_gradient_value_node_id(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<NodeId> {
	let target_input = gradient_chain_target_input(layer, network_interface);
	let walk_from = network_interface.upstream_output_connector(&target_input, &[])?.node_id()?;

	network_interface
		.upstream_flow_back_from_nodes(vec![walk_from], &[], FlowType::HorizontalFlow)
		.take_while(|node_id| !network_interface.is_layer(node_id, &[]))
		.find(|node_id| network_interface.reference(node_id, &[]).as_ref() == Some(&DefinitionIdentifier::ProtoNode(graphene_std::math_nodes::gradient_value::IDENTIFIER)))
}

/// Get the node connected to Fill's fill input, if any.
pub fn get_fill_input_node_id(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<NodeId> {
	let fill_node_id = NodeGraphLayer::new(layer, network_interface).upstream_node_id_from_name(&DefinitionIdentifier::ProtoNode(graphene_std::vector::fill::IDENTIFIER))?;
	let fill_node = network_interface.document_network().nodes.get(&fill_node_id)?;
	let NodeInput::Node { node_id, .. } = fill_node.inputs.get(graphene_std::vector::fill::FillInput::<Fill>::INDEX)? else {
		return None;
	};
	Some(*node_id)
}

/// Get the current gradient of a layer from the closest "Fill" node.
pub fn get_gradient(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<Gradient> {
	let fill_index = 1;

	let inputs = NodeGraphLayer::new(layer, network_interface).find_node_inputs(&DefinitionIdentifier::ProtoNode(graphene_std::vector::fill::IDENTIFIER))?;
	let TaggedValue::Fill(Fill::Gradient(gradient)) = inputs.get(fill_index)?.as_value()? else {
		return None;
	};
	Some(gradient.clone())
}

/// Get the gradient stops of a layer, if any.
pub fn get_gradient_stops(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<GradientStops> {
	let gradient_value_node = network_interface.document_network().nodes.get(&get_upstream_gradient_value_node_id(layer, network_interface)?)?;
	let TaggedValue::Gradient(stops) = gradient_value_node.inputs.get(graphene_std::math_nodes::gradient_value::GradientInput::INDEX)?.as_value()? else {
		return None;
	};
	Some(stops.clone())
}

/// Compute the transform from a gradient's local space to viewport space for the given layer. For a `List<GradientStops>`
/// layer this is the layer's incoming footprint transform; for the legacy `Fill::Gradient` path it composes the layer's
/// viewport transform with the [0,1]² → bounding-box mapping.
pub fn gradient_space_transform(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> glam::DAffine2 {
	use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;

	let metadata = network_interface.document_metadata();
	let is_gradient_list = is_layer_fed_by_node_of_name(layer, network_interface, &DefinitionIdentifier::ProtoNode(graphene_std::math_nodes::gradient_value::IDENTIFIER));
	if is_gradient_list {
		return metadata
			.upstream_footprints
			.get(&layer.to_node())
			.map(|footprint| footprint.transform)
			.unwrap_or(metadata.document_to_viewport);
	}
	let multiplied = metadata.transform_to_viewport(layer);
	let bounds = metadata.nonzero_bounding_box(layer);
	let bound_transform = glam::DAffine2::from_scale_angle_translation(bounds[1] - bounds[0], 0., bounds[0]);
	multiplied * bound_transform
}

/// True when start→end (mapped through `transform` into viewport space) points predominantly rightward. For purely
/// vertical lines we fall back to a stable tiebreaker on (x + y) so the choice doesn't flicker between equal alternatives.
pub fn gradient_orientation_rightward(start: glam::DVec2, end: glam::DVec2, transform: glam::DAffine2) -> bool {
	let viewport_start = transform.transform_point2(start);
	let viewport_end = transform.transform_point2(end);
	if (viewport_end.x - viewport_start.x).abs() > f64::EPSILON * 1e6 {
		viewport_end.x > viewport_start.x
	} else {
		(viewport_start.x + viewport_start.y) < (viewport_end.x + viewport_end.y)
	}
}

/// Get the current fill of a layer from the closest "Fill" node.
pub fn get_fill_color(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<Color> {
	let fill_index = 1;

	let inputs = NodeGraphLayer::new(layer, network_interface).find_node_inputs(&DefinitionIdentifier::ProtoNode(graphene_std::vector::fill::IDENTIFIER))?;
	let &TaggedValue::Fill(Fill::Solid(color)) = inputs.get(fill_index)?.as_value()? else {
		return None;
	};
	Some(color)
}

/// Get the current blend mode of a layer from the closest upstream "Blend Mode" node.
pub fn get_blend_mode(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<BlendMode> {
	let inputs = NodeGraphLayer::new(layer, network_interface).find_node_inputs(&DefinitionIdentifier::ProtoNode(graphene_std::blending_nodes::blend_mode::IDENTIFIER))?;
	let TaggedValue::BlendMode(blend_mode) = inputs.get(1)?.as_value()? else {
		return None;
	};
	Some(*blend_mode)
}

/// Get the current opacity of a layer from the closest upstream "Opacity" node, only when the node's `has_opacity` checkbox is enabled.
/// This may differ from the actual opacity contained within the data type reaching this layer, because that actual opacity may be:
/// - Multiplied with additional Opacity nodes earlier in the chain
/// - Set by an Opacity node with an exposed input value driven by another node
/// - Already factored into the pixel alpha channel of an image
/// - The default value of 100% if no Opacity node has its checkbox enabled (this function returns `None` in that case)
///
/// With those limitations in mind, the intention of this function is to show just the value already present in an upstream Opacity node so that value can be directly edited.
pub fn get_opacity(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<f64> {
	let inputs = NodeGraphLayer::new(layer, network_interface).find_node_inputs(&DefinitionIdentifier::ProtoNode(graphene_std::blending_nodes::opacity::IDENTIFIER))?;
	let TaggedValue::Bool(true) = inputs.get(1)?.as_value()? else {
		return None;
	};
	let TaggedValue::F64(opacity) = inputs.get(2)?.as_value()? else {
		return None;
	};
	Some(*opacity)
}

pub fn get_clip_mode(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<bool> {
	let inputs = NodeGraphLayer::new(layer, network_interface).find_node_inputs(&DefinitionIdentifier::ProtoNode(graphene_std::blending_nodes::clipping_mask::IDENTIFIER))?;
	let TaggedValue::Bool(clip) = inputs.get(1)?.as_value()? else {
		return None;
	};
	Some(*clip)
}

/// Get the current fill of a layer from the closest upstream "Opacity" node, only when the node's `has_fill` checkbox is enabled.
pub fn get_fill(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<f64> {
	let inputs = NodeGraphLayer::new(layer, network_interface).find_node_inputs(&DefinitionIdentifier::ProtoNode(graphene_std::blending_nodes::opacity::IDENTIFIER))?;
	let TaggedValue::Bool(true) = inputs.get(3)?.as_value()? else {
		return None;
	};
	let TaggedValue::F64(fill) = inputs.get(4)?.as_value()? else {
		return None;
	};
	Some(*fill)
}

pub fn get_fill_id(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<NodeId> {
	NodeGraphLayer::new(layer, network_interface).upstream_node_id_from_name(&DefinitionIdentifier::ProtoNode(graphene_std::vector_nodes::fill::IDENTIFIER))
}

pub fn get_circle_id(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<NodeId> {
	NodeGraphLayer::new(layer, network_interface).upstream_node_id_from_name(&DefinitionIdentifier::ProtoNode(graphene_std::vector_nodes::circle::IDENTIFIER))
}

pub fn get_ellipse_id(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<NodeId> {
	NodeGraphLayer::new(layer, network_interface).upstream_node_id_from_name(&DefinitionIdentifier::ProtoNode(graphene_std::vector_nodes::ellipse::IDENTIFIER))
}

pub fn get_line_id(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<NodeId> {
	NodeGraphLayer::new(layer, network_interface).upstream_node_id_from_name(&DefinitionIdentifier::ProtoNode(graphene_std::vector_nodes::line::IDENTIFIER))
}

pub fn get_polygon_id(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<NodeId> {
	NodeGraphLayer::new(layer, network_interface).upstream_node_id_from_name(&DefinitionIdentifier::ProtoNode(graphene_std::vector_nodes::regular_polygon::IDENTIFIER))
}

pub fn get_rectangle_id(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<NodeId> {
	NodeGraphLayer::new(layer, network_interface).upstream_node_id_from_name(&DefinitionIdentifier::ProtoNode(graphene_std::vector_nodes::rectangle::IDENTIFIER))
}

pub fn get_star_id(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<NodeId> {
	NodeGraphLayer::new(layer, network_interface).upstream_node_id_from_name(&DefinitionIdentifier::ProtoNode(graphene_std::vector_nodes::star::IDENTIFIER))
}

pub fn get_arc_id(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<NodeId> {
	NodeGraphLayer::new(layer, network_interface).upstream_node_id_from_name(&DefinitionIdentifier::ProtoNode(graphene_std::vector_nodes::arc::IDENTIFIER))
}

pub fn get_arrow_id(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<NodeId> {
	NodeGraphLayer::new(layer, network_interface).upstream_node_id_from_name(&DefinitionIdentifier::ProtoNode(graphene_std::vector_nodes::arrow::IDENTIFIER))
}

pub fn get_spiral_id(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<NodeId> {
	NodeGraphLayer::new(layer, network_interface).upstream_node_id_from_name(&DefinitionIdentifier::ProtoNode(graphene_std::vector_nodes::spiral::IDENTIFIER))
}

pub fn get_text_id(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<NodeId> {
	NodeGraphLayer::new(layer, network_interface).upstream_node_id_from_name(&DefinitionIdentifier::ProtoNode(graphene_std::text::text::IDENTIFIER))
}

pub fn get_grid_id(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<NodeId> {
	NodeGraphLayer::new(layer, network_interface).upstream_node_id_from_name(&DefinitionIdentifier::ProtoNode(graphene_std::vector::generator_nodes::grid::IDENTIFIER))
}

/// Gets properties from the Text node. Resolves the font selection by reading the resource id and lookup via the fonts message handler.
pub fn get_text<'a>(
	layer: LayerNodeIdentifier,
	network_interface: &'a NodeNetworkInterface,
	fonts: &FontsMessageHandler,
	resources: &ResourceMessageHandler,
) -> Option<(&'a String, Font, TypesettingConfig)> {
	let inputs = NodeGraphLayer::new(layer, network_interface).find_node_inputs(&DefinitionIdentifier::ProtoNode(graphene_std::text::text::IDENTIFIER))?;

	let Some(TaggedValue::String(text)) = inputs.get(graphene_std::text::text::TextInput::INDEX)?.as_value() else {
		return None;
	};
	let font = match inputs.get(graphene_std::text::text::FontInput::INDEX)?.as_value() {
		Some(TaggedValue::Resource(resource_id)) => fonts.id_font(resources, *resource_id).unwrap_or_default(),
		_ => Font::default(),
	};
	let Some(&TaggedValue::F64(font_size)) = inputs.get(graphene_std::text::text::SizeInput::INDEX)?.as_value() else {
		return None;
	};
	let Some(&TaggedValue::F64(line_height_ratio)) = inputs.get(graphene_std::text::text::LineHeightInput::INDEX)?.as_value() else {
		return None;
	};
	let Some(&TaggedValue::F64(character_spacing)) = inputs.get(graphene_std::text::text::CharacterSpacingInput::INDEX)?.as_value() else {
		return None;
	};
	let Some(&TaggedValue::Bool(has_max_width)) = inputs.get(graphene_std::text::text::HasMaxWidthInput::INDEX)?.as_value() else {
		return None;
	};
	let Some(&TaggedValue::F64(max_width)) = inputs.get(graphene_std::text::text::MaxWidthInput::INDEX)?.as_value() else {
		return None;
	};
	let Some(&TaggedValue::Bool(has_max_height)) = inputs.get(graphene_std::text::text::HasMaxHeightInput::INDEX)?.as_value() else {
		return None;
	};
	let Some(&TaggedValue::F64(max_height)) = inputs.get(graphene_std::text::text::MaxHeightInput::INDEX)?.as_value() else {
		return None;
	};
	let Some(&TaggedValue::F64(tilt)) = inputs.get(graphene_std::text::text::TiltInput::INDEX)?.as_value() else {
		return None;
	};
	let Some(&TaggedValue::TextAlign(align)) = inputs.get(graphene_std::text::text::AlignInput::INDEX)?.as_value() else {
		return None;
	};

	let typesetting = TypesettingConfig {
		font_size,
		line_height_ratio,
		max_width: has_max_width.then_some(max_width),
		max_height: has_max_height.then_some(max_height),
		character_spacing,
		tilt,
		align,
	};
	Some((text, font, typesetting))
}

pub fn get_stroke_width(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<f64> {
	let weight_node_input_index = graphene_std::vector::stroke::WeightInput::INDEX;
	if let TaggedValue::F64(width) = NodeGraphLayer::new(layer, network_interface).find_input(&DefinitionIdentifier::ProtoNode(graphene_std::vector::stroke::IDENTIFIER), weight_node_input_index)? {
		Some(*width)
	} else {
		None
	}
}

/// Subset of Stroke node inputs read for the control bar's stroke options popover.
#[derive(Debug, Clone, PartialEq)]
pub struct StrokeOptionsState {
	pub align: StrokeAlign,
	pub cap: StrokeCap,
	pub join: StrokeJoin,
	pub miter_limit: f64,
	pub paint_order: PaintOrder,
	pub dash_lengths: Vec<f64>,
	pub dash_offset: f64,
}

/// Reads the non-color stroke option inputs from a layer's Stroke proto node. Returns `None` when the layer has no Stroke node.
/// Inputs that aren't a static value (e.g. wired to another node) fall back to per-field defaults so the layer still participates in the sync.
pub fn get_stroke_options(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<StrokeOptionsState> {
	let stroke = &DefinitionIdentifier::ProtoNode(graphene_std::vector::stroke::IDENTIFIER);
	let layer_view = NodeGraphLayer::new(layer, network_interface);
	layer_view.upstream_node_id_from_name(stroke)?;
	let read = |index: usize| layer_view.find_input(stroke, index);

	let align = match read(graphene_std::vector::stroke::AlignInput::INDEX) {
		Some(TaggedValue::StrokeAlign(value)) => *value,
		_ => StrokeAlign::default(),
	};
	let cap = match read(graphene_std::vector::stroke::CapInput::INDEX) {
		Some(TaggedValue::StrokeCap(value)) => *value,
		_ => StrokeCap::default(),
	};
	let join = match read(graphene_std::vector::stroke::JoinInput::INDEX) {
		Some(TaggedValue::StrokeJoin(value)) => *value,
		_ => StrokeJoin::default(),
	};
	let miter_limit = match read(graphene_std::vector::stroke::MiterLimitInput::INDEX) {
		Some(TaggedValue::F64(value)) => *value,
		_ => 4.,
	};
	let paint_order = match read(graphene_std::vector::stroke::PaintOrderInput::INDEX) {
		Some(TaggedValue::PaintOrder(value)) => *value,
		_ => PaintOrder::default(),
	};
	let dash_lengths = match read(graphene_std::vector::stroke::DashLengthsInput::<List<f64>>::INDEX) {
		Some(TaggedValue::F64Array(value)) => value.clone(),
		_ => Vec::new(),
	};
	let dash_offset = match read(graphene_std::vector::stroke::DashOffsetInput::INDEX) {
		Some(TaggedValue::F64(value)) => *value,
		_ => 0.,
	};

	Some(StrokeOptionsState {
		align,
		cap,
		join,
		miter_limit,
		paint_order,
		dash_lengths,
		dash_offset,
	})
}

/// Returns the node ID of a layer's upstream Stroke proto node, if one exists.
pub fn get_stroke_id(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<NodeId> {
	NodeGraphLayer::new(layer, network_interface).upstream_node_id_from_name(&DefinitionIdentifier::ProtoNode(graphene_std::vector::stroke::IDENTIFIER))
}

/// Stroke weight of the first selected non-artboard layer, used by tool control bars to mirror the selection's weight.
/// Returns `Some(0.)` if the layer has no Stroke node so the widget reads "0 px", and `None` only when no layer is selected.
pub fn first_selected_stroke_weight(document: &DocumentMessageHandler) -> Option<f64> {
	document
		.network_interface
		.selected_nodes()
		.selected_layers_except_artboards(&document.network_interface)
		.next()
		.map(|layer| get_stroke_width(layer, &document.network_interface).unwrap_or(0.))
}

/// Writes the weight back to every selected non-artboard layer's stroke. Layers with an existing stroke just have their
/// `WeightInput` updated; layers without one get a fresh stroke node added (defaulting to a black stroke with the new
/// weight) only when the new weight is nonzero, so changing back to 0 doesn't keep adding empty strokes.
pub fn set_stroke_weight_for_selected_layers(weight: f64, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	let layers: Vec<_> = document.network_interface.selected_nodes().selected_layers_except_artboards(&document.network_interface).collect();
	for layer in layers {
		if let Some(node_id) = get_stroke_id(layer, &document.network_interface) {
			let input_index = graphene_std::vector::stroke::WeightInput::INDEX;
			let value = TaggedValue::F64(weight);
			responses.add(NodeGraphMessage::SetInputValue { node_id, input_index, value });
		} else if weight > 0. {
			let stroke = graphene_std::vector::style::Stroke::default().with_weight(weight);
			responses.add(GraphOperationMessage::StrokeSet { layer, stroke });
		}
	}
}

/// Returns the `Fill` value from a layer's upstream Fill node.
pub fn get_fill_value(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<Fill> {
	let fill_index = graphene_std::vector::fill::FillInput::<Fill>::INDEX;
	let tagged = NodeGraphLayer::new(layer, network_interface).find_input(&DefinitionIdentifier::ProtoNode(graphene_std::vector::fill::IDENTIFIER), fill_index)?;
	if let TaggedValue::Fill(fill) = tagged { Some(fill.clone()) } else { None }
}

/// Returns the stroke color from a layer's upstream Stroke node.
pub fn get_stroke_color(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<Option<Color>> {
	let color_index = graphene_std::vector::stroke::ColorInput::INDEX;
	let tagged = NodeGraphLayer::new(layer, network_interface).find_input(&DefinitionIdentifier::ProtoNode(graphene_std::vector::stroke::IDENTIFIER), color_index)?;
	if let TaggedValue::Color(color) = tagged { Some(*color) } else { None }
}

/// Aggregated fill state across all selected non-artboard layers.
pub struct SelectedFillState {
	/// `None` means mixed values between selected layers.
	pub enabled: Option<bool>,
	/// `None` means mixed values between selected layers.
	pub fill_choice: Option<FillChoice>,
}

/// Aggregated stroke state across all selected non-artboard layers.
pub struct SelectedStrokeState {
	/// `None` means mixed values between selected layers.
	pub enabled: Option<bool>,
	/// `None` means mixed values between selected layers.
	pub optional_color: Option<Option<Color>>,
}

/// Reads the fill state across all selected non-artboard layers, including whether their enabled states or colors differ.
/// "Enabled" tracks node attachment: a layer counts as enabled whenever a Fill node is attached, even when that fill's value is [`FillChoice::None`].
/// Unticked means there is no Fill node. Returns `None` only when no layer is selected.
pub fn selected_fill_state(document: &DocumentMessageHandler) -> Option<SelectedFillState> {
	let selected_nodes = document.network_interface.selected_nodes();
	let mut per_layer = selected_nodes.selected_layers_except_artboards(&document.network_interface).map(|layer| {
		if get_fill_id(layer, &document.network_interface).is_none() {
			return (false, FillChoice::None);
		}
		let fill_choice = get_fill_value(layer, &document.network_interface).map_or(FillChoice::None, FillChoice::from);
		(true, fill_choice)
	});

	let (initial_enabled, initial_choice) = per_layer.next()?;
	let mut enabled_mixed = false;
	let mut color_mixed = false;
	let mut comparison_enabled = initial_enabled;
	let mut comparison_choice = initial_choice;
	for (enabled, fill_choice) in per_layer {
		if enabled != initial_enabled {
			enabled_mixed = true;
		}
		if enabled {
			if comparison_enabled {
				if fill_choice != comparison_choice {
					color_mixed = true;
				}
			} else {
				comparison_enabled = true;
				comparison_choice = fill_choice;
			}
		}
	}

	Some(SelectedFillState {
		enabled: (!enabled_mixed).then_some(initial_enabled),
		fill_choice: (!color_mixed).then_some(comparison_choice),
	})
}

/// Reads the stroke state across all selected non-artboard layers, including whether their enabled states or colors differ.
/// "Enabled" tracks node attachment: a layer counts as enabled whenever a Stroke node is attached, even when that stroke's color is `None`.
/// Unticked means there is no Stroke node. Returns `None` only when no layer is selected.
pub fn selected_stroke_state(document: &DocumentMessageHandler) -> Option<SelectedStrokeState> {
	let selected_nodes = document.network_interface.selected_nodes();
	let mut per_layer = selected_nodes.selected_layers_except_artboards(&document.network_interface).map(|layer| {
		if get_stroke_id(layer, &document.network_interface).is_none() {
			return (false, None);
		}
		let color = get_stroke_color(layer, &document.network_interface).flatten();
		(true, color)
	});

	let (initial_enabled, initial_color) = per_layer.next()?;
	let mut enabled_mixed = false;
	let mut color_mixed = false;
	let mut comparison_enabled = initial_enabled;
	let mut comparison_color = initial_color;
	for (enabled, color) in per_layer {
		if enabled != initial_enabled {
			enabled_mixed = true;
		}
		if enabled {
			if comparison_enabled {
				if color != comparison_color {
					color_mixed = true;
				}
			} else {
				comparison_enabled = true;
				comparison_color = color;
			}
		}
	}

	Some(SelectedStrokeState {
		enabled: (!enabled_mixed).then_some(initial_enabled),
		optional_color: (!color_mixed).then_some(comparison_color),
	})
}

/// Sets the fill on all selected non-artboard layers, preserving gradient transform data when the layer already has a gradient fill.
pub fn set_fill_for_selected_layers(fill_choice: FillChoice, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	let layers: Vec<_> = document.network_interface.selected_nodes().selected_layers_except_artboards(&document.network_interface).collect();
	for layer in layers {
		let existing_gradient = get_fill_value(layer, &document.network_interface).and_then(|f| match f {
			Fill::Gradient(g) => Some(g),
			_ => None,
		});
		let fill = fill_choice.clone().to_fill(existing_gradient.as_ref());
		responses.add(GraphOperationMessage::FillSet { layer, fill });
	}
}

/// Sets the stroke color on all selected non-artboard layers. Layers without an existing Stroke node get one created using
/// the provided `weight`, so picking any color (including `None`) from an unticked stroke control bar entry both attaches
/// the Stroke node and applies the chosen color.
pub fn set_stroke_color_for_selected_layers(color: Option<Color>, weight: f64, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	let layers: Vec<_> = document.network_interface.selected_nodes().selected_layers_except_artboards(&document.network_interface).collect();
	for layer in layers {
		if let Some(node_id) = get_stroke_id(layer, &document.network_interface) {
			let input_index = graphene_std::vector::stroke::ColorInput::INDEX;
			let value = TaggedValue::Color(color);
			responses.add(NodeGraphMessage::SetInputValue { node_id, input_index, value });
		} else {
			let stroke = graphene_std::vector::style::Stroke::new(color, weight);
			responses.add(GraphOperationMessage::StrokeSet { layer, stroke });
		}
	}
}

/// Removes the Fill node from all selected non-artboard layers.
pub fn remove_fill_for_selected_layers(document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	let layers: Vec<_> = document.network_interface.selected_nodes().selected_layers_except_artboards(&document.network_interface).collect();
	for layer in layers {
		if let Some(node_id) = get_fill_id(layer, &document.network_interface) {
			responses.add(NodeGraphMessage::DeleteNodes {
				node_ids: vec![node_id],
				delete_children: true,
			});
		}
	}
	responses.add(NodeGraphMessage::RunDocumentGraph);
	responses.add(NodeGraphMessage::SendGraph);
}

/// Removes the Stroke node from all selected non-artboard layers.
pub fn remove_stroke_for_selected_layers(document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	let layers: Vec<_> = document.network_interface.selected_nodes().selected_layers_except_artboards(&document.network_interface).collect();
	for layer in layers {
		if let Some(node_id) = get_stroke_id(layer, &document.network_interface) {
			responses.add(NodeGraphMessage::DeleteNodes {
				node_ids: vec![node_id],
				delete_children: true,
			});
		}
	}
	responses.add(NodeGraphMessage::RunDocumentGraph);
	responses.add(NodeGraphMessage::SendGraph);
}

/// Reads a specific input from the matching proto node on the first selected non-artboard layer that has one.
/// Used by tool control bars to mirror per-shape parameters (sides, arc type, turns, etc.) from the selection
/// into the control bar's input widget state without each call site re-implementing the layer iteration.
pub fn first_selected_proto_node_input(document: &DocumentMessageHandler, identifier: graph_craft::ProtoNodeIdentifier, input_index: usize) -> Option<&TaggedValue> {
	let identifier = DefinitionIdentifier::ProtoNode(identifier);
	document
		.network_interface
		.selected_nodes()
		.selected_layers_except_artboards(&document.network_interface)
		.find_map(|layer| NodeGraphLayer::new(layer, &document.network_interface).find_input(&identifier, input_index))
}

/// Writes a value to a specific input on the matching proto node of every selected non-artboard layer that has one.
/// Used by tool control bars to push per-shape parameter changes back onto all selected layers of that shape.
pub fn set_proto_node_input_for_selected_layers(
	document: &DocumentMessageHandler,
	identifier: graph_craft::ProtoNodeIdentifier,
	input_index: usize,
	value: TaggedValue,
	responses: &mut VecDeque<Message>,
) {
	let identifier = DefinitionIdentifier::ProtoNode(identifier);

	let layers: Vec<_> = document.network_interface.selected_nodes().selected_layers_except_artboards(&document.network_interface).collect();

	for layer in layers {
		let Some(node_id) = NodeGraphLayer::new(layer, &document.network_interface).upstream_node_id_from_name(&identifier) else {
			continue;
		};
		responses.add(NodeGraphMessage::SetInputValue {
			node_id,
			input_index,
			value: value.clone(),
		});
	}
}

/// Checks if a specified layer uses an upstream node matching the given name.
pub fn is_layer_fed_by_node_of_name(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface, identifier: &DefinitionIdentifier) -> bool {
	NodeGraphLayer::new(layer, network_interface).find_node_inputs(identifier).is_some()
}

/// An immutable reference to a layer within the document node graph for easy access.
pub struct NodeGraphLayer<'a> {
	network_interface: &'a NodeNetworkInterface,
	layer_node: NodeId,
}

impl<'a> NodeGraphLayer<'a> {
	/// Get the layer node from the document
	pub fn new(layer: LayerNodeIdentifier, network_interface: &'a NodeNetworkInterface) -> Self {
		debug_assert!(layer != LayerNodeIdentifier::ROOT_PARENT, "Cannot create new NodeGraphLayer from ROOT_PARENT");
		Self {
			network_interface,
			layer_node: layer.to_node(),
		}
	}

	/// Return an iterator up the horizontal flow of the layer
	pub fn horizontal_layer_flow(&self) -> impl Iterator<Item = NodeId> + use<'a> {
		self.network_interface.upstream_flow_back_from_nodes(vec![self.layer_node], &[], FlowType::HorizontalFlow)
	}

	/// Node id of a node if it exists in this specific layer's primary flow, stopping at the next layer upstream so a group doesn't incorrectly match its children's nodes.
	pub fn upstream_node_id_from_name(&self, identifier: &DefinitionIdentifier) -> Option<NodeId> {
		self.horizontal_layer_flow()
			.take_while(|&node_id| node_id == self.layer_node || !self.network_interface.is_layer(&node_id, &[]))
			.find(|node_id| self.network_interface.reference(node_id, &[]).is_some_and(|reference| reference == *identifier))
	}

	/// Node id of a visible node if it exists in the layer's primary flow until another layer
	pub fn upstream_visible_node_id_from_name_in_layer(&self, identifier: &DefinitionIdentifier) -> Option<NodeId> {
		// `.skip(1)` is used to skip self
		self.horizontal_layer_flow()
			.skip(1)
			.take_while(|node_id| !self.network_interface.is_layer(node_id, &[]))
			.filter(|node_id| self.network_interface.is_visible(node_id, &[]))
			.find(|node_id| self.network_interface.reference(node_id, &[]).is_some_and(|reference| reference == *identifier))
	}

	/// Node id of a protonode if it exists in the layer's primary flow
	pub fn upstream_node_id_from_protonode(&self, protonode_identifier: ProtoNodeIdentifier) -> Option<NodeId> {
		self.horizontal_layer_flow()
			// Take until a different layer is reached
			.take_while(|&node_id| node_id == self.layer_node || !self.network_interface.is_layer(&node_id, &[]))
			.find(|node_id| {
				self.network_interface
					.implementation(node_id, &[])
					.is_some_and(|implementation| *implementation == graph_craft::document::DocumentNodeImplementation::ProtoNode(protonode_identifier.clone()))
			})
	}

	/// Find all of the inputs of a specific node within the layer's primary flow, up until the next layer is reached.
	pub fn find_node_inputs(&self, identifier: &DefinitionIdentifier) -> Option<&'a Vec<NodeInput>> {
		// `.skip(1)` is used to skip self
		self.horizontal_layer_flow()
			.skip(1)
			.take_while(|node_id| !self.network_interface.is_layer(node_id, &[]))
			.find(|node_id| self.network_interface.reference(node_id, &[]).is_some_and(|reference| reference == *identifier))
			.and_then(|node_id| self.network_interface.document_network().nodes.get(&node_id).map(|node| &node.inputs))
	}

	/// Find a specific input of a node within the layer's primary flow
	pub fn find_input(&self, identifier: &DefinitionIdentifier, index: usize) -> Option<&'a TaggedValue> {
		// TODO: Find a better way to accept a node input rather than using its index (which is quite unclear and fragile)
		self.find_node_inputs(identifier)?.get(index)?.as_value()
	}

	/// Check if a layer is a raster layer
	pub fn is_raster_layer(layer: LayerNodeIdentifier, network_interface: &mut NodeNetworkInterface) -> bool {
		let layer_input_type = network_interface.input_type(&InputConnector::node(layer.to_node(), 1), &[]);

		layer_input_type.compiled_nested_type() == Some(&concrete!(List<Raster<CPU>>)) || layer_input_type.compiled_nested_type() == Some(&concrete!(List<Raster<GPU>>))
	}
}

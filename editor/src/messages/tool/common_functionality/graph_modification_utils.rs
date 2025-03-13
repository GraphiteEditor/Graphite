use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{FlowType, InputConnector, NodeNetworkInterface, NodeTemplate};
use crate::messages::prelude::*;
use bezier_rs::Subpath;
use glam::DVec2;
use graph_craft::concrete;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeId, NodeInput};
use graphene_core::Color;
use graphene_core::raster::BlendMode;
use graphene_core::raster::image::ImageFrameTable;
use graphene_core::text::{Font, TypesettingConfig};
use graphene_core::vector::style::Gradient;
use graphene_std::vector::{ManipulatorPointId, PointId, SegmentId, VectorModificationType};
use std::collections::VecDeque;

/// Returns the ID of the first Spline node in the horizontal flow which is not followed by a `Path` node, or `None` if none exists.
pub fn find_spline(document: &DocumentMessageHandler, layer: LayerNodeIdentifier) -> Option<NodeId> {
	document
		.network_interface
		.upstream_flow_back_from_nodes([layer.to_node()].to_vec(), &[], FlowType::HorizontalFlow)
		.map(|node_id| (document.network_interface.reference(&node_id, &[]).unwrap(), node_id))
		.take_while(|(reference, _)| reference.as_ref().is_some_and(|node_ref| node_ref != "Path"))
		.find(|(reference, _)| reference.as_ref().is_some_and(|node_ref| node_ref == "Spline"))
		.map(|node| node.1)
}

/// Merge `second_layer` to the `first_layer`.
pub fn merge_layers(document: &DocumentMessageHandler, first_layer: LayerNodeIdentifier, second_layer: LayerNodeIdentifier, responses: &mut VecDeque<Message>) {
	if first_layer == second_layer {
		return;
	}
	// Calculate the downstream transforms in order to bring the other vector data into the same layer space
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
	let first_layer_parent = first_layer.parent(document.metadata()).unwrap();
	let first_layer_index = first_layer_parent.children(document.metadata()).position(|child| child == first_layer).unwrap();
	responses.add(NodeGraphMessage::MoveLayerToStack {
		layer: second_layer,
		parent: first_layer_parent,
		insert_index: first_layer_index + 1,
	});

	// Merge the inputs of the two layers
	let merge_node_id = NodeId::new();
	let merge_node = document_node_definitions::resolve_document_node_type("Merge")
		.expect("Failed to create merge node")
		.default_node_template();
	responses.add(NodeGraphMessage::InsertNode {
		node_id: merge_node_id,
		node_template: merge_node,
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

	// Add a flatten vector elements node after the merge
	let flatten_node_id = NodeId::new();
	let flatten_node = document_node_definitions::resolve_document_node_type("Flatten Vector Elements")
		.expect("Failed to create flatten node")
		.default_node_template();
	responses.add(NodeGraphMessage::InsertNode {
		node_id: flatten_node_id,
		node_template: flatten_node,
	});
	responses.add(NodeGraphMessage::MoveNodeToChainStart {
		node_id: flatten_node_id,
		parent: first_layer,
	});

	// Add a path node after the flatten node
	let path_node_id = NodeId::new();
	let path_node = document_node_definitions::resolve_document_node_type("Path")
		.expect("Failed to create path node")
		.default_node_template();
	responses.add(NodeGraphMessage::InsertNode {
		node_id: path_node_id,
		node_template: path_node,
	});
	responses.add(NodeGraphMessage::MoveNodeToChainStart {
		node_id: path_node_id,
		parent: first_layer,
	});

	// Add a Spline node after the Path node if both the layers we are merging is spline.
	if current_and_other_layer_is_spline {
		let spline_node_id = NodeId::new();
		let spline_node = document_node_definitions::resolve_document_node_type("Spline")
			.expect("Failed to create Spline node")
			.default_node_template();
		responses.add(NodeGraphMessage::InsertNode {
			node_id: spline_node_id,
			node_template: spline_node,
		});
		responses.add(NodeGraphMessage::MoveNodeToChainStart {
			node_id: spline_node_id,
			parent: first_layer,
		});
	}

	// Add a transform node to ensure correct tooling modifications
	let transform_node_id = NodeId::new();
	let transform_node = document_node_definitions::resolve_document_node_type("Transform")
		.expect("Failed to create transform node")
		.default_node_template();
	responses.add(NodeGraphMessage::InsertNode {
		node_id: transform_node_id,
		node_template: transform_node,
	});
	responses.add(NodeGraphMessage::MoveNodeToChainStart {
		node_id: transform_node_id,
		parent: first_layer,
	});

	responses.add(NodeGraphMessage::RunDocumentGraph);
	responses.add(Message::StartBuffer);
	responses.add(PenToolMessage::RecalculateLatestPointsPosition);
}

/// Merge the `first_endpoint` with `second_endpoint`.
pub fn merge_points(document: &DocumentMessageHandler, layer: LayerNodeIdentifier, first_endpoint: PointId, second_endpont: PointId, responses: &mut VecDeque<Message>) {
	let transform = document.metadata().transform_to_document(layer);
	let Some(vector_data) = document.network_interface.compute_modified_vector(layer) else { return };

	let segment = vector_data.segment_bezier_iter().find(|(_, _, start, end)| *end == second_endpont || *start == second_endpont);
	let Some((segment, _, mut segment_start_point, mut segment_end_point)) = segment else {
		log::error!("Could not get the segment for second_endpoint.");
		return;
	};

	let mut handles = [None; 2];
	if let Some(handle_position) = ManipulatorPointId::PrimaryHandle(segment).get_position(&vector_data) {
		let anchor_position = ManipulatorPointId::Anchor(segment_start_point).get_position(&vector_data).unwrap();
		let handle_position = transform.transform_point2(handle_position);
		let anchor_position = transform.transform_point2(anchor_position);
		let anchor_to_handle = handle_position - anchor_position;
		handles[0] = Some(anchor_to_handle);
	}
	if let Some(handle_position) = ManipulatorPointId::EndHandle(segment).get_position(&vector_data) {
		let anchor_position = ManipulatorPointId::Anchor(segment_end_point).get_position(&vector_data).unwrap();
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
pub fn new_image_layer(image_frame: ImageFrameTable<Color>, id: NodeId, parent: LayerNodeIdentifier, responses: &mut VecDeque<Message>) -> LayerNodeIdentifier {
	let insert_index = 0;
	responses.add(GraphOperationMessage::NewBitmapLayer {
		id,
		image_frame,
		parent,
		insert_index,
	});
	LayerNodeIdentifier::new_unchecked(id)
}

/// Create a new group layer from an SVG string.
pub fn new_svg_layer(svg: String, transform: glam::DAffine2, id: NodeId, parent: LayerNodeIdentifier, responses: &mut VecDeque<Message>) -> LayerNodeIdentifier {
	let insert_index = 0;
	responses.add(GraphOperationMessage::NewSvg {
		id,
		svg,
		transform,
		parent,
		insert_index,
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

/// Locate the final pivot from the transform (TODO: decide how the pivot should actually work)
pub fn get_pivot(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<DVec2> {
	let pivot_node_input_index = 5;
	if let TaggedValue::DVec2(pivot) = NodeGraphLayer::new(layer, network_interface).find_input("Transform", pivot_node_input_index)? {
		Some(*pivot)
	} else {
		None
	}
}

pub fn get_viewport_pivot(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> DVec2 {
	let [min, max] = network_interface.document_metadata().nonzero_bounding_box(layer);
	let pivot = get_pivot(layer, network_interface).unwrap_or(DVec2::splat(0.5));
	network_interface.document_metadata().transform_to_viewport(layer).transform_point2(min + (max - min) * pivot)
}

/// Get the current gradient of a layer from the closest Fill node
pub fn get_gradient(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<Gradient> {
	let fill_index = 1;

	let inputs = NodeGraphLayer::new(layer, network_interface).find_node_inputs("Fill")?;
	let TaggedValue::Fill(graphene_std::vector::style::Fill::Gradient(gradient)) = inputs.get(fill_index)?.as_value()? else {
		return None;
	};
	Some(gradient.clone())
}

/// Get the current fill of a layer from the closest Fill node
pub fn get_fill_color(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<Color> {
	let fill_index = 1;

	let inputs = NodeGraphLayer::new(layer, network_interface).find_node_inputs("Fill")?;
	let TaggedValue::Fill(graphene_std::vector::style::Fill::Solid(color)) = inputs.get(fill_index)?.as_value()? else {
		return None;
	};
	Some(*color)
}

/// Get the current blend mode of a layer from the closest Blend Mode node
pub fn get_blend_mode(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<BlendMode> {
	let inputs = NodeGraphLayer::new(layer, network_interface).find_node_inputs("Blend Mode")?;
	let TaggedValue::BlendMode(blend_mode) = inputs.get(1)?.as_value()? else {
		return None;
	};
	Some(*blend_mode)
}

/// Get the current opacity of a layer from the closest Opacity node.
/// This may differ from the actual opacity contained within the data type reaching this layer, because that actual opacity may be:
/// - Multiplied with additional opacity nodes earlier in the chain
/// - Set by an Opacity node with an exposed input value driven by another node
/// - Already factored into the pixel alpha channel of an image
/// - The default value of 100% if no Opacity node is present, but this function returns None in that case
///
/// With those limitations in mind, the intention of this function is to show just the value already present in an upstream Opacity node so that value can be directly edited.
pub fn get_opacity(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<f64> {
	let inputs = NodeGraphLayer::new(layer, network_interface).find_node_inputs("Opacity")?;
	let TaggedValue::F64(opacity) = inputs.get(1)?.as_value()? else {
		return None;
	};
	Some(*opacity)
}

pub fn get_fill_id(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<NodeId> {
	NodeGraphLayer::new(layer, network_interface).upstream_node_id_from_name("Fill")
}

pub fn get_ellipse_id(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<NodeId> {
	NodeGraphLayer::new(layer, network_interface).upstream_node_id_from_name("Ellipse")
}

pub fn get_line_id(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<NodeId> {
	NodeGraphLayer::new(layer, network_interface).upstream_node_id_from_name("Line")
}

pub fn get_polygon_id(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<NodeId> {
	NodeGraphLayer::new(layer, network_interface).upstream_node_id_from_name("Regular Polygon")
}

pub fn get_rectangle_id(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<NodeId> {
	NodeGraphLayer::new(layer, network_interface).upstream_node_id_from_name("Rectangle")
}

pub fn get_star_id(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<NodeId> {
	NodeGraphLayer::new(layer, network_interface).upstream_node_id_from_name("Star")
}

pub fn get_text_id(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<NodeId> {
	NodeGraphLayer::new(layer, network_interface).upstream_node_id_from_name("Text")
}

/// Gets properties from the Text node
pub fn get_text(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<(&String, &Font, TypesettingConfig)> {
	let inputs = NodeGraphLayer::new(layer, network_interface).find_node_inputs("Text")?;

	let Some(TaggedValue::String(text)) = &inputs[1].as_value() else { return None };
	let Some(TaggedValue::Font(font)) = &inputs[2].as_value() else { return None };
	let Some(&TaggedValue::F64(font_size)) = inputs[3].as_value() else { return None };
	let Some(&TaggedValue::F64(line_height_ratio)) = inputs[4].as_value() else { return None };
	let Some(&TaggedValue::F64(character_spacing)) = inputs[5].as_value() else { return None };
	let Some(&TaggedValue::OptionalF64(max_width)) = inputs[6].as_value() else { return None };
	let Some(&TaggedValue::OptionalF64(max_height)) = inputs[7].as_value() else { return None };

	let typesetting = TypesettingConfig {
		font_size,
		line_height_ratio,
		max_width,
		character_spacing,
		max_height,
	};
	Some((text, font, typesetting))
}

pub fn get_stroke_width(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<f64> {
	let weight_node_input_index = 2;
	if let TaggedValue::F64(width) = NodeGraphLayer::new(layer, network_interface).find_input("Stroke", weight_node_input_index)? {
		Some(*width)
	} else {
		None
	}
}

/// Checks if a specified layer uses an upstream node matching the given name.
pub fn is_layer_fed_by_node_of_name(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface, node_name: &str) -> bool {
	NodeGraphLayer::new(layer, network_interface).find_node_inputs(node_name).is_some()
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

	/// Node id of a node if it exists in the layer's primary flow
	pub fn upstream_node_id_from_name(&self, node_name: &str) -> Option<NodeId> {
		self.horizontal_layer_flow()
			.find(|node_id| self.network_interface.reference(node_id, &[]).is_some_and(|reference| *reference == Some(node_name.to_string())))
	}

	/// Node id of a protonode if it exists in the layer's primary flow
	pub fn upstream_node_id_from_protonode(&self, protonode_identifier: &'static str) -> Option<NodeId> {
		self.horizontal_layer_flow().find(move |node_id| {
			self.network_interface
				.implementation(node_id, &[])
				.is_some_and(move |implementation| *implementation == graph_craft::document::DocumentNodeImplementation::proto(protonode_identifier))
		})
	}

	/// Find all of the inputs of a specific node within the layer's primary flow, up until the next layer is reached.
	pub fn find_node_inputs(&self, node_name: &str) -> Option<&'a Vec<NodeInput>> {
		self.horizontal_layer_flow()
			.skip(1)// Skip self
			.take_while(|node_id| !self.network_interface.is_layer(node_id,&[]))
			.find(|node_id| self.network_interface.reference(node_id,&[]).is_some_and(|reference| *reference == Some(node_name.to_string())))
			.and_then(|node_id| self.network_interface.document_network().nodes.get(&node_id).map(|node| &node.inputs))
	}

	/// Find a specific input of a node within the layer's primary flow
	pub fn find_input(&self, node_name: &str, index: usize) -> Option<&'a TaggedValue> {
		// TODO: Find a better way to accept a node input rather than using its index (which is quite unclear and fragile)
		self.find_node_inputs(node_name)?.get(index)?.as_value()
	}

	/// Check if a layer is a raster layer
	pub fn is_raster_layer(layer: LayerNodeIdentifier, network_interface: &mut NodeNetworkInterface) -> bool {
		let layer_input_type = network_interface.input_type(&InputConnector::node(layer.to_node(), 1), &[]).0.nested_type();
		if layer_input_type == concrete!(graphene_core::raster::image::ImageFrameTable<graphene_core::Color>)
			|| layer_input_type == concrete!(graphene_core::application_io::TextureFrameTable)
			|| layer_input_type == concrete!(graphene_std::RasterFrame)
		{
			return true;
		}
		false
	}
}

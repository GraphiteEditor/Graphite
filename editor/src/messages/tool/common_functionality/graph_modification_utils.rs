use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{FlowType, NodeNetworkInterface, NodeTemplate};
use crate::messages::prelude::*;
use bezier_rs::Subpath;
use graph_craft::document::{value::TaggedValue, NodeId, NodeInput};
use graphene_core::raster::{BlendMode, ImageFrame};
use graphene_core::text::{Font, TypesettingConfig};
use graphene_core::vector::style::Gradient;
use graphene_core::vector::PointId;
use graphene_core::Color;

use glam::DVec2;
use std::collections::VecDeque;

/// Create a new vector layer from a vector of [`bezier_rs::Subpath`].
pub fn new_vector_layer(subpaths: Vec<Subpath<PointId>>, id: NodeId, parent: LayerNodeIdentifier, responses: &mut VecDeque<Message>) -> LayerNodeIdentifier {
	let insert_index = 0;
	responses.add(GraphOperationMessage::NewVectorLayer { id, subpaths, parent, insert_index });
	responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![id] });

	LayerNodeIdentifier::new_unchecked(id)
}

/// Create a new bitmap layer from an [`graphene_core::raster::ImageFrame<Color>`]
pub fn new_image_layer(image_frame: ImageFrame<Color>, id: NodeId, parent: LayerNodeIdentifier, responses: &mut VecDeque<Message>) -> LayerNodeIdentifier {
	let insert_index = 0;
	responses.add(GraphOperationMessage::NewBitmapLayer {
		id,
		image_frame,
		parent,
		insert_index,
	});
	LayerNodeIdentifier::new_unchecked(id)
}

/// Create a new group layer from an svg
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
	pub fn horizontal_layer_flow(&self) -> impl Iterator<Item = NodeId> + 'a {
		self.network_interface.upstream_flow_back_from_nodes(vec![self.layer_node], &[], FlowType::HorizontalFlow)
	}

	/// Node id of a node if it exists in the layer's primary flow
	pub fn upstream_node_id_from_name(&self, node_name: &str) -> Option<NodeId> {
		self.horizontal_layer_flow()
			.find(|node_id| self.network_interface.reference(node_id, &[]).is_some_and(|reference| *reference == Some(node_name.to_string())))
	}

	/// Find all of the inputs of a specific node within the layer's primary flow, up until the next layer is reached.
	pub fn find_node_inputs(&self, node_name: &str) -> Option<&'a Vec<NodeInput>> {
		self.horizontal_layer_flow()
			.skip(1)// Skip self
			.take_while(|node_id| !self.network_interface.is_layer(node_id,&[]))
			.find(|node_id| self.network_interface.reference(node_id,&[]).is_some_and(|reference| *reference == Some(node_name.to_string())))
			.and_then(|node_id| self.network_interface.network(&[]).unwrap().nodes.get(&node_id).map(|node| &node.inputs))
	}

	/// Find a specific input of a node within the layer's primary flow
	pub fn find_input(&self, node_name: &str, index: usize) -> Option<&'a TaggedValue> {
		// TODO: Find a better way to accept a node input rather than using its index (which is quite unclear and fragile)
		self.find_node_inputs(node_name)?.get(index)?.as_value()
	}
}

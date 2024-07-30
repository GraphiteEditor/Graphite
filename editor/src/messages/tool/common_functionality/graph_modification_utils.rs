use crate::messages::portfolio::document::utility_types::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use crate::messages::prelude::*;
use bezier_rs::Subpath;
use graph_craft::document::{value::TaggedValue, DocumentNode, NodeId, NodeInput, NodeNetwork};
use graphene_core::raster::{BlendMode, ImageFrame};
use graphene_core::text::Font;
use graphene_core::vector::style::Gradient;
use graphene_core::vector::PointId;
use graphene_core::Color;

use glam::DVec2;
use std::collections::VecDeque;

/// Create a new vector layer from a vector of [`bezier_rs::Subpath`].
pub fn new_vector_layer(subpaths: Vec<Subpath<PointId>>, id: NodeId, parent: LayerNodeIdentifier, responses: &mut VecDeque<Message>) -> LayerNodeIdentifier {
	let insert_index = -1;
	responses.add(GraphOperationMessage::NewVectorLayer { id, subpaths, parent, insert_index });
	responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![id] });

	LayerNodeIdentifier::new_unchecked(id)
}

/// Create a new bitmap layer from an [`graphene_core::raster::ImageFrame<Color>`]
pub fn new_image_layer(image_frame: ImageFrame<Color>, id: NodeId, parent: LayerNodeIdentifier, responses: &mut VecDeque<Message>) -> LayerNodeIdentifier {
	let insert_index = -1;
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
	let insert_index = -1;
	responses.add(DocumentMessage::ImportSvg {
		id,
		svg,
		transform,
		parent,
		insert_index,
	});
	LayerNodeIdentifier::new_unchecked(id)
}
pub fn new_custom(id: NodeId, nodes: HashMap<NodeId, DocumentNode>, parent: LayerNodeIdentifier, responses: &mut VecDeque<Message>) -> LayerNodeIdentifier {
	responses.add(GraphOperationMessage::NewCustomLayer {
		id,
		nodes,
		parent,
		insert_index: -1,
		alias: String::new(),
	});
	responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![id] });
	LayerNodeIdentifier::new_unchecked(id)
}

/// Locate the final pivot from the transform (TODO: decide how the pivot should actually work)
pub fn get_pivot(layer: LayerNodeIdentifier, network: &NodeNetwork) -> Option<DVec2> {
	let pivot_node_input_index = 5;
	if let TaggedValue::DVec2(pivot) = NodeGraphLayer::new(layer, network).find_input("Transform", pivot_node_input_index)? {
		Some(*pivot)
	} else {
		None
	}
}

pub fn get_viewport_pivot(layer: LayerNodeIdentifier, document_network: &NodeNetwork, document_metadata: &DocumentMetadata) -> DVec2 {
	let [min, max] = document_metadata.nonzero_bounding_box(layer);
	let pivot = get_pivot(layer, document_network).unwrap_or(DVec2::splat(0.5));
	document_metadata.transform_to_viewport(layer).transform_point2(min + (max - min) * pivot)
}

/// Get the current gradient of a layer from the closest Fill node
pub fn get_gradient(layer: LayerNodeIdentifier, document_network: &NodeNetwork) -> Option<Gradient> {
	let fill_index = 1;

	let inputs = NodeGraphLayer::new(layer, document_network).find_node_inputs("Fill")?;
	let TaggedValue::Fill(graphene_std::vector::style::Fill::Gradient(gradient)) = inputs.get(fill_index)?.as_value()? else {
		return None;
	};
	Some(gradient.clone())
}

/// Get the current fill of a layer from the closest Fill node
pub fn get_fill_color(layer: LayerNodeIdentifier, document_network: &NodeNetwork) -> Option<Color> {
	let fill_index = 1;

	let inputs = NodeGraphLayer::new(layer, document_network).find_node_inputs("Fill")?;
	let TaggedValue::Fill(graphene_std::vector::style::Fill::Solid(color)) = inputs.get(fill_index)?.as_value()? else {
		return None;
	};
	Some(*color)
}

/// Get the current blend mode of a layer from the closest Blend Mode node
pub fn get_blend_mode(layer: LayerNodeIdentifier, document_network: &NodeNetwork) -> Option<BlendMode> {
	let inputs = NodeGraphLayer::new(layer, document_network).find_node_inputs("Blend Mode")?;
	let TaggedValue::BlendMode(blend_mode) = inputs.get(1)?.as_value()? else {
		return None;
	};
	Some(*blend_mode)
}

/// Get the current opacity of a layer from the closest Opacity node.
/// This may differ from the actual opacity contained within the data type reaching this layer, because that actual opacity may be:
/// - Multiplied with additional opacity nodes earlier in the chain
/// - Set by an Opacity node with an exposed parameter value driven by another node
/// - Already factored into the pixel alpha channel of an image
/// - The default value of 100% if no Opacity node is present, but this function returns None in that case
/// With those limitations in mind, the intention of this function is to show just the value already present in an upstream Opacity node so that value can be directly edited.
pub fn get_opacity(layer: LayerNodeIdentifier, document_network: &NodeNetwork) -> Option<f64> {
	let inputs = NodeGraphLayer::new(layer, document_network).find_node_inputs("Opacity")?;
	let TaggedValue::F64(opacity) = inputs.get(1)?.as_value()? else {
		return None;
	};
	Some(*opacity)
}

pub fn get_fill_id(layer: LayerNodeIdentifier, document_network: &NodeNetwork) -> Option<NodeId> {
	NodeGraphLayer::new(layer, document_network).upstream_node_id_from_name("Fill")
}

pub fn get_text_id(layer: LayerNodeIdentifier, document_network: &NodeNetwork) -> Option<NodeId> {
	NodeGraphLayer::new(layer, document_network).upstream_node_id_from_name("Text")
}

/// Gets properties from the Text node
pub fn get_text(layer: LayerNodeIdentifier, document_network: &NodeNetwork) -> Option<(&String, &Font, f64)> {
	let inputs = NodeGraphLayer::new(layer, document_network).find_node_inputs("Text")?;

	let Some(TaggedValue::String(text)) = &inputs[1].as_value() else { return None };
	let Some(TaggedValue::Font(font)) = &inputs[2].as_value() else { return None };
	let Some(&TaggedValue::F64(font_size)) = inputs[3].as_value() else { return None };

	Some((text, font, font_size))
}

pub fn get_stroke_width(layer: LayerNodeIdentifier, network: &NodeNetwork) -> Option<f64> {
	let weight_node_input_index = 2;
	if let TaggedValue::F64(width) = NodeGraphLayer::new(layer, network).find_input("Stroke", weight_node_input_index)? {
		Some(*width)
	} else {
		None
	}
}

/// Checks if a specified layer uses an upstream node matching the given name.
pub fn is_layer_fed_by_node_of_name(layer: LayerNodeIdentifier, document_network: &NodeNetwork, node_name: &str) -> bool {
	NodeGraphLayer::new(layer, document_network).find_node_inputs(node_name).is_some()
}

/// An immutable reference to a layer within the document node graph for easy access.
pub struct NodeGraphLayer<'a> {
	node_graph: &'a NodeNetwork,
	layer_node: NodeId,
}

impl<'a> NodeGraphLayer<'a> {
	/// Get the layer node from the document
	pub fn new(layer: LayerNodeIdentifier, network: &'a NodeNetwork) -> Self {
		debug_assert!(layer != LayerNodeIdentifier::ROOT_PARENT, "Cannot create new NodeGraphLayer from ROOT_PARENT");
		Self {
			node_graph: network,
			layer_node: layer.to_node(),
		}
	}

	/// Return an iterator up the horizontal flow of the layer
	pub fn horizontal_layer_flow(&self) -> impl Iterator<Item = (&'a DocumentNode, NodeId)> {
		self.node_graph.upstream_flow_back_from_nodes(vec![self.layer_node], graph_craft::document::FlowType::HorizontalFlow)
	}

	/// Node id of a node if it exists in the layer's primary flow
	pub fn upstream_node_id_from_name(&self, node_name: &str) -> Option<NodeId> {
		self.horizontal_layer_flow().find(|(node, _)| node.name == node_name).map(|(_, id)| id)
	}

	/// Find all of the inputs of a specific node within the layer's primary flow, up until the next layer is reached.
	pub fn find_node_inputs(&self, node_name: &str) -> Option<&'a Vec<NodeInput>> {
		self.horizontal_layer_flow()
			.skip(1)// Skip self
			.take_while(|(node, _)| !node.is_layer)
			.find(|(node, _)| node.name == node_name)
			.map(|(node, _id)| &node.inputs)
	}

	/// Find a specific input of a node within the layer's primary flow
	pub fn find_input(&self, node_name: &str, index: usize) -> Option<&'a TaggedValue> {
		// TODO: Find a better way to accept a node input rather than using its index (which is quite unclear and fragile)
		self.find_node_inputs(node_name)?.get(index)?.as_value()
	}
}

use crate::messages::portfolio::document::node_graph::VectorDataModification;
use crate::messages::portfolio::document::utility_types::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use crate::messages::prelude::*;

use bezier_rs::{ManipulatorGroup, Subpath};
use graph_craft::document::{value::TaggedValue, DocumentNode, NodeId, NodeInput, NodeNetwork};
use graphene_core::raster::{BlendMode, ImageFrame};
use graphene_core::text::Font;
use graphene_core::uuid::ManipulatorGroupId;
use graphene_core::vector::style::{FillType, Gradient};
use graphene_core::Color;

use glam::DVec2;
use std::collections::VecDeque;

/// Create a new vector layer from a vector of [`bezier_rs::Subpath`].
pub fn new_vector_layer(subpaths: Vec<Subpath<ManipulatorGroupId>>, id: NodeId, parent: LayerNodeIdentifier, responses: &mut VecDeque<Message>) -> LayerNodeIdentifier {
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
	responses.add(GraphOperationMessage::NewSvg {
		id,
		svg,
		transform,
		parent,
		insert_index,
	});
	LayerNodeIdentifier::new_unchecked(id)
}

/// Batch set all of the manipulator groups to mirror on a specific layer
pub fn set_manipulator_mirror_angle(manipulator_groups: &[ManipulatorGroup<ManipulatorGroupId>], layer: LayerNodeIdentifier, mirror_angle: bool, responses: &mut VecDeque<Message>) {
	for manipulator_group in manipulator_groups {
		responses.add(GraphOperationMessage::Vector {
			layer,
			modification: VectorDataModification::SetManipulatorHandleMirroring {
				id: manipulator_group.id,
				mirror_angle,
			},
		});
	}
}

/// Locate the subpaths from the shape nodes of a particular layer
pub fn get_subpaths(layer: LayerNodeIdentifier, document_network: &NodeNetwork) -> Option<&Vec<Subpath<ManipulatorGroupId>>> {
	if let TaggedValue::Subpaths(subpaths) = NodeGraphLayer::new(layer, document_network)?.find_input("Shape", 0)? {
		Some(subpaths)
	} else {
		None
	}
}

/// Locate the final pivot from the transform (TODO: decide how the pivot should actually work)
pub fn get_pivot(layer: LayerNodeIdentifier, network: &NodeNetwork) -> Option<DVec2> {
	if let TaggedValue::DVec2(pivot) = NodeGraphLayer::new(layer, network)?.find_input("Transform", 5)? {
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

/// Get the currently mirrored handles for a particular layer from the shape node
pub fn get_mirror_handles(layer: LayerNodeIdentifier, document_network: &NodeNetwork) -> Option<&Vec<ManipulatorGroupId>> {
	if let TaggedValue::ManipulatorGroupIds(mirror_handles) = NodeGraphLayer::new(layer, document_network)?.find_input("Shape", 1)? {
		Some(mirror_handles)
	} else {
		None
	}
}

/// Get the current gradient of a layer from the closest Fill node
pub fn get_gradient(layer: LayerNodeIdentifier, document_network: &NodeNetwork) -> Option<Gradient> {
	let inputs = NodeGraphLayer::new(layer, document_network)?.find_node_inputs("Fill")?;
	let TaggedValue::FillType(FillType::Gradient) = inputs.get(1)?.as_value()? else {
		return None;
	};
	let TaggedValue::GradientType(gradient_type) = inputs.get(3)?.as_value()? else {
		return None;
	};
	let TaggedValue::DVec2(start) = inputs.get(4)?.as_value()? else {
		return None;
	};
	let TaggedValue::DVec2(end) = inputs.get(5)?.as_value()? else {
		return None;
	};
	let TaggedValue::DAffine2(transform) = inputs.get(6)?.as_value()? else {
		return None;
	};
	let TaggedValue::GradientPositions(positions) = inputs.get(7)?.as_value()? else {
		return None;
	};
	Some(Gradient {
		start: *start,
		end: *end,
		transform: *transform,
		positions: positions.clone(),
		gradient_type: *gradient_type,
	})
}

/// Get the current fill of a layer from the closest Fill node
pub fn get_fill_color(layer: LayerNodeIdentifier, document_network: &NodeNetwork) -> Option<Color> {
	let inputs = NodeGraphLayer::new(layer, document_network)?.find_node_inputs("Fill")?;
	let TaggedValue::Color(color) = inputs.get(2)?.as_value()? else {
		return None;
	};
	Some(*color)
}

/// Get the current blend mode of a layer from the closest Blend Mode node
pub fn get_blend_mode(layer: LayerNodeIdentifier, document_network: &NodeNetwork) -> Option<BlendMode> {
	let inputs = NodeGraphLayer::new(layer, document_network)?.find_node_inputs("Blend Mode")?;
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
	let inputs = NodeGraphLayer::new(layer, document_network)?.find_node_inputs("Opacity")?;
	let TaggedValue::F64(opacity) = inputs.get(1)?.as_value()? else {
		return None;
	};
	Some(*opacity)
}

pub fn get_fill_id(layer: LayerNodeIdentifier, document_network: &NodeNetwork) -> Option<NodeId> {
	NodeGraphLayer::new(layer, document_network)?.node_id("Fill")
}

pub fn get_text_id(layer: LayerNodeIdentifier, document_network: &NodeNetwork) -> Option<NodeId> {
	NodeGraphLayer::new(layer, document_network)?.node_id("Text")
}

/// Gets properties from the Text node
pub fn get_text(layer: LayerNodeIdentifier, document_network: &NodeNetwork) -> Option<(&String, &Font, f64)> {
	let inputs = NodeGraphLayer::new(layer, document_network)?.find_node_inputs("Text")?;
	let NodeInput::Value {
		tagged_value: TaggedValue::String(text),
		..
	} = &inputs[1]
	else {
		return None;
	};

	let NodeInput::Value {
		tagged_value: TaggedValue::Font(font),
		..
	} = &inputs[2]
	else {
		return None;
	};

	let NodeInput::Value {
		tagged_value: TaggedValue::F64(font_size),
		..
	} = inputs[3]
	else {
		return None;
	};

	Some((text, font, font_size))
}

pub fn get_stroke_width(layer: LayerNodeIdentifier, network: &NodeNetwork) -> Option<f64> {
	if let TaggedValue::F64(width) = NodeGraphLayer::new(layer, network)?.find_input("Stroke", 2)? {
		Some(*width)
	} else {
		None
	}
}

/// Checks if a specified layer uses an upstream node matching the given name.
pub fn is_layer_fed_by_node_of_name(layer: LayerNodeIdentifier, document_network: &NodeNetwork, node_name: &str) -> bool {
	NodeGraphLayer::new(layer, document_network).is_some_and(|layer| layer.find_node_inputs(node_name).is_some())
}

/// Convert subpaths to an iterator of manipulator groups
pub fn get_manipulator_groups(subpaths: &[Subpath<ManipulatorGroupId>]) -> impl Iterator<Item = &bezier_rs::ManipulatorGroup<ManipulatorGroupId>> + DoubleEndedIterator {
	subpaths.iter().flat_map(|subpath| subpath.manipulator_groups())
}

/// Find a manipulator group with a specific id from several subpaths
pub fn get_manipulator_from_id(subpaths: &[Subpath<ManipulatorGroupId>], id: ManipulatorGroupId) -> Option<&bezier_rs::ManipulatorGroup<ManipulatorGroupId>> {
	subpaths.iter().find_map(|subpath| subpath.manipulator_from_id(id))
}

/// An immutable reference to a layer within the document node graph for easy access.
pub struct NodeGraphLayer<'a> {
	node_graph: &'a NodeNetwork,
	_outwards_links: HashMap<NodeId, Vec<NodeId>>,
	layer_node: NodeId,
}

impl<'a> NodeGraphLayer<'a> {
	/// Get the layer node from the document
	pub fn new(layer: LayerNodeIdentifier, network: &'a NodeNetwork) -> Option<Self> {
		let outwards_links = network.collect_outwards_links();

		Some(Self {
			node_graph: network,
			_outwards_links: outwards_links,
			layer_node: layer.to_node(),
		})
	}

	/// Return an iterator up the primary flow of the layer
	pub fn primary_layer_flow(&self) -> impl Iterator<Item = (&'a DocumentNode, NodeId)> {
		self.node_graph.upstream_flow_back_from_nodes(vec![self.layer_node], true)
	}

	/// Node id of a node if it exists in the layer's primary flow
	pub fn node_id(&self, node_name: &str) -> Option<NodeId> {
		self.primary_layer_flow().find(|(node, _id)| node.name == node_name).map(|(_node, id)| id)
	}

	/// Find all of the inputs of a specific node within the layer's primary flow, up until the next layer is reached.
	pub fn find_node_inputs(&self, node_name: &str) -> Option<&'a Vec<NodeInput>> {
		self.primary_layer_flow()
			.skip(1)
			.take_while(|(node, _)| !node.is_layer())
			.find(|(node, _)| node.name == node_name)
			.map(|(node, _id)| &node.inputs)
	}

	/// Find a specific input of a node within the layer's primary flow
	pub fn find_input(&self, node_name: &str, index: usize) -> Option<&'a TaggedValue> {
		self.find_node_inputs(node_name)?.get(index)?.as_value()
	}
}

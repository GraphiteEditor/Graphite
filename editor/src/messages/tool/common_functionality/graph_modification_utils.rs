use crate::messages::portfolio::document::node_graph::VectorDataModification;
use crate::messages::prelude::*;

use bezier_rs::{ManipulatorGroup, Subpath};
use document_legacy::{document::Document, document_metadata::LayerNodeIdentifier, LayerId, Operation};
use graph_craft::document::{value::TaggedValue, DocumentNode, NodeId, NodeInput, NodeNetwork};
use graphene_core::raster::ImageFrame;
use graphene_core::text::Font;
use graphene_core::uuid::ManipulatorGroupId;
use graphene_core::vector::style::{FillType, Gradient};
use graphene_core::Color;

use glam::{DAffine2, DVec2};
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

/// Create a legacy node graph frame TODO: remove
pub fn new_custom_layer(network: NodeNetwork, layer_path: Vec<LayerId>, responses: &mut VecDeque<Message>) {
	responses.add(DocumentMessage::DeselectAllLayers);
	responses.add(Operation::AddFrame {
		path: layer_path.clone(),
		insert_index: -1,
		transform: DAffine2::ZERO.to_cols_array(),
		network,
	});
	responses.add(DocumentMessage::InputFrameRasterizeRegionBelowLayer { layer_path });
}

/// Batch set all of the manipulator groups to mirror on a specific layer
pub fn set_manipulator_mirror_angle(manipulator_groups: &[ManipulatorGroup<ManipulatorGroupId>], layer: LayerNodeIdentifier, mirror_angle: bool, responses: &mut VecDeque<Message>) {
	for manipulator_group in manipulator_groups {
		responses.add(GraphOperationMessage::Vector {
			layer: layer.to_path(),
			modification: VectorDataModification::SetManipulatorHandleMirroring {
				id: manipulator_group.id,
				mirror_angle,
			},
		});
	}
}

/// Locate the subpaths from the shape nodes of a particular layer
pub fn get_subpaths(layer: LayerNodeIdentifier, document: &Document) -> Option<&Vec<Subpath<ManipulatorGroupId>>> {
	if let TaggedValue::Subpaths(subpaths) = NodeGraphLayer::new(layer, document)?.find_input("Shape", 0)? {
		Some(subpaths)
	} else {
		None
	}
}

/// Locate the final pivot from the transform (TODO: decide how the pivot should actually work)
pub fn get_pivot(layer: LayerNodeIdentifier, document: &Document) -> Option<DVec2> {
	if let TaggedValue::DVec2(pivot) = NodeGraphLayer::new(layer, document)?.find_input("Transform", 5)? {
		Some(*pivot)
	} else {
		None
	}
}

pub fn get_document_pivot(layer: LayerNodeIdentifier, document: &Document) -> Option<DVec2> {
	let [min, max] = document.metadata.nonzero_bounding_box(layer);
	get_pivot(layer, document).map(|pivot| document.metadata.transform_to_document(layer).transform_point2(min + (max - min) * pivot))
}

pub fn get_viewport_pivot(layer: LayerNodeIdentifier, document: &Document) -> Option<DVec2> {
	let [min, max] = document.metadata.nonzero_bounding_box(layer);
	get_pivot(layer, document).map(|pivot| document.metadata.transform_to_viewport(layer).transform_point2(min + (max - min) * pivot))
}

/// Get the currently mirrored handles for a particular layer from the shape node
pub fn get_mirror_handles(layer: LayerNodeIdentifier, document: &Document) -> Option<&Vec<ManipulatorGroupId>> {
	if let TaggedValue::ManipulatorGroupIds(mirror_handles) = NodeGraphLayer::new(layer, document)?.find_input("Shape", 1)? {
		Some(mirror_handles)
	} else {
		None
	}
}

/// Get the current gradient of a layer from the closest fill node
pub fn get_gradient(layer: LayerNodeIdentifier, document: &Document) -> Option<Gradient> {
	let inputs = NodeGraphLayer::new(layer, document)?.find_node_inputs("Fill")?;
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

/// Get the current fill of a layer from the closest fill node
pub fn get_fill_color(layer: LayerNodeIdentifier, document: &Document) -> Option<Color> {
	let inputs = NodeGraphLayer::new(layer, document)?.find_node_inputs("Fill")?;
	let TaggedValue::Color(color) = inputs.get(2)?.as_value()? else {
		return None;
	};
	Some(*color)
}

pub fn get_text_id(layer: LayerNodeIdentifier, document: &Document) -> Option<NodeId> {
	NodeGraphLayer::new(layer, document)?.node_id("Text")
}
pub fn get_fill_id(layer: LayerNodeIdentifier, document: &Document) -> Option<NodeId> {
	NodeGraphLayer::new(layer, document)?.node_id("Fill")
}

/// Gets properties from the text node
pub fn get_text(layer: LayerNodeIdentifier, document: &Document) -> Option<(&String, &Font, f64)> {
	let inputs = NodeGraphLayer::new(layer, document)?.find_node_inputs("Text")?;
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

/// Is a specified layer an artboard?
pub fn is_artboard(layer: LayerNodeIdentifier, document: &Document) -> bool {
	NodeGraphLayer::new(layer, document).is_some_and(|layer| layer.uses_node("Artboard"))
}

/// Is a specified layer a shape?
pub fn is_shape_layer(layer: LayerNodeIdentifier, document: &Document) -> bool {
	NodeGraphLayer::new(layer, document).is_some_and(|layer| layer.uses_node("Shape"))
}

/// Is a specified layer text?
pub fn is_text_layer(layer: LayerNodeIdentifier, document: &Document) -> bool {
	NodeGraphLayer::new(layer, document).is_some_and(|layer| layer.uses_node("Text"))
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
	pub fn new(layer: LayerNodeIdentifier, document: &'a document_legacy::document::Document) -> Option<Self> {
		let node_graph = &document.document_network;
		let outwards_links = document.document_network.collect_outwards_links();

		Some(Self {
			node_graph,
			_outwards_links: outwards_links,
			layer_node: layer.to_node(),
		})
	}

	/// Get the nearest layer node from the path and the document
	pub fn new_from_path(layer: &[LayerId], document: &'a document_legacy::document::Document) -> Option<Self> {
		let node_graph = &document.document_network;
		let outwards_links = document.document_network.collect_outwards_links();

		let Some(mut layer_node) = layer.last().copied() else {
			error!("Tried to modify root layer");
			return None;
		};
		while node_graph.nodes.get(&layer_node)?.name != "Layer" {
			layer_node = outwards_links.get(&layer_node)?.first().copied()?;
		}
		Some(Self {
			node_graph,
			_outwards_links: outwards_links,
			layer_node,
		})
	}

	/// Return an iterator up the primary flow of the layer
	pub fn primary_layer_flow(&self) -> impl Iterator<Item = (&'a DocumentNode, u64)> {
		self.node_graph.primary_flow_from_node(Some(self.layer_node))
	}

	/// Does a node exist in the layer's primary flow
	pub fn uses_node(&self, node_name: &str) -> bool {
		self.primary_layer_flow().any(|(node, _id)| node.name == node_name)
	}

	/// Node id of a node if it exists in the layer's primary flow
	pub fn node_id(&self, node_name: &str) -> Option<NodeId> {
		self.primary_layer_flow().find(|(node, _id)| node.name == node_name).map(|(_node, id)| id)
	}

	/// Find all of the inputs of a specific node within the layer's primary flow
	pub fn find_node_inputs(&self, node_name: &str) -> Option<&'a Vec<NodeInput>> {
		self.primary_layer_flow().find(|(node, _id)| node.name == node_name).map(|(node, _id)| &node.inputs)
	}

	/// Find a specific input of a node within the layer's primary flow
	pub fn find_input(&self, node_name: &str, index: usize) -> Option<&'a TaggedValue> {
		self.find_node_inputs(node_name)?.get(index)?.as_value()
	}
}

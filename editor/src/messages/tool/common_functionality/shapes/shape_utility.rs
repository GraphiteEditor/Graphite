use crate::messages::message::Message;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::prelude::{DocumentMessageHandler, NodeGraphMessage, Responses};
use crate::messages::tool::common_functionality::graph_modification_utils::{self, NodeGraphLayer};
use crate::messages::tool::common_functionality::transformation_cage::BoundingBoxManager;
use crate::messages::tool::tool_messages::tool_prelude::Key;
use crate::messages::tool::utility_types::*;
use glam::{DMat2, DVec2};
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use std::collections::VecDeque;
use std::f64::consts::PI;

use super::ShapeToolData;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum ShapeType {
	#[default]
	Convex = 0,
	Star = 1,
	Rectangle = 2,
	Ellipse = 3,
	Line = 4,
}

impl ShapeType {
	pub fn name(&self) -> String {
		match self {
			Self::Convex => "Convex",
			Self::Star => "Star",
			Self::Rectangle => "Rectangle",
			Self::Ellipse => "Ellipse",
			Self::Line => "Line",
		}
		.into()
	}

	pub fn tooltip(&self) -> String {
		match self {
			Self::Line => "Line tool",
			Self::Rectangle => "Rectangle tool",
			Self::Ellipse => "Ellipse tool",
			_ => "",
		}
		.into()
	}

	pub fn icon_name(&self) -> String {
		match self {
			Self::Line => "VectorLineTool",
			Self::Rectangle => "VectorRectangleTool",
			Self::Ellipse => "VectorEllipseTool",
			_ => "",
		}
		.into()
	}

	pub fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		match self {
			Self::Line => ToolType::Line,
			Self::Rectangle => ToolType::Rectangle,
			Self::Ellipse => ToolType::Ellipse,
			_ => ToolType::Shape,
		}
	}
}

// Center, Lock Ratio, Lock Angle, Snap Angle
pub type ShapeToolModifierKey = [Key; 4];

pub fn update_radius_sign(end: DVec2, start: DVec2, layer: LayerNodeIdentifier, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	let sign_num = if end[1] > start[1] { 1. } else { -1. };
	let new_layer = NodeGraphLayer::new(layer, &document.network_interface);

	if new_layer.find_input("Regular Polygon", 1).unwrap_or(&TaggedValue::U32(0)).to_u32() % 2 == 1 {
		let Some(polygon_node_id) = new_layer.upstream_node_id_from_name("Regular Polygon") else { return };

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(polygon_node_id, 2),
			input: NodeInput::value(TaggedValue::F64(sign_num * 0.5), false),
		});
		return;
	}

	if new_layer.find_input("Star", 1).unwrap_or(&TaggedValue::U32(0)).to_u32() % 2 == 1 {
		let Some(star_node_id) = new_layer.upstream_node_id_from_name("Star") else { return };

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(star_node_id, 2),
			input: NodeInput::value(TaggedValue::F64(sign_num * 0.5), false),
		});
		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(star_node_id, 3),
			input: NodeInput::value(TaggedValue::F64(sign_num * 0.25), false),
		});
	}
}

pub fn transform_cage_overlays(document: &DocumentMessageHandler, tool_data: &mut ShapeToolData, overlay_context: &mut OverlayContext) {
	let mut transform = document
		.network_interface
		.selected_nodes()
		.selected_visible_and_unlocked_layers(&document.network_interface)
		.find(|layer| !document.network_interface.is_artboard(&layer.to_node(), &[]))
		.map(|layer| document.metadata().transform_to_viewport_with_first_transform_node_if_group(layer, &document.network_interface))
		.unwrap_or_default();

	// Check if the matrix is not invertible
	let mut transform_tampered = false;
	if transform.matrix2.determinant() == 0. {
		transform.matrix2 += DMat2::IDENTITY * 1e-4; // TODO: Is this the cleanest way to handle this?
		transform_tampered = true;
	}

	let bounds = document
		.network_interface
		.selected_nodes()
		.selected_visible_and_unlocked_layers(&document.network_interface)
		.filter(|layer| !document.network_interface.is_artboard(&layer.to_node(), &[]))
		.filter_map(|layer| {
			document
				.metadata()
				.bounding_box_with_transform(layer, transform.inverse() * document.metadata().transform_to_viewport(layer))
		})
		.reduce(graphene_core::renderer::Quad::combine_bounds);

	if let Some(bounds) = bounds {
		let bounding_box_manager = tool_data.bounding_box_manager.get_or_insert(BoundingBoxManager::default());

		bounding_box_manager.bounds = bounds;
		bounding_box_manager.transform = transform;
		bounding_box_manager.transform_tampered = transform_tampered;
		bounding_box_manager.render_overlays(overlay_context, true);
	} else {
		tool_data.bounding_box_manager.take();
	}
}

pub fn anchor_overlays(document: &DocumentMessageHandler, overlay_context: &mut OverlayContext) {
	for layer in document.network_interface.selected_nodes().selected_layers(document.metadata()) {
		let Some(vector_data) = document.network_interface.compute_modified_vector(layer) else { continue };
		let transform = document.metadata().transform_to_viewport(layer);

		overlay_context.outline_vector(&vector_data, transform);

		for (_, &position) in vector_data.point_domain.ids().iter().zip(vector_data.point_domain.positions()) {
			overlay_context.manipulator_anchor(transform.transform_point2(position), false, None);
		}
	}
}

pub fn points_on_inner_circle(document: &DocumentMessageHandler, mouse_position: DVec2) -> Option<(LayerNodeIdentifier, u32, usize, f64)> {
	for layer in document
		.network_interface
		.selected_nodes()
		.selected_visible_and_unlocked_layers(&document.network_interface)
		.filter(|layer| graph_modification_utils::get_star_id(*layer, &document.network_interface).is_some())
	{
		let Some(node_inputs) = NodeGraphLayer::new(layer, &document.network_interface).find_node_inputs("Star") else {
			continue;
		};

		let viewport = document.network_interface.document_metadata().transform_to_viewport(layer);

		let (Some(&TaggedValue::U32(n)), Some(&TaggedValue::F64(outer)), Some(&TaggedValue::F64(inner))) = (node_inputs[1].as_value(), node_inputs[2].as_value(), node_inputs[3].as_value()) else {
			continue;
		};

		for i in 0..(2 * n) {
			let angle = i as f64 * PI / n as f64;
			let (radius, index) = if i % 2 == 0 { (outer, 2) } else { (inner, 3) };

			let point = viewport.transform_point2(DVec2 {
				x: radius * angle.sin(),
				y: -radius * angle.cos(),
			});

			if point.distance(mouse_position) < 5.0 {
				return Some((layer, i, index, radius));
			};
		}
	}
	None
}

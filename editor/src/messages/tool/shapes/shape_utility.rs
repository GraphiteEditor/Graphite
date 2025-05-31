use crate::messages::message::Message;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::prelude::{DocumentMessageHandler, NodeGraphMessage, Responses};
use crate::messages::tool::common_functionality::graph_modification_utils::NodeGraphLayer;
use crate::messages::tool::tool_messages::tool_prelude::Key;
use crate::messages::tool::utility_types::*;
use glam::DVec2;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use std::collections::{HashMap, VecDeque};

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

pub struct LineInitData {
	pub drag_start: DVec2,
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

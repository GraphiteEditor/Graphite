pub mod ellipse_shape;
pub mod line_shape;
pub mod rectangle_shape;

pub use super::shapes::ellipse_shape::Ellipse;
pub use super::shapes::line_shape::{Line, LineEnd};
pub use super::shapes::rectangle_shape::Rectangle;
pub use super::tool_messages::shape_tool::ShapeToolData;
use super::tool_messages::tool_prelude::*;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::NodeTemplate;
use glam::DVec2;
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum ShapeType {
	#[default]
	Rectangle,
	Ellipse,
	Line,
}

pub trait Shape: Default + Send + Sync {
	fn name() -> &'static str;
	fn icon_name() -> &'static str;
	fn create_node(document: &DocumentMessageHandler, shape_data: ShapeInitData) -> NodeTemplate;
	fn update_shape(
		document: &DocumentMessageHandler,
		ipp: &InputPreprocessorMessageHandler,
		layer: LayerNodeIdentifier,
		shape_tool_data: &mut ShapeToolData,
		shape_data: ShapeUpdateData,
		responses: &mut VecDeque<Message>,
	) -> bool;
}

pub enum ShapeInitData {
	Line { drag_start: DVec2 },
	Rectangle,
	Ellipse,
}

pub enum ShapeUpdateData {
	Ellipse { center: Key, lock_ratio: Key },
	Rectangle { center: Key, lock_ratio: Key },
	Line { center: Key, snap_angle: Key, lock_angle: Key },
}

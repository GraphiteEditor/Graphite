pub mod ellipse_shape;
pub mod rectangle_shape;

pub use super::shapes::ellipse_shape::Ellipse;
pub use super::shapes::rectangle_shape::Rectangle;
use super::tool_messages::tool_prelude::*;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use glam::DVec2;
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum ShapeType {
	Rectangle,
	#[default]
	Ellipse,
}

pub trait Shape: Default + Send + Sync {
	fn name() -> &'static str;
	fn icon_name() -> &'static str;
	fn create_node(document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) -> LayerNodeIdentifier;
	fn update_shape(document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, layer: LayerNodeIdentifier, start: DVec2, end: DVec2, responses: &mut VecDeque<Message>) -> bool;
}

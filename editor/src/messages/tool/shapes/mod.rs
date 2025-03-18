pub mod ellipse_shape;
pub mod line_shape;
pub mod rectangle_shape;

pub use super::shapes::ellipse_shape::Ellipse;
pub use super::shapes::line_shape::{Line, LineEnd};
pub use super::shapes::rectangle_shape::Rectangle;
pub use super::tool_messages::shape_tool::ShapeToolData;
use super::tool_messages::tool_prelude::*;
use glam::DVec2;

#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum ShapeType {
	#[default]
	Rectangle,
	Ellipse,
	Line,
}

pub struct LineInitData {
	pub drag_start: DVec2
}

// Center, Lock ratio, Lock angle, Snap angle
// Saved in unnamed fashion to reduce boilerplate required
pub type ShapeToolModifierKey = [Key; 4];

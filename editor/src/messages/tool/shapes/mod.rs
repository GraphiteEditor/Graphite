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

impl ShapeType {
	pub fn name(&self) -> String {
		match self {
			Self::Line => "Line",
			Self::Rectangle => "Rectangle",
			Self::Ellipse => "Ellipse",
		}.into()
	}

	pub fn tooltip(&self) -> String {
		match self {
			Self::Line => "Line tool",
			Self::Rectangle => "Rectangle tool",
			Self::Ellipse => "Ellipse tool",
		}.into()
	}

	pub fn icon_name(&self) -> String {
		match self {
			Self::Line => "VectorLineTool",
			Self::Rectangle => "VectorRectangleTool",
			Self::Ellipse => "VectorEllipseTool",
		}.into()
	}

	pub fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		match self {
			Self::Line => ToolType::Line,
			Self::Rectangle => ToolType::Rectangle,
			Self::Ellipse => ToolType::Ellipse,
		}
	}
}

pub struct LineInitData {
	pub drag_start: DVec2,
}

// Center, Lock Ratio, Lock Angle, Snap Angle
pub type ShapeToolModifierKey = [Key; 4];

pub mod click_target;
pub mod math_ext;
pub mod reference_point;
pub mod style;
mod vector_data;

pub use bezier_rs;

pub use vector_data::*;

pub fn point_to_dvec2(point: kurbo::Point) -> glam::DVec2 {
	glam::DVec2 { x: point.x, y: point.y }
}

pub fn dvec2_to_point(value: glam::DVec2) -> kurbo::Point {
	kurbo::Point { x: value.x, y: value.y }
}

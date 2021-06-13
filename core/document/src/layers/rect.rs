use kurbo::Point;
use kurbo::Shape;

use crate::intersection::intersect_quad_bez_path;
use crate::LayerId;

use super::style;
use super::LayerData;
use super::KURBO_TOLERANCE;

use std::fmt::Write;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
	shape: kurbo::Rect,
	style: style::PathStyle,
}

impl Rect {
	pub fn new(p0: impl Into<Point>, p1: impl Into<Point>, style: style::PathStyle) -> Rect {
		Rect {
			shape: kurbo::Rect::from_points(p0, p1),
			style,
		}
	}
}

impl LayerData for Rect {
	fn render(&mut self, svg: &mut String) {
		let _ = write!(
			svg,
			r#"<rect x="{}" y="{}" width="{}" height="{}"{} />"#,
			self.shape.min_x(),
			self.shape.min_y(),
			self.shape.width(),
			self.shape.height(),
			self.style.render(),
		);
	}

	fn intersects_quad(&self, quad: [Point; 4], path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>) {
		if intersect_quad_bez_path(quad, &self.shape.to_path(KURBO_TOLERANCE)) {
			intersections.push(path.clone());
		}
	}

	fn intersects_point(&self, point: Point, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>) {
		if self.shape.contains(point) {
			intersections.push(path.clone());
		}
	}
}

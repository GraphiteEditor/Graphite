use kurbo::Point;
use kurbo::Shape;

use crate::intersection::intersect_quad_bez_path;
use crate::LayerId;

use super::style;
use super::LayerData;
use super::KURBO_TOLERANCE;

use std::fmt::Write;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Circle {
	shape: kurbo::Circle,
	style: style::PathStyle,
}

impl Circle {
	pub fn new(center: impl Into<kurbo::Point>, radius: f64, style: style::PathStyle) -> Circle {
		Circle {
			shape: kurbo::Circle::new(center, radius),
			style,
		}
	}
}

impl LayerData for Circle {
	fn render(&mut self, svg: &mut String) {
		let _ = write!(
			svg,
			r#"<circle cx="{}" cy="{}" r="{}"{} />"#,
			self.shape.center.x,
			self.shape.center.y,
			self.shape.radius,
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

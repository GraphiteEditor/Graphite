use kurbo::Point;
use kurbo::Shape;
use kurbo::Vec2;

use crate::intersection::intersect_quad_bez_path;
use crate::LayerId;

use super::style;
use super::LayerData;
use super::KURBO_TOLERANCE;

use std::fmt::Write;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Ellipse {
	shape: kurbo::Ellipse,
	style: style::PathStyle,
}

impl Ellipse {
	pub fn new(center: impl Into<Point>, radii: impl Into<Vec2>, rotation: f64, style: style::PathStyle) -> Ellipse {
		Ellipse {
			shape: kurbo::Ellipse::new(center, radii, rotation),
			style,
		}
	}
}

impl LayerData for Ellipse {
	fn render(&mut self, svg: &mut String) {
		let kurbo::Vec2 { x: rx, y: ry } = self.shape.radii();
		let kurbo::Point { x: cx, y: cy } = self.shape.center();

		let _ = write!(
			svg,
			r#"<ellipse cx="0" cy="0" rx="{}" ry="{}" transform="translate({} {}) rotate({})"{} />"#,
			rx,
			ry,
			cx,
			cy,
			self.shape.rotation().to_degrees(),
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

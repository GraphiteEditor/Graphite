use kurbo::Point;

use crate::shape_points;
use crate::LayerId;

use super::style;
use super::LayerData;

use std::fmt::Write;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shape {
	bounding_rect: kurbo::Rect,
	shape: shape_points::ShapePoints,
	style: style::PathStyle,
}

impl Shape {
	pub fn new(p0: impl Into<Point>, p1: impl Into<Point>, sides: u8, style: style::PathStyle) -> Shape {
		Shape {
			bounding_rect: kurbo::Rect::from_points(p0, p1),
			shape: shape_points::ShapePoints::new(Point::new(0.5, 0.5), kurbo::Vec2::new(0.5, 0.0), sides),
			style,
		}
	}
}

impl LayerData for Shape {
	fn render(&mut self, svg: &mut String) {
		let _ = write!(
			svg,
			r#"<polygon points="{}" transform="translate({} {}) scale({} {})"{} />"#,
			self.shape,
			self.bounding_rect.origin().x,
			self.bounding_rect.origin().y,
			self.bounding_rect.width(),
			self.bounding_rect.height(),
			self.style.render(),
		);
	}

	fn intersects_quad(&self, quad: [Point; 4], path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>) {}

	fn intersects_point(&self, point: Point, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>) {}
}

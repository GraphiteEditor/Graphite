use glam::DAffine2;
use glam::DVec2;
use kurbo::Point;
use kurbo::Shape;

use crate::intersection::intersect_quad_bez_path;
use crate::LayerId;
use crate::intersection::transform_kurbo_point;

use super::style;
use super::LayerData;

use std::fmt::Write;

#[derive(Debug, Clone, PartialEq)]
pub struct Rect {}

impl Rect {
	pub fn new() -> Rect {
		Rect {}
	}
}

impl LayerData for Rect {
	fn to_kurbo_path(&self, transform: glam::DAffine2, _style: style::PathStyle) -> kurbo::BezPath {
		fn new_point(a: DVec2) -> Point {
			Point::new(a.x, a.y)
		}
		let mut path = kurbo::BezPath::new();
		path.move_to(new_point(transform.translation));

		// TODO: Use into_iter when new impls get added in rust 2021
		[(1., 0.), (1., 1.), (0., 1.)].iter().for_each(|v| path.line_to(new_point(transform.transform_point2((*v).into()))));
		path.close_path();
		path
	}
	fn render(&mut self, svg: &mut String, transform: glam::DAffine2, style: style::PathStyle) {
		let _ = write!(svg, r#"<path d="{}" {} />"#, self.to_kurbo_path(transform, style).to_svg(), style.render());
	}

	fn intersects_quad(&self, quad: [Point; 4], path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>, style: style::PathStyle) {
		if intersect_quad_bez_path(quad, &self.to_kurbo_path(DAffine2::IDENTITY, style)) {
			intersections.push(path.clone());
		}
	}

	fn intersects_point(&self, point: Point, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>, style: style::PathStyle) {
		log::debug!("Intersects point");
		if self.to_kurbo_path(DAffine2::IDENTITY, style).contains(point) {
			intersections.push(path.clone());
		}
	}
}

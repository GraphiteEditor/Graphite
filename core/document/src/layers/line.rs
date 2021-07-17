use glam::DAffine2;
use glam::DVec2;
use kurbo::Point;

use crate::intersection::intersect_quad_bez_path;
use crate::LayerId;

use super::style;
use super::LayerData;

use serde::{Deserialize, Serialize};
use std::fmt::Write;

#[derive(Debug, Default, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub struct Line;

impl LayerData for Line {
	fn to_kurbo_path(&self, transform: glam::DAffine2, _style: style::PathStyle) -> kurbo::BezPath {
		fn new_point(a: DVec2) -> Point {
			Point::new(a.x, a.y)
		}
		let mut path = kurbo::BezPath::new();
		path.move_to(new_point(transform.translation));
		path.line_to(new_point(transform.transform_point2(DVec2::ONE)));
		path
	}

	fn render(&mut self, svg: &mut String, transform: glam::DAffine2, style: style::PathStyle) {
		let [x1, y1] = transform.translation.to_array();
		let [x2, y2] = transform.transform_point2(DVec2::ONE).to_array();

		let _ = write!(svg, r#"<line x1="{}" y1="{}" x2="{}" y2="{}"{} />"#, x1, y1, x2, y2, style.render(),);
	}

	fn intersects_quad(&self, quad: [DVec2; 4], path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>, style: style::PathStyle) {
		if intersect_quad_bez_path(quad, &self.to_kurbo_path(DAffine2::IDENTITY, style), false) {
			intersections.push(path.clone());
		}
	}
}

use glam::DAffine2;
use glam::DVec2;
use kurbo::Shape;

use crate::intersection::intersect_quad_bez_path;
use crate::LayerId;

use super::style;
use super::LayerData;

use serde::{Deserialize, Serialize};
use std::fmt::Write;

#[derive(Debug, Clone, Copy, PartialEq, Default, Deserialize, Serialize)]
pub struct Ellipse {}

impl Ellipse {
	pub fn new() -> Ellipse {
		Ellipse {}
	}
}

impl LayerData for Ellipse {
	fn to_kurbo_path(&self, transform: glam::DAffine2, _style: style::PathStyle) -> kurbo::BezPath {
		kurbo::Ellipse::from_affine(kurbo::Affine::new(transform.to_cols_array())).to_path(0.01)
	}

	fn render(&mut self, svg: &mut String, transform: glam::DAffine2, style: style::PathStyle) {
		let _ = write!(svg, r#"<path d="{}" {} />"#, self.to_kurbo_path(transform, style).to_svg(), style.render());
	}

	fn intersects_quad(&self, quad: [DVec2; 4], path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>, style: style::PathStyle) {
		if intersect_quad_bez_path(quad, &self.to_kurbo_path(DAffine2::IDENTITY, style), true) {
			intersections.push(path.clone());
		}
	}
}

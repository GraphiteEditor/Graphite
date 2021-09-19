use glam::DAffine2;
use glam::DMat2;
use glam::DVec2;

use kurbo::Affine;
use kurbo::Shape as KurboShape;

use crate::intersection::intersect_quad_bez_path;
use crate::LayerId;
use crate::Quad;
use kurbo::BezPath;

use super::style;
use super::style::PathStyle;
use super::LayerData;

use serde::{Deserialize, Serialize};
use std::fmt::Write;

fn glam_to_kurbo(transform: DAffine2) -> Affine {
	Affine::new(transform.to_cols_array())
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Text {
	pub text: String,
	pub style: style::PathStyle,
	pub render_index: i32,
}

impl LayerData for Text {
	fn render(&mut self, svg: &mut String, transforms: &mut Vec<DAffine2>) {
		let transform = self.transform(transforms);
		let inverse = transform.inverse();
		if !inverse.is_finite() {
			let _ = write!(svg, "<!-- SVG shape has an invalid transform -->");
			return;
		}
		let (x, y) = transform.translation.into();

		let _ = svg.write_str(r#")">"#);
		let _ = write!(
			svg,
			r#"<foreignObject width=1000px height=1000px><textarea {} onchange="console.log('Editing');">{}</textarea></foreignObject>"#,
			self.style.render(),
			self.text
		);
	}

	fn bounding_box(&self, transform: glam::DAffine2) -> Option<[DVec2; 2]> {
		Some([transform.transform_point2(DVec2::ZERO), transform.transform_point2(DVec2::new(200., 50.))])
	}

	fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>) {}
}

impl Text {
	pub fn transform(&self, transforms: &[DAffine2]) -> DAffine2 {
		let start = match self.render_index {
			-1 => 0,
			x => (transforms.len() as i32 - x).max(0) as usize,
		};
		transforms.iter().skip(start).cloned().reduce(|a, b| a * b).unwrap_or(DAffine2::IDENTITY)
	}

	pub fn from_string(text: String, style: PathStyle) -> Self {
		Self { style, text, render_index: 1 }
	}
}

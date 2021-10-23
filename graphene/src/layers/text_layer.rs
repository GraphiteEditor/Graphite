use glam::{DAffine2, DMat2, DVec2};
use kurbo::{Affine, BezPath, Rect, Shape};

use crate::intersection::intersect_quad_bez_path;
use crate::LayerId;
use crate::Quad;

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
	pub bezpath: BezPath,
	pub render_index: i32,
	pub size: DVec2,
}

impl LayerData for Text {
	fn render(&mut self, svg: &mut String, transforms: &mut Vec<DAffine2>, path: &mut Vec<LayerId>, text_editable: bool) {
		log::info!("Path {:?} size{:?}", path, self.size);
		let _ = svg.write_str(r#")">"#);
		let size = format!("style=\"width:{}px;height:{}px\"", self.size.x, self.size.y);
		if text_editable {
			let _ = write!(
				svg,
				r#"<foreignObject style="width:1px;height:1px;overflow:visible;"><textarea {} {} data-path='{}'>{}</textarea></foreignObject>"#,
				self.style.render(),
				size,
				path.iter().map(|x| x.to_string()).collect::<Vec<String>>().join(","),
				self.text
			);
		} else {
			let mut path = self.bezpath.clone();
			let transform = self.transform(transforms);
			let inverse = transform.inverse();
			if !inverse.is_finite() {
				let _ = write!(svg, "<!-- SVG shape has an invalid transform -->");
				return;
			}
			path.apply_affine(glam_to_kurbo(transform));

			let _ = writeln!(svg, r#"<g transform="matrix("#);
			inverse.to_cols_array().iter().enumerate().for_each(|(i, entry)| {
				let _ = svg.write_str(&(entry.to_string() + if i != 5 { "," } else { "" }));
			});
			let _ = svg.write_str(r#")">"#);
			let _ = write!(svg, r#"<path d="{}" {} />"#, path.to_svg(), self.style.render());
			let _ = svg.write_str("</g>");
		}
	}

	fn bounding_box(&self, transform: glam::DAffine2) -> Option<[DVec2; 2]> {
		let mut path = self.bezpath.clone();
		if transform.matrix2 == DMat2::ZERO {
			return None;
		}
		path.apply_affine(glam_to_kurbo(transform));

		use kurbo::Shape;
		let kurbo::Rect { x0, y0, x1, y1 } = path.bounding_box();
		Some([(x0, y0).into(), (x1, y1).into()])
	}

	fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>) {
		if intersect_quad_bez_path(quad, &self.bezpath, true) {
			intersections.push(path.clone());
		}
	}
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
		let mut result = Self {
			style,
			text,
			bezpath: BezPath::new(),
			render_index: 1,
			size: DVec2::new(200., 50.),
		};
		result.rerender();
		result
	}
	pub fn rerender(&mut self) {
		let mut font = fonterator::source_font();
		let iter = font.render(&self.text, 13800, -1000);
		self.bezpath = kurbo::BezPath::from_vec(iter.map(|p| p).collect());
		self.bezpath
			.apply_affine(glam_to_kurbo(DAffine2::from_translation(DVec2::new(0., -1.8)) * DAffine2::from_scale(DVec2::splat(17.6))));
	}
}

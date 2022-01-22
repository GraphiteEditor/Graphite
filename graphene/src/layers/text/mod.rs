use super::layer_info::LayerData;
use super::style::{self, PathStyle, ViewMode};
use crate::intersection::{intersect_quad_bez_path, Quad};
use crate::LayerId;

use glam::{DAffine2, DMat2, DVec2};
use kurbo::{Affine, BezPath, Shape};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

mod to_kurbo;

fn glam_to_kurbo(transform: DAffine2) -> Affine {
	Affine::new(transform.to_cols_array())
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Text {
	pub text: String,
	pub style: style::PathStyle,
	pub size: f64,
	pub line_width: f64,
	#[serde(skip)]
	pub editable: bool,
	#[serde(skip)]
	cached_path: BezPath,
}

impl LayerData for Text {
	fn render(&mut self, svg: &mut String, transforms: &mut Vec<DAffine2>, view_mode: ViewMode) {
		let transform = self.transform(transforms, view_mode);
		let inverse = transform.inverse();
		if !inverse.is_finite() {
			let _ = write!(svg, "<!-- SVG shape has an invalid transform -->");
			return;
		}
		let _ = writeln!(svg, r#"<g transform="matrix("#);
		inverse.to_cols_array().iter().enumerate().for_each(|(i, entry)| {
			let _ = svg.write_str(&(entry.to_string() + if i == 5 { "" } else { "," }));
		});
		let _ = svg.write_str(r#")">"#);

		if self.editable {
			let _ = write!(
				svg,
				r#"<foreignObject transform="matrix({})" style="width: {}px; height: 1000px">"#,
				transform
					.to_cols_array()
					.iter()
					.enumerate()
					.map(|(i, entry)| { entry.to_string() + if i == 5 { "" } else { "," } })
					.collect::<String>(),
				self.line_width,
			);
			let _ = write!(svg, r#"<textarea {}>{}</textarea></foreignObject>"#, self.style.render(view_mode), self.text,);
		} else {
			let mut path = self.to_bez_path();

			path.apply_affine(glam_to_kurbo(transform));

			let _ = write!(svg, r#"<path d="{}" {} />"#, path.to_svg(), self.style.render(view_mode));
		}
		let _ = svg.write_str("</g>");
	}

	fn bounding_box(&self, transform: glam::DAffine2) -> Option<[DVec2; 2]> {
		let mut path = self.to_bez_path();

		if transform.matrix2 == DMat2::ZERO {
			return None;
		}
		path.apply_affine(glam_to_kurbo(transform));

		let kurbo::Rect { x0, y0, x1, y1 } = path.bounding_box();
		Some([(x0, y0).into(), (x1, y1).into()])
	}

	fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>) {
		if intersect_quad_bez_path(quad, &self.to_bez_path().bounding_box().to_path(0.), true) {
			intersections.push(path.clone());
		}
	}
}

impl Text {
	pub fn transform(&self, transforms: &[DAffine2], mode: ViewMode) -> DAffine2 {
		let start = match mode {
			ViewMode::Outline => 0,
			_ => (transforms.len() as i32 - 1).max(0) as usize,
		};
		transforms.iter().skip(start).cloned().reduce(|a, b| a * b).unwrap_or(DAffine2::IDENTITY)
	}

	pub fn new(text: String, style: PathStyle, size: f64) -> Self {
		let mut new = Self {
			text,
			style,
			size,
			line_width: 300.,
			editable: false,
			cached_path: BezPath::new(),
		};

		new.regenerate_path();

		new
	}

	#[inline]
	pub fn to_bez_path(&self) -> BezPath {
		self.cached_path.clone()
	}

	pub fn regenerate_path(&mut self) {
		let buzz_face = rustybuzz::Face::from_slice(include_bytes!("SourceSansPro/SourceSansPro-Regular.ttf"), 0).unwrap();
		self.cached_path = to_kurbo::to_kurbo(&self.text, buzz_face, self.size, self.line_width);
	}

	pub fn update_text(&mut self, text: String) {
		self.text = text;
		self.regenerate_path();
	}
}

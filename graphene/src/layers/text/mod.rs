use super::layer_info::LayerData;
use super::style::{self, PathStyle, ViewMode};
use crate::intersection::{intersect_quad_bez_path, Quad};
use crate::LayerId;

use glam::{DAffine2, DMat2, DVec2};
use kurbo::{Affine, BezPath, Rect, Shape};
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
	pub line_width: Option<f64>,
	#[serde(skip)]
	pub editable: bool,
	#[serde(skip)]
	cached_path: Option<BezPath>,
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
				r#"<foreignObject transform="matrix({})" style="color: {}"></foreignObject>"#,
				transform
					.to_cols_array()
					.iter()
					.enumerate()
					.map(|(i, entry)| { entry.to_string() + if i == 5 { "" } else { "," } })
					.collect::<String>(),
				match self.style.fill() {
					Some(fill) => format!("#{}", fill.color().rgba_hex()),
					None => "#00000000".to_string(),
				}
			);
		} else {
			let mut path = self.to_bez_path();

			path.apply_affine(glam_to_kurbo(transform));

			let _ = write!(svg, r#"<path d="{}" {} />"#, path.to_svg(), self.style.render(view_mode));
		}
		let _ = svg.write_str("</g>");
	}

	fn bounding_box(&self, transform: glam::DAffine2) -> Option<[DVec2; 2]> {
		let mut path = self.bounding_box(&self.text).to_path(0.1);

		if transform.matrix2 == DMat2::ZERO {
			return None;
		}
		path.apply_affine(glam_to_kurbo(transform));

		let kurbo::Rect { x0, y0, x1, y1 } = path.bounding_box();
		Some([(x0, y0).into(), (x1, y1).into()])
	}

	fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>) {
		if intersect_quad_bez_path(quad, &self.bounding_box(&self.text).to_path(0.), true) {
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
			line_width: None,
			editable: false,
			cached_path: None,
		};

		new.regenerate_path();

		new
	}

	/// Converts to a BezPath, populating the cache if necessary
	#[inline]
	pub fn to_bez_path(&mut self) -> BezPath {
		if self.cached_path.is_none() {
			self.regenerate_path();
		}
		self.cached_path.clone().unwrap()
	}

	/// Converts to a bezpath, without populating the cache
	#[inline]
	pub fn to_bez_path_nonmut(&self) -> BezPath {
		self.cached_path.clone().unwrap_or_else(|| self.generate_path())
	}

	#[inline]
	fn font_face() -> rustybuzz::Face<'static> {
		rustybuzz::Face::from_slice(include_bytes!("SourceSansPro/SourceSansPro-Regular.ttf"), 0).unwrap()
	}

	#[inline]
	fn generate_path(&self) -> BezPath {
		to_kurbo::to_kurbo(&self.text, Self::font_face(), self.size, self.line_width)
	}

	#[inline]
	pub fn bounding_box(&self, text: &str) -> Rect {
		let far = to_kurbo::bounding_box(text, Self::font_face(), self.size, self.line_width);
		Rect::new(0., 0., far.x, far.y)
	}

	pub fn regenerate_path(&mut self) {
		self.cached_path = Some(self.generate_path());
	}

	pub fn update_text(&mut self, text: String) {
		self.text = text;
		self.regenerate_path();
	}
}

use super::layer_info::LayerData;
use super::style::{PathStyle, ViewMode};
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

/// A line, or multiple lines, of text drawn in the document.
/// Like [ShapeLayers](super::shape_layer::ShapeLayer), [TextLayer] are rendered as
/// [`<path>`s](https://developer.mozilla.org/en-US/docs/Web/SVG/Element/path).
/// Currently, the only supported font is `SourceSansPro-Regular`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct TextLayer {
	/// The string of text, encompassing one or multiple lines.
	pub text: String,
	/// Fill color and stroke used to render the text.
	pub style: PathStyle,
	/// Font size in pixels.
	pub size: f64,
	pub line_width: Option<f64>,
	#[serde(skip)]
	pub editable: bool,
	#[serde(skip)]
	cached_path: Option<BezPath>,
}

impl LayerData for TextLayer {
	fn render(&mut self, svg: &mut String, svg_defs: &mut String, transforms: &mut Vec<DAffine2>, view_mode: ViewMode) {
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
				r#"<foreignObject transform="matrix({})"></foreignObject>"#,
				transform
					.to_cols_array()
					.iter()
					.enumerate()
					.map(|(i, entry)| { entry.to_string() + if i == 5 { "" } else { "," } })
					.collect::<String>(),
			);
		} else {
			let mut path = self.to_bez_path();

			let kurbo::Rect { x0, y0, x1, y1 } = path.bounding_box();
			let bounds = [(x0, y0).into(), (x1, y1).into()];
			let transformed_bounds = (self as &mut dyn LayerData).bounding_box(*transforms.last().unwrap()).unwrap_or([DVec2::ZERO, DVec2::ONE]);

			path.apply_affine(glam_to_kurbo(transform));

			let _ = write!(
				svg,
				r#"<path d="{}" {} />"#,
				path.to_svg(),
				self.style.render(view_mode, svg_defs, transforms, bounds, transformed_bounds)
			);
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

impl TextLayer {
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

	/// Converts to a [BezPath], populating the cache if necessary.
	#[inline]
	pub fn to_bez_path(&mut self) -> BezPath {
		if self.cached_path.is_none() {
			self.regenerate_path();
		}
		self.cached_path.clone().unwrap()
	}

	/// Converts to a [BezPath], without populating the cache.
	#[inline]
	pub fn to_bez_path_nonmut(&self) -> BezPath {
		self.cached_path.clone().unwrap_or_else(|| self.generate_path())
	}

	/// Get the font face for `SourceSansPro-Regular`.
	/// For now, the font is hardcoded in the wasm binary.
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

	/// Populate the cache.
	pub fn regenerate_path(&mut self) {
		self.cached_path = Some(self.generate_path());
	}

	pub fn update_text(&mut self, text: String) {
		self.text = text;
		self.regenerate_path();
	}
}

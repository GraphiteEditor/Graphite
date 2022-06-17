use super::layer_info::LayerData;
use super::style::{PathStyle, ViewMode};
use crate::intersection::{intersect_quad_bez_path, Quad};
use crate::LayerId;
pub use font_cache::{Font, FontCache};

use glam::{DAffine2, DMat2, DVec2};
use kurbo::{Affine, BezPath, Rect, Shape};
use rustybuzz::Face;
use serde::{Deserialize, Serialize};
use std::fmt::Write;

mod font_cache;
mod to_kurbo;

fn glam_to_kurbo(transform: DAffine2) -> Affine {
	Affine::new(transform.to_cols_array())
}

/// A line, or multiple lines, of text drawn in the document.
/// Like [ShapeLayers](super::shape_layer::ShapeLayer), [TextLayer] are rendered as
/// [`<path>`s](https://developer.mozilla.org/en-US/docs/Web/SVG/Element/path).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct TextLayer {
	/// The string of text, encompassing one or multiple lines.
	pub text: String,
	/// Fill color and stroke used to render the text.
	pub path_style: PathStyle,
	/// Font size in pixels.
	pub size: f64,
	pub line_width: Option<f64>,
	pub font: Font,
	#[serde(skip)]
	pub editable: bool,
	#[serde(skip)]
	cached_path: Option<BezPath>,
}

impl LayerData for TextLayer {
	fn render(&mut self, svg: &mut String, svg_defs: &mut String, transforms: &mut Vec<DAffine2>, view_mode: ViewMode, font_cache: &FontCache, _culling_bounds: Option<[DVec2; 2]>) {
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
			let font = font_cache.resolve_font(&self.font);
			if let Some(url) = font.and_then(|font| font_cache.get_preview_url(font)) {
				let _ = write!(svg, r#"<style>@font-face {{font-family: local-font;src: url({});}}")</style>"#, url);
			}

			let _ = write!(
				svg,
				r#"<foreignObject transform="matrix({})"{}></foreignObject>"#,
				transform
					.to_cols_array()
					.iter()
					.enumerate()
					.map(|(i, entry)| { entry.to_string() + if i == 5 { "" } else { "," } })
					.collect::<String>(),
				font.map(|_| r#" style="font-family: local-font;""#).unwrap_or_default()
			);
		} else {
			let buzz_face = self.load_face(font_cache);

			let mut path = self.to_bez_path(buzz_face);

			let kurbo::Rect { x0, y0, x1, y1 } = path.bounding_box();
			let bounds = [(x0, y0).into(), (x1, y1).into()];

			path.apply_affine(glam_to_kurbo(transform));

			let kurbo::Rect { x0, y0, x1, y1 } = path.bounding_box();
			let transformed_bounds = [(x0, y0).into(), (x1, y1).into()];

			let _ = write!(
				svg,
				r#"<path d="{}" {} />"#,
				path.to_svg(),
				self.path_style.render(view_mode, svg_defs, transform, bounds, transformed_bounds)
			);
		}
		let _ = svg.write_str("</g>");
	}

	fn bounding_box(&self, transform: glam::DAffine2, font_cache: &FontCache) -> Option<[DVec2; 2]> {
		let buzz_face = Some(self.load_face(font_cache)?);

		let mut path = self.bounding_box(&self.text, buzz_face).to_path(0.1);

		if transform.matrix2 == DMat2::ZERO {
			return None;
		}
		path.apply_affine(glam_to_kurbo(transform));

		let kurbo::Rect { x0, y0, x1, y1 } = path.bounding_box();
		Some([(x0, y0).into(), (x1, y1).into()])
	}

	fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>, font_cache: &FontCache) {
		let buzz_face = self.load_face(font_cache);

		if intersect_quad_bez_path(quad, &self.bounding_box(&self.text, buzz_face).to_path(0.), true) {
			intersections.push(path.clone());
		}
	}
}

impl TextLayer {
	pub fn load_face<'a>(&self, font_cache: &'a FontCache) -> Option<Face<'a>> {
		font_cache.get(&self.font).map(|data| rustybuzz::Face::from_slice(data, 0).expect("Loading font failed"))
	}

	pub fn transform(&self, transforms: &[DAffine2], mode: ViewMode) -> DAffine2 {
		let start = match mode {
			ViewMode::Outline => 0,
			_ => (transforms.len() as i32 - 1).max(0) as usize,
		};
		transforms.iter().skip(start).cloned().reduce(|a, b| a * b).unwrap_or(DAffine2::IDENTITY)
	}

	pub fn new(text: String, style: PathStyle, size: f64, font: Font, font_cache: &FontCache) -> Self {
		let mut new = Self {
			text,
			path_style: style,
			size,
			line_width: None,
			font,
			editable: false,
			cached_path: None,
		};

		new.regenerate_path(new.load_face(font_cache));

		new
	}

	/// Converts to a [BezPath], populating the cache if necessary.
	#[inline]
	pub fn to_bez_path(&mut self, buzz_face: Option<Face>) -> BezPath {
		if self.cached_path.is_none() {
			self.regenerate_path(buzz_face);
		}
		self.cached_path.clone().unwrap()
	}

	/// Converts to a [BezPath], without populating the cache.
	#[inline]
	pub fn to_bez_path_nonmut(&self, font_cache: &FontCache) -> BezPath {
		let buzz_face = self.load_face(font_cache);

		self.cached_path.clone().unwrap_or_else(|| self.generate_path(buzz_face))
	}

	#[inline]
	fn generate_path(&self, buzz_face: Option<Face>) -> BezPath {
		to_kurbo::to_kurbo(&self.text, buzz_face, self.size, self.line_width)
	}

	#[inline]
	pub fn bounding_box(&self, text: &str, buzz_face: Option<Face>) -> Rect {
		let far = to_kurbo::bounding_box(text, buzz_face, self.size, self.line_width);
		Rect::new(0., 0., far.x, far.y)
	}

	/// Populate the cache.
	pub fn regenerate_path(&mut self, buzz_face: Option<Face>) {
		self.cached_path = Some(self.generate_path(buzz_face));
	}

	pub fn update_text(&mut self, text: String, font_cache: &FontCache) {
		let buzz_face = self.load_face(font_cache);

		self.text = text;
		self.regenerate_path(buzz_face);
	}
}

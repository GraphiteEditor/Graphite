use super::layer_info::LayerData;
use super::style::{PathStyle, RenderData, ViewMode};
use super::vector::subpath::Subpath;
use crate::intersection::{intersect_quad_bez_path, Quad};
use crate::LayerId;
pub use font_cache::{Font, FontCache};

use glam::{DAffine2, DMat2, DVec2};
use rustybuzz::Face;
use serde::{Deserialize, Serialize};
use std::fmt::Write;

mod font_cache;
mod to_path;

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
	pub cached_path: Option<Subpath>,
}

impl LayerData for TextLayer {
	fn render(&mut self, svg: &mut String, svg_defs: &mut String, transforms: &mut Vec<DAffine2>, render_data: RenderData) {
		let transform = self.transform(transforms, render_data.view_mode);
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
			let font = render_data.font_cache.resolve_font(&self.font);
			if let Some(url) = font.and_then(|font| render_data.font_cache.get_preview_url(font)) {
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
			let buzz_face = self.load_face(render_data.font_cache);

			let mut path = self.to_vector_path(buzz_face);

			let kurbo::Rect { x0, y0, x1, y1 } = path.bounding_box();
			let bounds = [(x0, y0).into(), (x1, y1).into()];

			path.apply_affine(transform);

			let kurbo::Rect { x0, y0, x1, y1 } = path.bounding_box();
			let transformed_bounds = [(x0, y0).into(), (x1, y1).into()];

			let _ = write!(
				svg,
				r#"<path d="{}" {} />"#,
				path.to_svg(),
				self.path_style.render(render_data.view_mode, svg_defs, transform, bounds, transformed_bounds)
			);
		}
		let _ = svg.write_str("</g>");
	}

	fn bounding_box(&self, transform: glam::DAffine2, font_cache: &FontCache) -> Option<[DVec2; 2]> {
		let buzz_face = Some(self.load_face(font_cache)?);

		if transform.matrix2 == DMat2::ZERO {
			return None;
		}

		Some((transform * self.bounding_box(&self.text, buzz_face)).bounding_box())
	}

	fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>, font_cache: &FontCache) {
		let buzz_face = self.load_face(font_cache);

		if intersect_quad_bez_path(quad, &self.bounding_box(&self.text, buzz_face).path(), true) {
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

		new.cached_path = Some(new.generate_path(new.load_face(font_cache)));

		new
	}

	/// Converts to a [VectorShape], populating the cache if necessary.
	#[inline]
	pub fn to_vector_path(&mut self, buzz_face: Option<Face>) -> Subpath {
		if self.cached_path.as_ref().filter(|x| !x.groups().is_empty()).is_none() {
			let path = self.generate_path(buzz_face);
			self.cached_path = Some(path.clone());
			return path;
		}
		self.cached_path.clone().unwrap()
	}

	/// Converts to a [VectorShape], without populating the cache.
	#[inline]
	pub fn to_subpath_nonmut(&self, font_cache: &FontCache) -> Subpath {
		let buzz_face = self.load_face(font_cache);

		self.cached_path.clone().filter(|x| !x.groups().is_empty()).unwrap_or_else(|| self.generate_path(buzz_face))
	}

	#[inline]
	pub fn generate_path(&self, buzz_face: Option<Face>) -> Subpath {
		to_path::to_path(&self.text, buzz_face, self.size, self.line_width)
	}

	#[inline]
	pub fn bounding_box(&self, text: &str, buzz_face: Option<Face>) -> Quad {
		let far = to_path::bounding_box(text, buzz_face, self.size, self.line_width);
		Quad::from_box([DVec2::ZERO, far])
	}

	pub fn update_text(&mut self, text: String, font_cache: &FontCache) {
		let buzz_face = self.load_face(font_cache);

		self.text = text;
		self.cached_path = Some(self.generate_path(buzz_face));
	}
}

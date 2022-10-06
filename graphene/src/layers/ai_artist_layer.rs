use super::layer_info::LayerData;
use super::style::{RenderData, ViewMode};
use crate::intersection::{intersect_quad_bez_path, Quad};
use crate::layers::text_layer::FontCache;
use crate::LayerId;

use glam::{DAffine2, DMat2, DVec2};
use kurbo::{Affine, BezPath, Shape as KurboShape};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct AiArtistLayer {
	#[serde(skip)]
	pub dimensions: DVec2,
}

impl LayerData for AiArtistLayer {
	fn render(&mut self, svg: &mut String, _svg_defs: &mut String, transforms: &mut Vec<DAffine2>, render_data: RenderData) {
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

		let svg_transform = transform
			.to_cols_array()
			.iter()
			.enumerate()
			.map(|(i, entry)| entry.to_string() + if i == 5 { "" } else { "," })
			.collect::<String>();
		let _ = write!(
			svg,
			r#"<image width="{}" height="{}" transform="matrix({})" href=""#,
			self.dimensions.x, self.dimensions.y, svg_transform,
		);

		let _ = write!(
			svg,
			"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAAXNSR0IArs4c6QAAAA1JREFUGFdjYLtZ/h8ABJUCVn7cQO8AAAAASUVORK5CYII="
		);

		let _ = svg.write_str(r#""/>"#);
		let _ = svg.write_str("</g>");
	}

	fn bounding_box(&self, transform: glam::DAffine2, _font_cache: &FontCache) -> Option<[DVec2; 2]> {
		let mut path = self.bounds();

		if transform.matrix2 == DMat2::ZERO {
			return None;
		}
		path.apply_affine(glam_to_kurbo(transform));

		let kurbo::Rect { x0, y0, x1, y1 } = path.bounding_box();
		Some([(x0, y0).into(), (x1, y1).into()])
	}

	fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>, _font_cache: &FontCache) {
		if intersect_quad_bez_path(quad, &self.bounds(), true) {
			intersections.push(path.clone());
		}
	}
}

impl AiArtistLayer {
	pub fn new() -> Self {
		let dimensions = DVec2::ONE;
		Self { dimensions }
	}

	pub fn transform(&self, transforms: &[DAffine2], mode: ViewMode) -> DAffine2 {
		let start = match mode {
			ViewMode::Outline => 0,
			_ => (transforms.len() as i32 - 1).max(0) as usize,
		};
		transforms.iter().skip(start).cloned().reduce(|a, b| a * b).unwrap_or(DAffine2::IDENTITY)
	}

	fn bounds(&self) -> BezPath {
		kurbo::Rect::from_origin_size(kurbo::Point::ZERO, kurbo::Size::new(self.dimensions.x, self.dimensions.y)).to_path(0.)
	}
}

impl Default for AiArtistLayer {
	fn default() -> Self {
		Self::new()
	}
}

fn glam_to_kurbo(transform: DAffine2) -> Affine {
	Affine::new(transform.to_cols_array())
}

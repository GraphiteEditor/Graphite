use super::base64_serde;
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
	// User-configurable layer parameters
	pub seed: u64,
	pub samples: u32,
	pub use_img2img: bool,
	pub denoising_strength: f64,
	pub cfg_scale: f64,
	pub prompt: String,
	pub negative_prompt: String,
	pub restore_faces: bool,
	pub tiling: bool,

	// Image stored in layer after generation completes
	pub image_data: Option<ImageData>,
	pub mime: String,
	/// 0 is not started, 100 is complete.
	pub percent_complete: f64,

	// TODO: Have the browser dispose of this blob URL when this is dropped (like when the layer is deleted)
	#[serde(skip)]
	pub blob_url: Option<String>,
	#[serde(skip)]
	pub status: AiArtistStatus,
	#[serde(skip)]
	pub dimensions: DVec2,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum AiArtistStatus {
	#[default]
	Idle,
	Beginning,
	Uploading(f64),
	Generating,
	Terminating,
	Terminated,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub struct ImageData {
	#[serde(serialize_with = "base64_serde::as_base64", deserialize_with = "base64_serde::from_base64")]
	pub image_data: Vec<u8>,
}

impl Default for AiArtistLayer {
	fn default() -> Self {
		Self {
			seed: 0,
			samples: 30,
			use_img2img: false,
			denoising_strength: 0.66,
			cfg_scale: 10.,
			prompt: "".into(),
			negative_prompt: "".into(),
			restore_faces: false,
			tiling: false,

			image_data: None,
			mime: "image/png".into(),

			blob_url: None,
			percent_complete: 0.,
			status: Default::default(),
			dimensions: Default::default(),
		}
	}
}

impl LayerData for AiArtistLayer {
	fn render(&mut self, svg: &mut String, _svg_defs: &mut String, transforms: &mut Vec<DAffine2>, render_data: RenderData) {
		let transform = self.transform(transforms, render_data.view_mode);
		let inverse = transform.inverse();

		let matrix_values = transform.matrix2.to_cols_array();
		let (width, height) = (matrix_values[0], matrix_values[3]);

		if !inverse.is_finite() {
			let _ = write!(svg, "<!-- SVG shape has an invalid transform -->");
			return;
		}

		let _ = writeln!(svg, r#"<g transform="matrix("#);
		inverse.to_cols_array().iter().enumerate().for_each(|(i, entry)| {
			let _ = svg.write_str(&(entry.to_string() + if i == 5 { "" } else { "," }));
		});
		let _ = svg.write_str(r#")">"#);

		if let Some(blob_url) = &self.blob_url {
			let _ = write!(
				svg,
				r#"<image width="{}" height="{}" transform="matrix(1,0,0,1,{},{})" preserveAspectRatio="none" href="{}"/>"#,
				width.abs(),
				height.abs(),
				if width >= 0. { transform.translation.x } else { transform.translation.x + width },
				if height >= 0. { transform.translation.y } else { transform.translation.y + height },
				blob_url
			);
		} else {
			let _ = write!(
				svg,
				r#"<rect width="{}" height="{}" transform="matrix(1,0,0,1,{},{})" fill="none" stroke="var(--color-data-raster)" stroke-width="3" stroke-dasharray="8"/>"#,
				width.abs(),
				height.abs(),
				if width >= 0. { transform.translation.x } else { transform.translation.x + width },
				if height >= 0. { transform.translation.y } else { transform.translation.y + height },
			);
		}

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
	pub fn transform(&self, transforms: &[DAffine2], mode: ViewMode) -> DAffine2 {
		let start = match mode {
			ViewMode::Outline => 0,
			_ => (transforms.len() as i32 - 1).max(0) as usize,
		};
		transforms.iter().skip(start).cloned().reduce(|a, b| a * b).unwrap_or(DAffine2::IDENTITY)
	}

	fn bounds(&self) -> BezPath {
		kurbo::Rect::from_origin_size(kurbo::Point::ZERO, kurbo::Size::new(1., 1.)).to_path(0.)
	}
}

fn glam_to_kurbo(transform: DAffine2) -> Affine {
	Affine::new(transform.to_cols_array())
}

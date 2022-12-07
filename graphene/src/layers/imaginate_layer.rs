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

#[derive(Clone, PartialEq, Deserialize, Serialize)]
pub struct ImaginateLayer {
	// User-configurable layer parameters
	pub seed: u64,
	pub samples: u32,
	pub sampling_method: ImaginateSamplingMethod,
	pub use_img2img: bool,
	pub denoising_strength: f64,
	pub mask_layer_ref: Option<Vec<LayerId>>,
	pub mask_paint_mode: ImaginateMaskPaintMode,
	pub mask_blur_px: u32,
	pub mask_fill_content: ImaginateMaskFillContent,
	pub cfg_scale: f64,
	pub prompt: String,
	pub negative_prompt: String,
	pub restore_faces: bool,
	pub tiling: bool,

	// Image stored in layer after generation completes
	pub image_data: Option<ImaginateImageData>,
	pub mime: String,
	/// 0 is not started, 100 is complete.
	pub percent_complete: f64,

	// TODO: Have the browser dispose of this blob URL when this is dropped (like when the layer is deleted)
	#[serde(skip)]
	pub blob_url: Option<String>,
	#[serde(skip)]
	pub status: ImaginateStatus,
	#[serde(skip)]
	pub dimensions: DVec2,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum ImaginateStatus {
	#[default]
	Idle,
	Beginning,
	Uploading(f64),
	Generating,
	Terminating,
	Terminated,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub struct ImaginateImageData {
	#[serde(serialize_with = "base64_serde::as_base64", deserialize_with = "base64_serde::from_base64")]
	pub image_data: std::sync::Arc<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ImaginateBaseImage {
	pub svg: String,
	pub size: DVec2,
}

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Deserialize, Serialize)]
pub enum ImaginateMaskPaintMode {
	#[default]
	Inpaint,
	Outpaint,
}

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Deserialize, Serialize)]
pub enum ImaginateMaskFillContent {
	#[default]
	Fill,
	Original,
	LatentNoise,
	LatentNothing,
}

impl ImaginateMaskFillContent {
	pub fn list() -> [ImaginateMaskFillContent; 4] {
		[
			ImaginateMaskFillContent::Fill,
			ImaginateMaskFillContent::Original,
			ImaginateMaskFillContent::LatentNoise,
			ImaginateMaskFillContent::LatentNothing,
		]
	}
}

impl std::fmt::Display for ImaginateMaskFillContent {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			ImaginateMaskFillContent::Fill => write!(f, "Smeared Surroundings"),
			ImaginateMaskFillContent::Original => write!(f, "Original Base Image"),
			ImaginateMaskFillContent::LatentNoise => write!(f, "Randomness (Latent Noise)"),
			ImaginateMaskFillContent::LatentNothing => write!(f, "Neutral (Latent Nothing)"),
		}
	}
}

#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub enum ImaginateSamplingMethod {
	#[default]
	EulerA,
	Euler,
	LMS,
	Heun,
	DPM2,
	DPM2A,
	DPMPlusPlus2sA,
	DPMPlusPlus2m,
	DPMFast,
	DPMAdaptive,
	LMSKarras,
	DPM2Karras,
	DPM2AKarras,
	DPMPlusPlus2sAKarras,
	DPMPlusPlus2mKarras,
	DDIM,
	PLMS,
}

impl ImaginateSamplingMethod {
	pub fn api_value(&self) -> &str {
		match self {
			ImaginateSamplingMethod::EulerA => "Euler a",
			ImaginateSamplingMethod::Euler => "Euler",
			ImaginateSamplingMethod::LMS => "LMS",
			ImaginateSamplingMethod::Heun => "Heun",
			ImaginateSamplingMethod::DPM2 => "DPM2",
			ImaginateSamplingMethod::DPM2A => "DPM2 a",
			ImaginateSamplingMethod::DPMPlusPlus2sA => "DPM++ 2S a",
			ImaginateSamplingMethod::DPMPlusPlus2m => "DPM++ 2M",
			ImaginateSamplingMethod::DPMFast => "DPM fast",
			ImaginateSamplingMethod::DPMAdaptive => "DPM adaptive",
			ImaginateSamplingMethod::LMSKarras => "LMS Karras",
			ImaginateSamplingMethod::DPM2Karras => "DPM2 Karras",
			ImaginateSamplingMethod::DPM2AKarras => "DPM2 a Karras",
			ImaginateSamplingMethod::DPMPlusPlus2sAKarras => "DPM++ 2S a Karras",
			ImaginateSamplingMethod::DPMPlusPlus2mKarras => "DPM++ 2M Karras",
			ImaginateSamplingMethod::DDIM => "DDIM",
			ImaginateSamplingMethod::PLMS => "PLMS",
		}
	}

	pub fn list() -> [ImaginateSamplingMethod; 17] {
		[
			ImaginateSamplingMethod::EulerA,
			ImaginateSamplingMethod::Euler,
			ImaginateSamplingMethod::LMS,
			ImaginateSamplingMethod::Heun,
			ImaginateSamplingMethod::DPM2,
			ImaginateSamplingMethod::DPM2A,
			ImaginateSamplingMethod::DPMPlusPlus2sA,
			ImaginateSamplingMethod::DPMPlusPlus2m,
			ImaginateSamplingMethod::DPMFast,
			ImaginateSamplingMethod::DPMAdaptive,
			ImaginateSamplingMethod::LMSKarras,
			ImaginateSamplingMethod::DPM2Karras,
			ImaginateSamplingMethod::DPM2AKarras,
			ImaginateSamplingMethod::DPMPlusPlus2sAKarras,
			ImaginateSamplingMethod::DPMPlusPlus2mKarras,
			ImaginateSamplingMethod::DDIM,
			ImaginateSamplingMethod::PLMS,
		]
	}
}

impl std::fmt::Display for ImaginateSamplingMethod {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			ImaginateSamplingMethod::EulerA => write!(f, "Euler A (Recommended)"),
			ImaginateSamplingMethod::Euler => write!(f, "Euler"),
			ImaginateSamplingMethod::LMS => write!(f, "LMS"),
			ImaginateSamplingMethod::Heun => write!(f, "Heun"),
			ImaginateSamplingMethod::DPM2 => write!(f, "DPM2"),
			ImaginateSamplingMethod::DPM2A => write!(f, "DPM2 A"),
			ImaginateSamplingMethod::DPMPlusPlus2sA => write!(f, "DPM++ 2S a"),
			ImaginateSamplingMethod::DPMPlusPlus2m => write!(f, "DPM++ 2M"),
			ImaginateSamplingMethod::DPMFast => write!(f, "DPM Fast"),
			ImaginateSamplingMethod::DPMAdaptive => write!(f, "DPM Adaptive"),
			ImaginateSamplingMethod::LMSKarras => write!(f, "LMS Karras"),
			ImaginateSamplingMethod::DPM2Karras => write!(f, "DPM2 Karras"),
			ImaginateSamplingMethod::DPM2AKarras => write!(f, "DPM2 A Karras"),
			ImaginateSamplingMethod::DPMPlusPlus2sAKarras => write!(f, "DPM++ 2S a Karras"),
			ImaginateSamplingMethod::DPMPlusPlus2mKarras => write!(f, "DPM++ 2M Karras"),
			ImaginateSamplingMethod::DDIM => write!(f, "DDIM"),
			ImaginateSamplingMethod::PLMS => write!(f, "PLMS"),
		}
	}
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ImaginateGenerationParameters {
	pub seed: u64,
	pub samples: u32,
	/// Use `ImaginateSamplingMethod::api_value()` to generate this string
	#[serde(rename = "samplingMethod")]
	pub sampling_method: String,
	#[serde(rename = "denoisingStrength")]
	pub denoising_strength: Option<f64>,
	#[serde(rename = "cfgScale")]
	pub cfg_scale: f64,
	pub prompt: String,
	#[serde(rename = "negativePrompt")]
	pub negative_prompt: String,
	pub resolution: (u64, u64),
	#[serde(rename = "restoreFaces")]
	pub restore_faces: bool,
	pub tiling: bool,
}

impl Default for ImaginateLayer {
	fn default() -> Self {
		Self {
			seed: 0,
			samples: 30,
			sampling_method: Default::default(),
			use_img2img: false,
			denoising_strength: 0.66,
			mask_paint_mode: ImaginateMaskPaintMode::default(),
			mask_layer_ref: None,
			mask_blur_px: 4,
			mask_fill_content: ImaginateMaskFillContent::default(),
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

impl LayerData for ImaginateLayer {
	fn render(&mut self, svg: &mut String, _svg_defs: &mut String, transforms: &mut Vec<DAffine2>, render_data: RenderData) {
		let transform = self.transform(transforms, render_data.view_mode);
		let inverse = transform.inverse();

		let (width, height) = (transform.transform_vector2(DVec2::new(1., 0.)).length(), transform.transform_vector2(DVec2::new(0., 1.)).length());

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
				r#"<image width="{}" height="{}" preserveAspectRatio="none" href="{}" transform="matrix("#,
				width.abs(),
				height.abs(),
				blob_url
			);
		} else {
			let _ = write!(
				svg,
				r#"<rect width="{}" height="{}" fill="none" stroke="var(--color-data-raster)" stroke-width="3" stroke-dasharray="8" transform="matrix("#,
				width.abs(),
				height.abs(),
			);
		}

		(transform * DAffine2::from_scale((width, height).into()).inverse())
			.to_cols_array()
			.iter()
			.enumerate()
			.for_each(|(i, entry)| {
				let _ = svg.write_str(&(entry.to_string() + if i == 5 { "" } else { "," }));
			});

		let _ = svg.write_str(r#")" /> </g>"#);
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

impl ImaginateLayer {
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

impl std::fmt::Debug for ImaginateLayer {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ImaginateLayer")
			.field("seed", &self.seed)
			.field("samples", &self.samples)
			.field("use_img2img", &self.use_img2img)
			.field("denoising_strength", &self.denoising_strength)
			.field("cfg_scale", &self.cfg_scale)
			.field("prompt", &self.prompt)
			.field("negative_prompt", &self.negative_prompt)
			.field("restore_faces", &self.restore_faces)
			.field("tiling", &self.tiling)
			.field("image_data", &self.image_data.as_ref().map(|_| "..."))
			.field("mime", &self.mime)
			.field("percent_complete", &self.percent_complete)
			.field("blob_url", &self.blob_url)
			.field("status", &self.status)
			.field("dimensions", &self.dimensions)
			.finish()
	}
}

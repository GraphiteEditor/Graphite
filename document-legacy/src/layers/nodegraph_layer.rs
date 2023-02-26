use super::base64_serde;
use super::layer_info::LayerData;
use super::style::{RenderData, ViewMode};
use crate::intersection::{intersect_quad_bez_path, Quad};
use crate::LayerId;

use glam::{DAffine2, DMat2, DVec2};
use kurbo::{Affine, BezPath, Shape as KurboShape};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct NodeGraphFrameLayer {
	// Image stored in layer after generation completes
	pub mime: String,

	/// The document node network that this layer contains
	pub network: graph_craft::document::NodeNetwork,

	// TODO: Have the browser dispose of this blob URL when this is dropped (like when the layer is deleted)
	#[serde(skip)]
	pub blob_url: Option<String>,
	#[serde(skip)]
	pub dimensions: DVec2,
	pub image_data: Option<ImageData>,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize, specta::Type)]
pub struct ImageData {
	#[serde(serialize_with = "base64_serde::as_base64", deserialize_with = "base64_serde::from_base64")]
	#[specta(type = String)]
	pub image_data: std::sync::Arc<Vec<u8>>,
}

impl LayerData for NodeGraphFrameLayer {
	fn render(&mut self, svg: &mut String, _svg_defs: &mut String, transforms: &mut Vec<DAffine2>, render_data: &RenderData) -> bool {
		let transform = self.transform(transforms, render_data.view_mode);
		let inverse = transform.inverse();

		let (width, height) = (transform.transform_vector2(DVec2::new(1., 0.)).length(), transform.transform_vector2(DVec2::new(0., 1.)).length());

		if !inverse.is_finite() {
			let _ = write!(svg, "<!-- SVG shape has an invalid transform -->");
			return false;
		}

		let _ = writeln!(svg, r#"<g transform="matrix("#);
		inverse.to_cols_array().iter().enumerate().for_each(|(i, entry)| {
			let _ = svg.write_str(&(entry.to_string() + if i == 5 { "" } else { "," }));
		});
		let _ = svg.write_str(r#")">"#);

		let matrix = (transform * DAffine2::from_scale((width, height).into()).inverse())
			.to_cols_array()
			.iter()
			.enumerate()
			.fold(String::new(), |val, (i, entry)| val + &(entry.to_string() + if i == 5 { "" } else { "," }));

		if let Some(blob_url) = &self.blob_url {
			let _ = write!(
				svg,
				r#"<image width="{}" height="{}" preserveAspectRatio="none" href="{}" transform="matrix({})" />"#,
				width.abs(),
				height.abs(),
				blob_url,
				matrix
			);
		} else {
			let _ = write!(
				svg,
				r#"<rect width="{}" height="{}" fill="none" stroke="var(--color-data-vector)" stroke-width="3" stroke-dasharray="8" transform="matrix({})" />"#,
				width.abs(),
				height.abs(),
				matrix,
			);
		}

		let _ = svg.write_str(r#"</g>"#);

		false
	}

	fn bounding_box(&self, transform: glam::DAffine2, _render_data: &RenderData) -> Option<[DVec2; 2]> {
		let mut path = self.bounds();

		if transform.matrix2 == DMat2::ZERO {
			return None;
		}
		path.apply_affine(glam_to_kurbo(transform));

		let kurbo::Rect { x0, y0, x1, y1 } = path.bounding_box();
		Some([(x0, y0).into(), (x1, y1).into()])
	}

	fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>, _render_data: &RenderData) {
		if intersect_quad_bez_path(quad, &self.bounds(), true) {
			intersections.push(path.clone());
		}
	}
}

impl NodeGraphFrameLayer {
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

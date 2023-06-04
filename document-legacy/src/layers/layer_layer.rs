use super::layer_info::LayerData;
use super::style::{RenderData, ViewMode};
use crate::intersection::{intersect_quad_bez_path, intersect_quad_subpath, Quad};
use crate::LayerId;

use glam::{DAffine2, DMat2, DVec2};
use graphene_core::vector::VectorData;
use graphene_core::SurfaceId;
use kurbo::{Affine, BezPath, Shape as KurboShape};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub enum CachedOutputData {
	#[default]
	None,
	BlobURL(String),
	VectorPath(Box<VectorData>),
	SurfaceId(SurfaceId),
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct LayerLayer {
	/// The document node network that this layer contains
	pub network: graph_craft::document::NodeNetwork,

	#[serde(skip)]
	pub cached_output_data: CachedOutputData,
}

impl LayerData for LayerLayer {
	fn render(&mut self, svg: &mut String, svg_defs: &mut String, transforms: &mut Vec<DAffine2>, render_data: &RenderData) -> bool {
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

		// Render any paths if they exist
		match &self.cached_output_data {
			CachedOutputData::VectorPath(vector_data) => {
				let layer_bounds = vector_data.bounding_box().unwrap_or_default();
				let transformed_bounds = vector_data.bounding_box_with_transform(transform).unwrap_or_default();

				let _ = write!(svg, "<path d=\"");
				for subpath in &vector_data.subpaths {
					let _ = subpath.subpath_to_svg(svg, transform);
				}
				svg.push('"');

				svg.push_str(&vector_data.style.render(render_data.view_mode, svg_defs, transform, layer_bounds, transformed_bounds));
				let _ = write!(svg, "/>");
			}
			CachedOutputData::BlobURL(blob_url) => {
				// Render the image if it exists
				let _ = write!(
					svg,
					r#"<image width="{}" height="{}" preserveAspectRatio="none" href="{}" transform="matrix({})" />"#,
					width.abs(),
					height.abs(),
					blob_url,
					matrix
				);
			}
			CachedOutputData::SurfaceId(SurfaceId(id)) => {
				// Render the image if it exists
				let _ = write!(
					svg,
					r#"
					<foreignObject width="{}" height="{}" transform="matrix({})"><div data-canvas-placeholder="canvas{}"></div></foreignObject>
					"#,
					width.abs(),
					height.abs(),
					matrix,
					id
				);
			}
			_ => {
				// Render a dotted blue outline if there is no image or vector data
				let _ = write!(
					svg,
					r#"<rect width="{}" height="{}" fill="none" stroke="var(--color-data-vector)" stroke-width="3" stroke-dasharray="8" transform="matrix({})" />"#,
					width.abs(),
					height.abs(),
					matrix,
				);
			}
		}

		let _ = svg.write_str(r#"</g>"#);

		false
	}

	fn bounding_box(&self, transform: glam::DAffine2, _render_data: &RenderData) -> Option<[DVec2; 2]> {
		if let CachedOutputData::VectorPath(vector_data) = &self.cached_output_data {
			return vector_data.bounding_box_with_transform(transform);
		}

		let mut path = self.bounds();

		if transform.matrix2 == DMat2::ZERO {
			return None;
		}
		path.apply_affine(glam_to_kurbo(transform));

		let kurbo::Rect { x0, y0, x1, y1 } = path.bounding_box();
		Some([(x0, y0).into(), (x1, y1).into()])
	}

	fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>, _render_data: &RenderData) {
		if let CachedOutputData::VectorPath(vector_data) = &self.cached_output_data {
			let filled_style = vector_data.style.fill().is_some();
			if vector_data.subpaths.iter().any(|subpath| intersect_quad_subpath(quad, subpath, filled_style || subpath.closed())) {
				intersections.push(path.clone());
			}
		} else if intersect_quad_bez_path(quad, &self.bounds(), true) {
			intersections.push(path.clone());
		}
	}
}

impl LayerLayer {
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

	pub fn as_vector_data(&self) -> Option<&VectorData> {
		if let CachedOutputData::VectorPath(vector_data) = &self.cached_output_data {
			Some(vector_data)
		} else {
			None
		}
	}
	pub fn as_blob_url(&self) -> Option<&String> {
		if let CachedOutputData::BlobURL(blob_url) = &self.cached_output_data {
			Some(blob_url)
		} else {
			None
		}
	}
}

fn glam_to_kurbo(transform: DAffine2) -> Affine {
	Affine::new(transform.to_cols_array())
}

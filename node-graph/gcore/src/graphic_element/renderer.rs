use crate::raster::{Image, ImageFrame};
use crate::{uuid::generate_uuid, vector::VectorData, Artboard, Color, GraphicElementData, GraphicGroup};
use quad::Quad;

use glam::{DAffine2, DVec2};

mod quad;

/// Mutable state used whilst rendering to an SVG
pub struct SvgRender {
	pub svg: SvgSegmentList,
	pub svg_defs: String,
	pub transform: DAffine2,
	pub image_data: Vec<(u64, Image<Color>)>,
}

impl SvgRender {
	pub fn new() -> Self {
		Self {
			svg: SvgSegmentList::default(),
			svg_defs: String::new(),
			transform: DAffine2::IDENTITY,
			image_data: Vec::new(),
		}
	}

	/// Add an outer `<svg />` tag with a `viewBox` and the `<defs />`
	pub fn format_svg(&mut self, bounds_min: DVec2, bounds_max: DVec2) {
		let (x, y) = bounds_min.into();
		let (size_x, size_y) = (bounds_max - bounds_min).into();
		let defs = &self.svg_defs;
		let svg_header = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{x} {y} {size_x} {size_y}"><defs>{defs}</defs>"#,);
		self.svg.insert(0, svg_header.into());
		self.svg.push("</svg>".into());
	}
}

impl Default for SvgRender {
	fn default() -> Self {
		Self::new()
	}
}

/// Static state used whilst rendering
pub struct RenderParams {
	pub view_mode: crate::vector::style::ViewMode,
	pub culling_bounds: Option<[DVec2; 2]>,
	pub thumbnail: bool,
}

impl RenderParams {
	pub fn new(view_mode: crate::vector::style::ViewMode, culling_bounds: Option<[DVec2; 2]>, thumbnail: bool) -> Self {
		Self { view_mode, culling_bounds, thumbnail }
	}
}

fn format_transform_matrix(transform: DAffine2) -> String {
	transform.to_cols_array().iter().map(ToString::to_string).collect::<Vec<_>>().join(",")
}

pub trait GraphicElementRendered {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams);
	fn bounding_box(&self, transform: DAffine2) -> Option<[DVec2; 2]>;
}

impl GraphicElementRendered for GraphicGroup {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		self.iter().for_each(|element| element.graphic_element_data.render_svg(render, render_params))
	}
	fn bounding_box(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		self.iter().filter_map(|element| element.graphic_element_data.bounding_box(transform)).reduce(Quad::combine_bounds)
	}
}

impl GraphicElementRendered for VectorData {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		let layer_bounds = self.bounding_box().unwrap_or_default();
		let transformed_bounds = self.bounding_box_with_transform(render.transform).unwrap_or_default();

		render.svg.push("<path d=\"".into());
		let mut path = String::new();
		for subpath in &self.subpaths {
			let _ = subpath.subpath_to_svg(&mut path, self.transform * render.transform);
		}
		render.svg.push(path.into());
		render.svg.push("\"".into());

		let style = self.style.render(render_params.view_mode, &mut render.svg_defs, render.transform, layer_bounds, transformed_bounds);
		render.svg.push(style.into());
		render.svg.push("/>".into());
	}
	fn bounding_box(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		self.bounding_box_with_transform(self.transform * transform)
	}
}

impl GraphicElementRendered for Artboard {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		self.graphic_group.render_svg(render, render_params)
	}
	fn bounding_box(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		let artboard_bounds = (transform * Quad::from_box([self.location.as_dvec2(), self.location.as_dvec2() + self.dimensions.as_dvec2()])).bounding_box();
		[self.graphic_group.bounding_box(transform), Some(artboard_bounds)].into_iter().flatten().reduce(Quad::combine_bounds)
	}
}

impl GraphicElementRendered for ImageFrame<Color> {
	fn render_svg(&self, render: &mut SvgRender, _render_params: &RenderParams) {
		let transform: String = format_transform_matrix(self.transform * render.transform);
		render
			.svg
			.push(format!(r#"<image width="1" height="1" preserveAspectRatio="none" transform="matrix({transform})" href=""#).into());
		let uuid = generate_uuid();
		render.svg.push(SvgSegment::BlobUrl(uuid));
		render.svg.push("\" />".into());
		render.image_data.push((uuid, self.image.clone()))
	}
	fn bounding_box(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		let transform = self.transform * transform;
		(transform.matrix2 != glam::DMat2::ZERO).then(|| (transform * Quad::from_box([DVec2::ZERO, DVec2::ONE])).bounding_box())
	}
}

impl GraphicElementRendered for GraphicElementData {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		match self {
			GraphicElementData::VectorShape(vector_data) => vector_data.render_svg(render, render_params),
			GraphicElementData::ImageFrame(image_frame) => image_frame.render_svg(render, render_params),
			GraphicElementData::Text(_) => todo!("Render a text GraphicElementData"),
			GraphicElementData::GraphicGroup(graphic_group) => graphic_group.render_svg(render, render_params),
			GraphicElementData::Artboard(artboard) => artboard.render_svg(render, render_params),
		}
	}

	fn bounding_box(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		match self {
			GraphicElementData::VectorShape(vector_data) => GraphicElementRendered::bounding_box(&**vector_data, transform),
			GraphicElementData::ImageFrame(image_frame) => image_frame.bounding_box(transform),
			GraphicElementData::Text(_) => todo!("Bounds of a text GraphicElementData"),
			GraphicElementData::GraphicGroup(graphic_group) => graphic_group.bounding_box(transform),
			GraphicElementData::Artboard(artboard) => artboard.bounding_box(transform),
		}
	}
}

/// A segment of an svg string to allow for embedding blob urls
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SvgSegment {
	Slice(&'static str),
	String(String),
	BlobUrl(u64),
}

impl From<String> for SvgSegment {
	fn from(value: String) -> Self {
		Self::String(value)
	}
}

impl From<&'static str> for SvgSegment {
	fn from(value: &'static str) -> Self {
		Self::Slice(value)
	}
}

/// A list of [`SvgSegment`]s.
///
/// Can be modified with `list.push("hello".into())`. Use `list.to_string()` to convert the segments into one string.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SvgSegmentList(Vec<SvgSegment>);

impl core::ops::Deref for SvgSegmentList {
	type Target = Vec<SvgSegment>;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
impl core::ops::DerefMut for SvgSegmentList {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl core::fmt::Display for SvgSegmentList {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		for segment in self.iter() {
			f.write_str(match segment {
				SvgSegment::Slice(x) => x,
				SvgSegment::String(x) => x,
				SvgSegment::BlobUrl(_) => "<!-- Blob url not yet loaded -->",
			})?;
		}
		Ok(())
	}
}

use glam::{DAffine2, DVec2};

use crate::{vector::VectorData, Artboard, GraphicElementData, GraphicGroup};
mod quad;
use quad::{combine_bounds, Quad};

pub struct SvgRender {
	pub svg: String,
	pub svg_defs: String,
	pub transform: DAffine2,
}

impl SvgRender {
	pub fn new() -> Self {
		Self {
			svg: String::new(),
			svg_defs: String::new(),
			transform: DAffine2::IDENTITY,
		}
	}
}

pub struct RenderParams {
	pub view_mode: crate::vector::style::ViewMode,
	pub culling_bounds: Option<[DVec2; 2]>,
	pub thumbnail: bool,
}

fn format_transform(transform: DAffine2) -> String {
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
		self.iter().filter_map(|element| element.graphic_element_data.bounding_box(transform)).reduce(combine_bounds)
	}
}

impl GraphicElementRendered for VectorData {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		let layer_bounds = self.bounding_box().unwrap_or_default();
		let transfomed_bounds = self.bounding_box_with_transform(render.transform).unwrap_or_default();

		render.svg.push_str("<path d=\"");
		for subpath in &self.subpaths {
			let _ = subpath.subpath_to_svg(&mut render.svg, self.transform * render.transform);
		}
		render.svg.push('"');

		render
			.svg
			.push_str(&self.style.render(render_params.view_mode, &mut render.svg_defs, render.transform, layer_bounds, transfomed_bounds));
		render.svg.push_str("/>");
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
		let artboard_bounds = self.bounds.map(|[a, b]| (transform * Quad::from_box([a.as_dvec2(), b.as_dvec2()])).bounding_box());
		[self.graphic_group.bounding_box(transform), artboard_bounds].into_iter().flatten().reduce(combine_bounds)
	}
}

impl GraphicElementRendered for GraphicElementData {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		match self {
			GraphicElementData::VectorShape(vector_data) => vector_data.render_svg(render, render_params),
			GraphicElementData::ImageFrame(_) => todo!("Render an ImageFrame GraphicElementData"),
			GraphicElementData::Text(_) => todo!("Render a text GraphicElementData"),
			GraphicElementData::GraphicGroup(graphic_group) => graphic_group.render_svg(render, render_params),
			GraphicElementData::Artboard(artboard) => artboard.render_svg(render, render_params),
		}
	}

	fn bounding_box(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		match self {
			GraphicElementData::VectorShape(vector_data) => GraphicElementRendered::bounding_box(&**vector_data, transform),
			GraphicElementData::ImageFrame(_) => todo!("Bounds of an ImageFrame GraphicElementData"),
			GraphicElementData::Text(_) => todo!("Bounds of a text GraphicElementData"),
			GraphicElementData::GraphicGroup(graphic_group) => graphic_group.bounding_box(transform),
			GraphicElementData::Artboard(artboard) => artboard.bounding_box(transform),
		}
	}
}

mod quad;

use crate::raster::{BlendMode, Image, ImageFrame};
use crate::transform::Transform;
use crate::uuid::{generate_uuid, ManipulatorGroupId};
use crate::{vector::VectorData, Artboard, Color, GraphicElement, GraphicGroup};
pub use quad::Quad;

use bezier_rs::Subpath;

use base64::Engine;
use glam::{DAffine2, DVec2};

/// Represents a clickable target for the layer
#[derive(Clone, Debug)]
pub struct ClickTarget {
	pub subpath: bezier_rs::Subpath<ManipulatorGroupId>,
	pub stroke_width: f64,
}

impl ClickTarget {
	/// Does the click target intersect the rectangle
	pub fn intersect_rectangle(&self, document_quad: Quad, layer_transform: DAffine2) -> bool {
		// Check if the matrix is not invertible
		if layer_transform.matrix2.determinant().abs() <= std::f64::EPSILON {
			return false;
		}
		let quad = layer_transform.inverse() * document_quad;

		// Check if outlines intersect
		if self
			.subpath
			.iter()
			.any(|path_segment| quad.bezier_lines().any(|line| !path_segment.intersections(&line, None, None).is_empty()))
		{
			return true;
		}
		// Check if selection is entirely within the shape
		if self.subpath.closed() && self.subpath.contains_point(quad.center()) {
			return true;
		}

		// Check if shape is entirely within selection
		self.subpath
			.manipulator_groups()
			.first()
			.map(|group| group.anchor)
			.map(|shape_point| quad.contains(shape_point))
			.unwrap_or_default()
	}

	/// Does the click target intersect the point (accounting for stroke size)
	pub fn intersect_point(&self, point: DVec2, layer_transform: DAffine2) -> bool {
		// Allows for selecting lines
		// TODO: actual intersection of stroke
		let inflated_quad = Quad::from_box([point - DVec2::splat(self.stroke_width / 2.), point + DVec2::splat(self.stroke_width / 2.)]);
		self.intersect_rectangle(inflated_quad, layer_transform)
	}
}

/// Mutable state used whilst rendering to an SVG
pub struct SvgRender {
	pub svg: Vec<SvgSegment>,
	pub svg_defs: String,
	pub transform: DAffine2,
	pub image_data: Vec<(u64, Image<Color>)>,
	indent: usize,
}

impl SvgRender {
	pub fn new() -> Self {
		Self {
			svg: Vec::default(),
			svg_defs: String::new(),
			transform: DAffine2::IDENTITY,
			image_data: Vec::new(),
			indent: 0,
		}
	}

	pub fn indent(&mut self) {
		self.svg.push("\n".into());
		self.svg.push("\t".repeat(self.indent).into());
	}

	/// Add an outer `<svg>...</svg>` tag with a `viewBox` and the `<defs />`
	pub fn format_svg(&mut self, bounds_min: DVec2, bounds_max: DVec2) {
		let (x, y) = bounds_min.into();
		let (size_x, size_y) = (bounds_max - bounds_min).into();
		let defs = &self.svg_defs;
		let svg_header = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{x} {y} {size_x} {size_y}"><defs>{defs}</defs>"#,);
		self.svg.insert(0, svg_header.into());
		self.svg.push("</svg>".into());
	}

	/// Wraps the SVG with `<svg><g transform="...">...</g></svg>`, which allows for rotation
	pub fn wrap_with_transform(&mut self, transform: DAffine2, size: Option<DVec2>) {
		let defs = &self.svg_defs;
		let view_box = size
			.map(|size| format!("viewbox=\"0 0 {} {}\" width=\"{}\" height=\"{}\"", size.x, size.y, size.x, size.y))
			.unwrap_or_default();

		let svg_header = format!(
			r#"<svg xmlns="http://www.w3.org/2000/svg" {}><defs>{defs}</defs><g transform="{}">"#,
			view_box,
			format_transform_matrix(transform)
		);
		self.svg.insert(0, svg_header.into());
		self.svg.push("</g></svg>".into());
	}

	pub fn leaf_tag(&mut self, name: impl Into<SvgSegment>, attributes: impl FnOnce(&mut SvgRenderAttrs)) {
		self.indent();
		self.svg.push("<".into());
		self.svg.push(name.into());
		attributes(&mut SvgRenderAttrs(self));

		self.svg.push("/>".into());
	}

	pub fn leaf_node(&mut self, content: impl Into<SvgSegment>) {
		self.indent();
		self.svg.push(content.into());
	}

	pub fn parent_tag(&mut self, name: impl Into<SvgSegment>, attributes: impl FnOnce(&mut SvgRenderAttrs), inner: impl FnOnce(&mut Self)) {
		let name = name.into();
		self.indent();
		self.svg.push("<".into());
		self.svg.push(name.clone());
		// Wraps `self` in a newtype (1-tuple) which is then mutated by the `attributes` closure
		attributes(&mut SvgRenderAttrs(self));
		self.svg.push(">".into());
		let length = self.svg.len();
		self.indent += 1;
		inner(self);
		self.indent -= 1;
		if self.svg.len() != length {
			self.indent();
			self.svg.push("</".into());
			self.svg.push(name);
			self.svg.push(">".into());
		} else {
			self.svg.pop();
			self.svg.push("/>".into());
		}
	}
}

impl Default for SvgRender {
	fn default() -> Self {
		Self::new()
	}
}

pub enum ImageRenderMode {
	Base64,
}

/// Static state used whilst rendering
pub struct RenderParams {
	pub view_mode: crate::vector::style::ViewMode,
	pub image_render_mode: ImageRenderMode,
	pub culling_bounds: Option<[DVec2; 2]>,
	pub thumbnail: bool,
	/// Don't render the rectangle for an artboard to allow exporting with a transparent background.
	pub hide_artboards: bool,
	/// Are we exporting? Causes the text above an artboard to be hidden.
	pub for_export: bool,
}

impl RenderParams {
	pub fn new(view_mode: crate::vector::style::ViewMode, image_render_mode: ImageRenderMode, culling_bounds: Option<[DVec2; 2]>, thumbnail: bool, hide_artboards: bool, for_export: bool) -> Self {
		Self {
			view_mode,
			image_render_mode,
			culling_bounds,
			thumbnail,
			hide_artboards,
			for_export,
		}
	}
}

pub fn format_transform_matrix(transform: DAffine2) -> String {
	use std::fmt::Write;
	let mut result = "matrix(".to_string();
	let cols = transform.to_cols_array();
	for (index, item) in cols.iter().enumerate() {
		write!(result, "{item}").unwrap();
		if index != cols.len() - 1 {
			result.push_str(", ");
		}
	}
	result.push(')');
	result
}

fn to_transform(transform: DAffine2) -> usvg::Transform {
	let cols = transform.to_cols_array();
	usvg::Transform::from_row(cols[0] as f32, cols[1] as f32, cols[2] as f32, cols[3] as f32, cols[4] as f32, cols[5] as f32)
}

pub trait GraphicElementRendered {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams);
	fn bounding_box(&self, transform: DAffine2) -> Option<[DVec2; 2]>;
	fn add_click_targets(&self, click_targets: &mut Vec<ClickTarget>);
	fn to_usvg_node(&self) -> usvg::Node {
		let mut render = SvgRender::new();
		let render_params = RenderParams::new(crate::vector::style::ViewMode::Normal, ImageRenderMode::Base64, None, false, false, false);
		self.render_svg(&mut render, &render_params);
		render.format_svg(DVec2::ZERO, DVec2::ONE);
		let svg = render.svg.to_svg_string();

		let opt = usvg::Options::default();

		let tree = usvg::Tree::from_str(&svg, &opt).expect("Failed to parse SVG");
		usvg::Node::Group(Box::new(tree.root.clone()))
	}

	fn to_usvg_tree(&self, resolution: glam::UVec2, viewbox: [DVec2; 2]) -> usvg::Tree {
		let root = match self.to_usvg_node() {
			usvg::Node::Group(root_node) => *root_node,
			_ => usvg::Group::default(),
		};
		usvg::Tree {
			size: usvg::Size::from_wh(resolution.x as f32, resolution.y as f32).unwrap(),
			view_box: usvg::ViewBox {
				rect: usvg::NonZeroRect::from_ltrb(viewbox[0].x as f32, viewbox[0].y as f32, viewbox[1].x as f32, viewbox[1].y as f32).unwrap(),
				aspect: usvg::AspectRatio::default(),
			},
			root,
		}
	}

	fn contains_artboard(&self) -> bool {
		false
	}
}

impl GraphicElementRendered for GraphicGroup {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		render.parent_tag(
			"g",
			|attributes| {
				attributes.push("transform", format_transform_matrix(self.transform));

				if self.alpha_blending.opacity < 1. {
					attributes.push("opacity", self.alpha_blending.opacity.to_string());
				}

				if self.alpha_blending.blend_mode != BlendMode::default() {
					attributes.push("style", self.alpha_blending.blend_mode.render());
				}
			},
			|render| {
				for element in self.iter() {
					element.render_svg(render, render_params);
				}
			},
		);
	}

	fn bounding_box(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		self.iter().filter_map(|element| element.bounding_box(transform * self.transform)).reduce(Quad::combine_bounds)
	}

	fn add_click_targets(&self, click_targets: &mut Vec<ClickTarget>) {
		for element in self.elements.iter() {
			let mut new_click_targets = Vec::new();
			element.add_click_targets(&mut new_click_targets);
			for click_target in new_click_targets.iter_mut() {
				click_target.subpath.apply_transform(element.transform())
			}
			click_targets.extend(new_click_targets);
		}
	}

	fn to_usvg_node(&self) -> usvg::Node {
		let mut root_node = usvg::Group::default();
		for element in self.iter() {
			root_node.children.push(element.to_usvg_node());
		}
		usvg::Node::Group(Box::new(root_node))
	}

	fn contains_artboard(&self) -> bool {
		self.iter().any(|element| element.contains_artboard())
	}
}

impl GraphicElementRendered for VectorData {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		let multiplied_transform = render.transform * self.transform;
		let layer_bounds = self.bounding_box().unwrap_or_default();
		let transformed_bounds = self.bounding_box_with_transform(multiplied_transform).unwrap_or_default();

		let mut path = String::new();
		for subpath in &self.subpaths {
			let _ = subpath.subpath_to_svg(&mut path, multiplied_transform);
		}

		render.leaf_tag("path", |attributes| {
			attributes.push("class", "vector-data");

			attributes.push("d", path);

			let fill_and_stroke = self
				.style
				.render(render_params.view_mode, &mut attributes.0.svg_defs, multiplied_transform, layer_bounds, transformed_bounds);
			attributes.push_val(fill_and_stroke);

			if self.alpha_blending.opacity < 1. {
				attributes.push("opacity", self.alpha_blending.opacity.to_string());
			}

			if self.alpha_blending.blend_mode != BlendMode::default() {
				attributes.push("style", self.alpha_blending.blend_mode.render());
			}
		});
	}

	fn bounding_box(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		self.bounding_box_with_transform(self.transform * transform)
	}

	fn add_click_targets(&self, click_targets: &mut Vec<ClickTarget>) {
		let stroke_width = self.style.stroke().as_ref().map_or(0., crate::vector::style::Stroke::weight);
		let update_closed = |mut subpath: bezier_rs::Subpath<ManipulatorGroupId>| {
			subpath.set_closed(self.style.fill().is_some());
			subpath
		};
		click_targets.extend(self.subpaths.iter().cloned().map(update_closed).map(|subpath| ClickTarget { stroke_width, subpath }))
	}

	fn to_usvg_node(&self) -> usvg::Node {
		use bezier_rs::BezierHandles;
		use usvg::tiny_skia_path::PathBuilder;
		let mut builder = PathBuilder::new();
		let vector_data = self;

		let transform = to_transform(vector_data.transform);
		for subpath in vector_data.subpaths.iter() {
			let start = vector_data.transform.transform_point2(subpath[0].anchor);
			builder.move_to(start.x as f32, start.y as f32);
			for bezier in subpath.iter() {
				bezier.apply_transformation(|pos| vector_data.transform.transform_point2(pos));
				let end = bezier.end;
				match bezier.handles {
					BezierHandles::Linear => builder.line_to(end.x as f32, end.y as f32),
					BezierHandles::Quadratic { handle } => builder.quad_to(handle.x as f32, handle.y as f32, end.x as f32, end.y as f32),
					BezierHandles::Cubic { handle_start, handle_end } => {
						builder.cubic_to(handle_start.x as f32, handle_start.y as f32, handle_end.x as f32, handle_end.y as f32, end.x as f32, end.y as f32)
					}
				}
			}
			if subpath.closed {
				builder.close()
			}
		}
		let path = builder.finish().unwrap();
		let mut path = usvg::Path::new(path.into());
		path.abs_transform = transform;
		// TODO: use proper style
		path.fill = None;
		path.stroke = Some(usvg::Stroke::default());
		usvg::Node::Path(Box::new(path))
	}
}

impl GraphicElementRendered for Artboard {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		if !render_params.hide_artboards {
			// Background
			render.leaf_tag("rect", |attributes| {
				attributes.push("class", "artboard-bg");
				attributes.push("fill", format!("#{}", self.background.rgba_hex()));
				attributes.push("x", self.location.x.min(self.location.x + self.dimensions.x).to_string());
				attributes.push("y", self.location.y.min(self.location.y + self.dimensions.y).to_string());
				attributes.push("width", self.dimensions.x.abs().to_string());
				attributes.push("height", self.dimensions.y.abs().to_string());
			});
		}
		if !render_params.hide_artboards && !render_params.for_export {
			// Label
			render.parent_tag(
				"text",
				|attributes| {
					attributes.push("class", "artboard-label");
					attributes.push("fill", "white");
					attributes.push("x", (self.location.x.min(self.location.x + self.dimensions.x)).to_string());
					attributes.push("y", (self.location.y.min(self.location.y + self.dimensions.y) - 4).to_string());
					attributes.push("font-size", "14px");
				},
				|render| {
					// TODO: Use the artboard's layer name
					render.svg.push("Artboard".into());
				},
			);
		}

		// Contents group (includes the artwork but not the background)
		render.parent_tag(
			// SVG group tag
			"g",
			// Group tag attributes
			|attributes| {
				attributes.push("class", "artboard");

				attributes.push(
					"transform",
					format_transform_matrix(DAffine2::from_translation(self.location.as_dvec2()) * self.graphic_group.transform),
				);

				if self.clip {
					let id = format!("artboard-{}", generate_uuid());
					let selector = format!("url(#{id})");
					use std::fmt::Write;
					write!(
						&mut attributes.0.svg_defs,
						r##"<clipPath id="{id}"><rect x="0" y="0" width="{}" height="{}" transform="{}"/></clipPath>"##,
						self.dimensions.x,
						self.dimensions.y,
						format_transform_matrix(self.graphic_group.transform.inverse())
					)
					.unwrap();
					attributes.push("clip-path", selector);
				}
			},
			// Artboard contents
			|render| {
				for element in self.graphic_group.iter() {
					element.render_svg(render, render_params);
				}
			},
		);
	}

	fn bounding_box(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		let artboard_bounds = (transform * Quad::from_box([self.location.as_dvec2(), self.location.as_dvec2() + self.dimensions.as_dvec2()])).bounding_box();
		if self.clip {
			Some(artboard_bounds)
		} else {
			[self.graphic_group.bounding_box(transform), Some(artboard_bounds)].into_iter().flatten().reduce(Quad::combine_bounds)
		}
	}

	fn add_click_targets(&self, click_targets: &mut Vec<ClickTarget>) {
		let subpath = Subpath::new_rect(DVec2::ZERO, self.dimensions.as_dvec2());
		click_targets.push(ClickTarget { stroke_width: 0., subpath });
	}

	fn contains_artboard(&self) -> bool {
		true
	}
}

impl GraphicElementRendered for ImageFrame<Color> {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		let transform: String = format_transform_matrix(self.transform * render.transform);

		match render_params.image_render_mode {
			ImageRenderMode::Base64 => {
				let image = &self.image;
				if image.data.is_empty() {
					return;
				}

				let base64_string = image.base64_string.clone().unwrap_or_else(|| {
					let output = image.to_png();
					let preamble = "data:image/png;base64,";
					let mut base64_string = String::with_capacity(preamble.len() + output.len() * 4);
					base64_string.push_str(preamble);
					base64::engine::general_purpose::STANDARD.encode_string(output, &mut base64_string);
					base64_string
				});
				render.leaf_tag("image", |attributes| {
					attributes.push("width", 1.to_string());
					attributes.push("height", 1.to_string());
					attributes.push("preserveAspectRatio", "none");
					attributes.push("transform", transform);
					attributes.push("href", base64_string);
					if self.alpha_blending.blend_mode != BlendMode::default() {
						attributes.push("style", self.alpha_blending.blend_mode.render());
					}
				});
			}
		}
	}

	fn bounding_box(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		let transform = self.transform * transform;
		(transform.matrix2 != glam::DMat2::ZERO).then(|| (transform * Quad::from_box([DVec2::ZERO, DVec2::ONE])).bounding_box())
	}

	fn add_click_targets(&self, click_targets: &mut Vec<ClickTarget>) {
		let subpath = Subpath::new_rect(DVec2::ZERO, DVec2::ONE);
		click_targets.push(ClickTarget { subpath, stroke_width: 0. });
	}

	fn to_usvg_node(&self) -> usvg::Node {
		let image_frame = self;
		if image_frame.image.width * image_frame.image.height == 0 {
			return usvg::Node::Group(Box::default());
		}
		let png = image_frame.image.to_png();
		usvg::Node::Image(Box::new(usvg::Image {
			id: String::new(),
			abs_transform: to_transform(image_frame.transform),
			visibility: usvg::Visibility::Visible,
			view_box: usvg::ViewBox {
				rect: usvg::NonZeroRect::from_xywh(0., 0., 1., 1.).unwrap(),
				aspect: usvg::AspectRatio::default(),
			},
			rendering_mode: usvg::ImageRendering::OptimizeSpeed,
			kind: usvg::ImageKind::PNG(png.into()),
			bounding_box: None,
		}))
	}
}

impl GraphicElementRendered for GraphicElement {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		match self {
			GraphicElement::VectorData(vector_data) => vector_data.render_svg(render, render_params),
			GraphicElement::ImageFrame(image_frame) => image_frame.render_svg(render, render_params),
			GraphicElement::Text(_) => todo!("Render a text GraphicElement"),
			GraphicElement::GraphicGroup(graphic_group) => graphic_group.render_svg(render, render_params),
			GraphicElement::Artboard(artboard) => artboard.render_svg(render, render_params),
		}
	}

	fn bounding_box(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		match self {
			GraphicElement::VectorData(vector_data) => GraphicElementRendered::bounding_box(&**vector_data, transform),
			GraphicElement::ImageFrame(image_frame) => image_frame.bounding_box(transform),
			GraphicElement::Text(_) => todo!("Bounds of a text GraphicElement"),
			GraphicElement::GraphicGroup(graphic_group) => graphic_group.bounding_box(transform),
			GraphicElement::Artboard(artboard) => artboard.bounding_box(transform),
		}
	}

	fn add_click_targets(&self, click_targets: &mut Vec<ClickTarget>) {
		match self {
			GraphicElement::VectorData(vector_data) => vector_data.add_click_targets(click_targets),
			GraphicElement::ImageFrame(image_frame) => image_frame.add_click_targets(click_targets),
			GraphicElement::Text(_) => todo!("click target for text GraphicElement"),
			GraphicElement::GraphicGroup(graphic_group) => graphic_group.add_click_targets(click_targets),
			GraphicElement::Artboard(artboard) => artboard.add_click_targets(click_targets),
		}
	}

	fn to_usvg_node(&self) -> usvg::Node {
		match self {
			GraphicElement::VectorData(vector_data) => vector_data.to_usvg_node(),
			GraphicElement::ImageFrame(image_frame) => image_frame.to_usvg_node(),
			GraphicElement::Text(text) => text.to_usvg_node(),
			GraphicElement::GraphicGroup(graphic_group) => graphic_group.to_usvg_node(),
			GraphicElement::Artboard(artboard) => artboard.to_usvg_node(),
		}
	}

	fn contains_artboard(&self) -> bool {
		match self {
			GraphicElement::VectorData(vector_data) => vector_data.contains_artboard(),
			GraphicElement::ImageFrame(image_frame) => image_frame.contains_artboard(),
			GraphicElement::Text(text) => text.contains_artboard(),
			GraphicElement::GraphicGroup(graphic_group) => graphic_group.contains_artboard(),
			GraphicElement::Artboard(artboard) => artboard.contains_artboard(),
		}
	}
}

/// Used to stop rust complaining about upstream traits adding display implementations to `Option<Color>`. This would not be an issue as we control that crate.
trait Primitive: core::fmt::Display {}
impl Primitive for String {}
impl Primitive for bool {}
impl Primitive for f32 {}
impl Primitive for f64 {}

fn text_attributes(attributes: &mut SvgRenderAttrs) {
	attributes.push("fill", "white");
	attributes.push("y", "30");
	attributes.push("font-size", "30");
}

impl<T: Primitive> GraphicElementRendered for T {
	fn render_svg(&self, render: &mut SvgRender, _render_params: &RenderParams) {
		render.parent_tag("text", text_attributes, |render| render.leaf_node(format!("{self}")));
	}

	fn bounding_box(&self, _transform: DAffine2) -> Option<[DVec2; 2]> {
		None
	}

	fn add_click_targets(&self, _click_targets: &mut Vec<ClickTarget>) {}

	fn to_usvg_node(&self) -> usvg::Node {
		let text = self;
		usvg::Node::Text(Box::new(usvg::Text {
			id: String::new(),
			abs_transform: usvg::Transform::identity(),
			rendering_mode: usvg::TextRendering::OptimizeSpeed,
			writing_mode: usvg::WritingMode::LeftToRight,
			chunks: vec![usvg::TextChunk {
				text: text.to_string(),
				x: None,
				y: None,
				anchor: usvg::TextAnchor::Start,
				spans: vec![],
				text_flow: usvg::TextFlow::Linear,
			}],
			dx: Vec::new(),
			dy: Vec::new(),
			rotate: Vec::new(),
			bounding_box: None,
			abs_bounding_box: None,
			stroke_bounding_box: None,
			abs_stroke_bounding_box: None,
			flattened: None,
		}))
	}
}

impl GraphicElementRendered for Option<Color> {
	fn render_svg(&self, render: &mut SvgRender, _render_params: &RenderParams) {
		let Some(color) = self else {
			render.parent_tag("text", |_| {}, |render| render.leaf_node("Empty color"));
			return;
		};
		let color_info = format!("{:?} #{} {:?}", color, color.rgba_hex(), color.to_rgba8_srgb());

		render.leaf_tag("rect", |attributes| {
			attributes.push("width", "100");
			attributes.push("height", "100");
			attributes.push("y", "40");
			attributes.push("fill", format!("#{}", color.rgba_hex()));
		});
		render.parent_tag("text", text_attributes, |render| render.leaf_node(color_info))
	}

	fn bounding_box(&self, _transform: DAffine2) -> Option<[DVec2; 2]> {
		None
	}

	fn add_click_targets(&self, _click_targets: &mut Vec<ClickTarget>) {}
}

impl GraphicElementRendered for Vec<Color> {
	fn render_svg(&self, render: &mut SvgRender, _render_params: &RenderParams) {
		for (index, &color) in self.iter().enumerate() {
			render.leaf_tag("rect", |attributes| {
				attributes.push("width", "100");
				attributes.push("height", "100");
				attributes.push("x", (index * 120).to_string());
				attributes.push("y", "40");
				attributes.push("fill", format!("#{}", color.rgba_hex()));
			});
		}
	}

	fn bounding_box(&self, _transform: DAffine2) -> Option<[DVec2; 2]> {
		None
	}

	fn add_click_targets(&self, _click_targets: &mut Vec<ClickTarget>) {}
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

pub trait RenderSvgSegmentList {
	fn to_svg_string(&self) -> String;
}

impl RenderSvgSegmentList for Vec<SvgSegment> {
	fn to_svg_string(&self) -> String {
		let mut result = String::new();
		for segment in self.iter() {
			result.push_str(match segment {
				SvgSegment::Slice(x) => x,
				SvgSegment::String(x) => x,
				SvgSegment::BlobUrl(_) => "<!-- Blob url not yet loaded -->",
			});
		}
		result
	}
}

pub struct SvgRenderAttrs<'a>(&'a mut SvgRender);

impl<'a> SvgRenderAttrs<'a> {
	pub fn push_complex(&mut self, name: impl Into<SvgSegment>, value: impl FnOnce(&mut SvgRender)) {
		self.0.svg.push(" ".into());
		self.0.svg.push(name.into());
		self.0.svg.push("=\"".into());
		value(self.0);
		self.0.svg.push("\"".into());
	}
	pub fn push(&mut self, name: impl Into<SvgSegment>, value: impl Into<SvgSegment>) {
		self.push_complex(name, move |renderer| renderer.svg.push(value.into()));
	}
	pub fn push_val(&mut self, value: impl Into<SvgSegment>) {
		self.0.svg.push(value.into());
	}
}

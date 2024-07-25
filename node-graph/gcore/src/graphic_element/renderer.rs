mod quad;

use crate::raster::bbox::Bbox;
use crate::raster::{BlendMode, Image, ImageFrame};
use crate::transform::Transform;
use crate::uuid::generate_uuid;
use crate::vector::style::{Fill, Stroke, ViewMode};
use crate::vector::PointId;
use crate::SurfaceFrame;
use crate::{vector::VectorData, Artboard, Color, GraphicElement, GraphicGroup};
pub use quad::Quad;

use bezier_rs::Subpath;

use base64::Engine;
use glam::{DAffine2, DVec2};
#[cfg(feature = "vello")]
use vello::*;

/// Represents a clickable target for the layer
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ClickTarget {
	pub subpath: bezier_rs::Subpath<PointId>,
	pub stroke_width: f64,
}

impl ClickTarget {
	/// Does the click target intersect the rectangle
	pub fn intersect_rectangle(&self, document_quad: Quad, layer_transform: DAffine2) -> bool {
		// Check if the matrix is not invertible
		if layer_transform.matrix2.determinant().abs() <= f64::EPSILON {
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

#[derive(Default)]
pub enum ImageRenderMode {
	#[default]
	Base64,
}

/// Static state used whilst rendering
#[derive(Default)]
pub struct RenderParams {
	pub view_mode: ViewMode,
	pub image_render_mode: ImageRenderMode,
	pub culling_bounds: Option<[DVec2; 2]>,
	pub thumbnail: bool,
	/// Don't render the rectangle for an artboard to allow exporting with a transparent background.
	pub hide_artboards: bool,
	/// Are we exporting? Causes the text above an artboard to be hidden.
	pub for_export: bool,
}

impl RenderParams {
	pub fn new(view_mode: ViewMode, image_render_mode: ImageRenderMode, culling_bounds: Option<[DVec2; 2]>, thumbnail: bool, hide_artboards: bool, for_export: bool) -> Self {
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

pub fn to_transform(transform: DAffine2) -> usvg::Transform {
	let cols = transform.to_cols_array();
	usvg::Transform::from_row(cols[0] as f32, cols[1] as f32, cols[2] as f32, cols[3] as f32, cols[4] as f32, cols[5] as f32)
}

pub trait GraphicElementRendered {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams);
	fn bounding_box(&self, transform: DAffine2) -> Option<[DVec2; 2]>;
	fn add_click_targets(&self, click_targets: &mut Vec<ClickTarget>);
	#[cfg(feature = "vello")]
	fn to_vello_scene(&self, transform: DAffine2) -> Scene {
		let mut scene = vello::Scene::new();
		self.render_to_vello(&mut scene, transform);
		scene
	}
	#[cfg(feature = "vello")]
	fn render_to_vello(&self, _scene: &mut Scene, _transform: DAffine2) {}

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

	#[cfg(feature = "vello")]
	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2) {
		let kurbo_transform = kurbo::Affine::new((transform * self.transform).to_cols_array());
		let Some(bounds) = self.bounding_box(DAffine2::IDENTITY) else { return };
		let blending = vello::peniko::BlendMode::new(self.alpha_blending.blend_mode.into(), vello::peniko::Compose::SrcOver);
		scene.push_layer(
			blending,
			self.alpha_blending.opacity,
			kurbo_transform,
			&vello::kurbo::Rect::new(bounds[0].x, bounds[0].y, bounds[1].x, bounds[1].y),
		);
		for element in self.iter() {
			element.render_to_vello(scene, transform * self.transform);
		}
		scene.pop_layer();
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
		for (_, subpath) in self.region_bezier_paths() {
			let _ = subpath.subpath_to_svg(&mut path, multiplied_transform);
		}
		for subpath in self.stroke_bezier_paths() {
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
		let stroke_width = self.style.stroke().map(|s| s.weight()).unwrap_or_default();
		let scale = transform.decompose_scale();
		let offset = DVec2::splat(stroke_width * scale.x.max(scale.y) / 2.);
		self.bounding_box_with_transform(self.transform * transform).map(|[a, b]| [a - offset, b + offset])
	}

	fn add_click_targets(&self, click_targets: &mut Vec<ClickTarget>) {
		let stroke_width = self.style.stroke().as_ref().map_or(0., Stroke::weight);
		let filled = self.style.fill() != &Fill::None;
		let fill = |mut subpath: bezier_rs::Subpath<_>| {
			if filled {
				subpath.set_closed(true);
			}
			subpath
		};
		click_targets.extend(self.stroke_bezier_paths().map(fill).map(|subpath| ClickTarget { stroke_width, subpath }));
	}

	#[cfg(feature = "vello")]
	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2) {
		use crate::vector::style::GradientType;
		use vello::peniko;

		let kurbo_transform = kurbo::Affine::new(transform.to_cols_array());
		let to_point = |p: DVec2| kurbo::Point::new(p.x, p.y);
		let mut path = kurbo::BezPath::new();
		// TODO: Is this correct and efficient? Deesn't this lead to us potentially rendering a path twice?
		for (_, subpath) in self.region_bezier_paths() {
			subpath.to_vello_path(self.transform, &mut path);
		}
		for subpath in self.stroke_bezier_paths() {
			subpath.to_vello_path(self.transform, &mut path);
		}

		match self.style.fill() {
			Fill::Solid(color) => {
				let fill = peniko::Brush::Solid(peniko::Color::rgba(color.r() as f64, color.g() as f64, color.b() as f64, color.a() as f64));
				scene.fill(peniko::Fill::NonZero, kurbo_transform, &fill, None, &path);
			}
			Fill::Gradient(gradient) => {
				let mut stops = peniko::ColorStops::new();
				for &(offset, color) in &gradient.stops.0 {
					stops.push(peniko::ColorStop {
						offset: offset as f32,
						color: peniko::Color::rgba(color.r() as f64, color.g() as f64, color.b() as f64, color.a() as f64),
					});
				}
				// Compute bounding box of the shape to determine the gradient start and end points
				let bounds = self.bounding_box().unwrap_or_default();
				let lerp_bounds = |p: DVec2| bounds[0] + (bounds[1] - bounds[0]) * p;
				let start = lerp_bounds(gradient.start);
				let end = lerp_bounds(gradient.end);

				let transform = self.transform * gradient.transform;
				let start = transform.transform_point2(start);
				let end = transform.transform_point2(end);
				let fill = peniko::Brush::Gradient(peniko::Gradient {
					kind: match gradient.gradient_type {
						GradientType::Linear => peniko::GradientKind::Linear {
							start: to_point(start),
							end: to_point(end),
						},
						GradientType::Radial => {
							let radius = start.distance(end);
							peniko::GradientKind::Radial {
								start_center: to_point(start),
								start_radius: 0.,
								end_center: to_point(end),
								end_radius: radius as f32,
							}
						}
					},
					stops,
					..Default::default()
				});
				scene.fill(peniko::Fill::NonZero, kurbo_transform, &fill, None, &path);
			}
			Fill::None => (),
		};

		if let Some(stroke) = self.style.stroke() {
			let color = match stroke.color {
				Some(color) => peniko::Color::rgba(color.r() as f64, color.g() as f64, color.b() as f64, color.a() as f64),
				None => peniko::Color::TRANSPARENT,
			};
			let stroke = kurbo::Stroke {
				width: stroke.weight,
				miter_limit: stroke.line_join_miter_limit,
				..Default::default()
			};
			scene.stroke(&stroke, kurbo_transform, color, None, &path);
		}
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
					render.svg.push(self.label.to_string().into());
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

	#[cfg(feature = "vello")]
	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2) {
		use vello::peniko;

		// Render background
		let color = peniko::Color::rgba(self.background.r() as f64, self.background.g() as f64, self.background.b() as f64, self.background.a() as f64);
		let rect = kurbo::Rect::new(self.location.x as f64, self.location.y as f64, self.dimensions.x as f64, self.dimensions.y as f64);
		let blend_mode = peniko::BlendMode::new(peniko::Mix::Clip, peniko::Compose::SrcOver);

		scene.push_layer(peniko::Mix::Normal, 1., kurbo::Affine::new(transform.to_cols_array()), &rect);
		scene.fill(peniko::Fill::NonZero, kurbo::Affine::new(transform.to_cols_array()), color, None, &rect);
		scene.pop_layer();

		if self.clip {
			scene.push_layer(blend_mode, 1., kurbo::Affine::new(transform.to_cols_array()), &rect);
		}
		self.graphic_group.render_to_vello(scene, transform * self.transform());
		if self.clip {
			scene.pop_layer();
		}
	}

	fn add_click_targets(&self, click_targets: &mut Vec<ClickTarget>) {
		let mut subpath = Subpath::new_rect(DVec2::ZERO, self.dimensions.as_dvec2());
		subpath.apply_transform(self.graphic_group.transform.inverse());
		click_targets.push(ClickTarget { stroke_width: 0., subpath });
	}

	fn contains_artboard(&self) -> bool {
		true
	}
}

impl GraphicElementRendered for crate::ArtboardGroup {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		for artboard in &self.artboards {
			artboard.render_svg(render, render_params);
		}
	}

	fn bounding_box(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		self.artboards.iter().filter_map(|element| element.bounding_box(transform)).reduce(Quad::combine_bounds)
	}

	fn add_click_targets(&self, click_targets: &mut Vec<ClickTarget>) {
		for artboard in &self.artboards {
			artboard.add_click_targets(click_targets);
		}
	}

	#[cfg(feature = "vello")]
	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2) {
		for artboard in &self.artboards {
			artboard.render_to_vello(scene, transform)
		}
	}

	fn contains_artboard(&self) -> bool {
		!self.artboards.is_empty()
	}
}

impl GraphicElementRendered for SurfaceFrame {
	fn render_svg(&self, render: &mut SvgRender, _render_params: &RenderParams) {
		let transform = self.transform;
		let (width, height) = (transform.transform_vector2(DVec2::new(1., 0.)).length(), transform.transform_vector2(DVec2::new(0., 1.)).length());
		let matrix = (transform * DAffine2::from_scale((width, height).into()).inverse())
			.to_cols_array()
			.iter()
			.enumerate()
			.fold(String::new(), |val, (i, entry)| val + &(entry.to_string() + if i == 5 { "" } else { "," }));

		let canvas = format!(
			r#"<foreignObject width="{}" height="{}" transform="matrix({})"><div data-canvas-placeholder="canvas{}"></div></foreignObject>"#,
			width.abs(),
			height.abs(),
			matrix,
			self.surface_id
		);
		render.svg.push(canvas.into())
	}

	#[cfg(feature = "vello")]
	fn render_to_vello(&self, _scene: &mut Scene, _transform: DAffine2) {
		todo!()
	}

	fn bounding_box(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		let bbox = Bbox::from_transform(transform);
		let aabb = bbox.to_axis_aligned_bbox();
		Some([aabb.start, aabb.end])
	}

	fn add_click_targets(&self, _click_targets: &mut Vec<ClickTarget>) {}

	fn contains_artboard(&self) -> bool {
		false
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

	#[cfg(feature = "vello")]
	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2) {
		use vello::peniko;

		let image = &self.image;
		if image.data.is_empty() {
			return;
		}
		let image = vello::peniko::Image {
			data: image.to_flat_u8().0.into(),
			width: image.width,
			height: image.height,
			format: peniko::Format::Rgba8,
			extend: peniko::Extend::Repeat,
		};
		let transform = transform * self.transform * DAffine2::from_scale(1. / DVec2::new(image.width as f64, image.height as f64));

		scene.draw_image(&image, vello::kurbo::Affine::new(transform.to_cols_array()));
	}
}

impl GraphicElementRendered for GraphicElement {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		match self {
			GraphicElement::VectorData(vector_data) => vector_data.render_svg(render, render_params),
			GraphicElement::ImageFrame(image_frame) => image_frame.render_svg(render, render_params),
			GraphicElement::GraphicGroup(graphic_group) => graphic_group.render_svg(render, render_params),
			GraphicElement::Surface(surface) => surface.render_svg(render, render_params),
		}
	}

	fn bounding_box(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		match self {
			GraphicElement::VectorData(vector_data) => GraphicElementRendered::bounding_box(&**vector_data, transform),
			GraphicElement::ImageFrame(image_frame) => image_frame.bounding_box(transform),
			GraphicElement::GraphicGroup(graphic_group) => graphic_group.bounding_box(transform),
			GraphicElement::Surface(surface) => surface.bounding_box(transform),
		}
	}

	fn add_click_targets(&self, click_targets: &mut Vec<ClickTarget>) {
		match self {
			GraphicElement::VectorData(vector_data) => vector_data.add_click_targets(click_targets),
			GraphicElement::ImageFrame(image_frame) => image_frame.add_click_targets(click_targets),
			GraphicElement::GraphicGroup(graphic_group) => graphic_group.add_click_targets(click_targets),
			GraphicElement::Surface(surface) => surface.add_click_targets(click_targets),
		}
	}

	#[cfg(feature = "vello")]
	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2) {
		match self {
			GraphicElement::VectorData(vector_data) => vector_data.render_to_vello(scene, transform),
			GraphicElement::ImageFrame(image_frame) => image_frame.render_to_vello(scene, transform),
			GraphicElement::GraphicGroup(graphic_group) => graphic_group.render_to_vello(scene, transform),
			GraphicElement::Surface(surface) => surface.render_to_vello(scene, transform),
		}
	}

	fn contains_artboard(&self) -> bool {
		match self {
			GraphicElement::VectorData(vector_data) => vector_data.contains_artboard(),
			GraphicElement::ImageFrame(image_frame) => image_frame.contains_artboard(),
			GraphicElement::GraphicGroup(graphic_group) => graphic_group.contains_artboard(),
			GraphicElement::Surface(surface) => surface.contains_artboard(),
		}
	}
}

/// Used to stop rust complaining about upstream traits adding display implementations to `Option<Color>`. This would not be an issue as we control that crate.
trait Primitive: core::fmt::Display {}
impl Primitive for String {}
impl Primitive for bool {}
impl Primitive for f32 {}
impl Primitive for f64 {}
impl Primitive for DVec2 {}

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

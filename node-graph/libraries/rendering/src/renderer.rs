use crate::render_ext::RenderExt;
use crate::to_peniko::BlendModeExt;
use core_types::blending::BlendMode;
use core_types::bounds::BoundingBox;
use core_types::bounds::RenderBoundingBox;
use core_types::color::Color;
use core_types::math::quad::Quad;
use core_types::render_complexity::RenderComplexity;
use core_types::table::{Table, TableRow};
use core_types::transform::{Footprint, Transform};
use core_types::uuid::{NodeId, generate_uuid};
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};
use graphic_types::Vector;
use graphic_types::raster_types::BitmapMut;
use graphic_types::raster_types::Image;
use graphic_types::raster_types::{CPU, GPU, Raster};
use graphic_types::vector_types::gradient::GradientStops;
use graphic_types::vector_types::gradient::GradientType;
use graphic_types::vector_types::subpath::Subpath;
use graphic_types::vector_types::vector::click_target::{ClickTarget, FreePoint};
use graphic_types::vector_types::vector::style::{Fill, PaintOrder, RenderMode, Stroke, StrokeAlign};
use graphic_types::{Artboard, Graphic};
use kurbo::Affine;
use kurbo::Shape;
use num_traits::Zero;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::{Arc, LazyLock};
use vello::*;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
enum MaskType {
	Clip,
	Mask,
}

impl MaskType {
	fn to_attribute(self) -> String {
		match self {
			Self::Mask => "mask".to_string(),
			Self::Clip => "clip-path".to_string(),
		}
	}

	fn write_to_defs(self, svg_defs: &mut String, uuid: u64, svg_string: String) {
		let id = format!("mask-{uuid}");
		match self {
			Self::Clip => write!(svg_defs, r##"<clipPath id="{id}">{svg_string}</clipPath>"##).unwrap(),
			Self::Mask => write!(svg_defs, r##"<mask id="{id}" mask-type="alpha">{svg_string}</mask>"##).unwrap(),
		}
	}
}

/// Mutable state used whilst rendering to an SVG
pub struct SvgRender {
	pub svg: Vec<SvgSegment>,
	pub svg_defs: String,
	pub transform: DAffine2,
	pub image_data: HashMap<Image<Color>, u64>,
	indent: usize,
}

impl SvgRender {
	pub fn new() -> Self {
		Self {
			svg: Vec::default(),
			svg_defs: String::new(),
			transform: DAffine2::IDENTITY,
			image_data: HashMap::new(),
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
		let svg_header = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:graphite="https://graphite.art" viewBox="{x} {y} {size_x} {size_y}"><defs>{defs}</defs>"#,);
		self.svg.insert(0, svg_header.into());
		self.svg.push("</svg>".into());
	}

	/// Wraps the SVG with `<svg><g transform="...">...</g></svg>`, which allows for rotation
	pub fn wrap_with_transform(&mut self, transform: DAffine2, size: Option<DVec2>) {
		let defs = &self.svg_defs;
		let view_box = size
			.map(|size| format!("viewBox=\"0 0 {} {}\" width=\"{}\" height=\"{}\"", size.x, size.y, size.x, size.y))
			.unwrap_or_default();

		let matrix = format_transform_matrix(transform);
		let transform = if matrix.is_empty() { String::new() } else { format!(r#" transform="{matrix}""#) };

		let svg_header = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:graphite="https://graphite.art" {view_box}><defs>{defs}</defs><g{transform}>"#);
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

#[derive(Clone, Debug, Default)]
pub struct RenderContext {
	pub resource_overrides: Vec<(peniko::ImageBrush, wgpu::Texture)>,
}

#[derive(Default, Clone, Copy, Hash)]
pub enum RenderOutputType {
	#[default]
	Svg,
	Vello,
}

/// Static state used whilst rendering
#[derive(Default, Clone)]
pub struct RenderParams {
	pub render_mode: RenderMode,
	pub footprint: Footprint,
	/// Ratio of physical pixels to logical pixels. `scale := physical_pixels / logical_pixels`
	/// Ignored when rendering to SVG.
	pub scale: f64,
	pub render_output_type: RenderOutputType,
	pub thumbnail: bool,
	/// Don't render the rectangle for an artboard to allow exporting with a transparent background.
	pub hide_artboards: bool,
	/// Are we exporting
	pub for_export: bool,
	/// Are we generating a mask in this render pass? Used to see if fill should be multiplied with alpha.
	pub for_mask: bool,
	/// Are we generating a mask for alignment? Used to prevent unnecessary transforms in masks
	pub alignment_parent_transform: Option<DAffine2>,
	pub aligned_strokes: bool,
	pub override_paint_order: bool,
	pub artboard_background: Option<Color>,
}

impl Hash for RenderParams {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.render_mode.hash(state);
		self.footprint.hash(state);
		self.render_output_type.hash(state);
		self.thumbnail.hash(state);
		self.hide_artboards.hash(state);
		self.for_export.hash(state);
		self.for_mask.hash(state);
		if let Some(x) = self.alignment_parent_transform {
			x.to_cols_array().iter().for_each(|x| x.to_bits().hash(state))
		}
		self.aligned_strokes.hash(state);
		self.override_paint_order.hash(state);
		self.artboard_background.hash(state);
	}
}

impl RenderParams {
	pub fn for_clipper(&self) -> Self {
		Self { for_mask: true, ..*self }
	}

	pub fn for_alignment(&self, transform: DAffine2) -> Self {
		let alignment_parent_transform = Some(transform);
		Self { alignment_parent_transform, ..*self }
	}

	pub fn to_canvas(&self) -> bool {
		!self.for_export && !self.thumbnail && !self.for_mask
	}
}

pub fn format_transform_matrix(transform: DAffine2) -> String {
	if transform == DAffine2::IDENTITY {
		return String::new();
	}

	transform.to_cols_array().iter().enumerate().fold("matrix(".to_string(), |val, (i, num)| {
		let num = if num.abs() < 1_000_000_000. { (num * 1_000_000_000.).round() / 1_000_000_000. } else { *num };
		let num = if num.is_zero() { "0".to_string() } else { num.to_string() };
		let comma = if i == 5 { "" } else { "," };
		val + &(num + comma)
	}) + ")"
}

fn max_scale(transform: DAffine2) -> f64 {
	let sx = transform.x_axis.length_squared();
	let sy = transform.y_axis.length_squared();
	(sx + sy).sqrt()
}

pub fn black_or_white_for_best_contrast(background: Option<Color>) -> Color {
	let Some(bg) = background else { return core_types::consts::LAYER_OUTLINE_STROKE_COLOR };

	let alpha = bg.a();

	// Un-premultiply, then convert to gamma sRGB
	let srgb = if alpha > f32::EPSILON {
		Color::from_rgbaf32_unchecked(bg.r() / alpha, bg.g() / alpha, bg.b() / alpha, alpha).to_gamma_srgb()
	} else {
		Color::TRANSPARENT
	};

	// Composite over black in sRGB space, then convert back to linear for luminance
	let composited = Color::from_rgbaf32_unchecked(srgb.r() * alpha, srgb.g() * alpha, srgb.b() * alpha, 1.).to_linear_srgb();

	let threshold = (1.05 * 0.05f32).sqrt() - 0.05;

	if composited.luminance_srgb() > threshold { Color::BLACK } else { Color::WHITE }
}

pub fn to_transform(transform: DAffine2) -> usvg::Transform {
	let cols = transform.to_cols_array();
	usvg::Transform::from_row(cols[0] as f32, cols[1] as f32, cols[2] as f32, cols[3] as f32, cols[4] as f32, cols[5] as f32)
}

// TODO: Click targets can be removed from the render output, since the vector data is available in the vector modify data from Monitor nodes.
// This will require that the transform for child layers into that layer space be calculated, or it could be returned from the RenderOutput instead of click targets.
#[derive(Debug, Default, Clone, PartialEq, DynAny, serde::Serialize, serde::Deserialize)]
pub struct RenderMetadata {
	pub upstream_footprints: HashMap<NodeId, Footprint>,
	pub local_transforms: HashMap<NodeId, DAffine2>,
	pub first_element_source_id: HashMap<NodeId, Option<NodeId>>,
	pub click_targets: HashMap<NodeId, Vec<Arc<ClickTarget>>>,
	pub clip_targets: HashSet<NodeId>,
	pub vector_data: HashMap<NodeId, Arc<Vector>>,
}

impl RenderMetadata {
	pub fn apply_transform(&mut self, transform: DAffine2) {
		for value in self.upstream_footprints.values_mut() {
			value.transform = transform * value.transform;
		}
	}

	/// Merge another RenderMetadata into this one.
	/// Values from `other` take precedence for duplicate keys.
	pub fn merge(&mut self, other: &RenderMetadata) {
		// Destructure Self to get errors when new fields are added to the struct
		let RenderMetadata {
			upstream_footprints,
			local_transforms,
			first_element_source_id,
			click_targets,
			clip_targets,
			vector_data,
		} = self;
		upstream_footprints.extend(other.upstream_footprints.iter());
		local_transforms.extend(other.local_transforms.iter());
		first_element_source_id.extend(other.first_element_source_id.iter());
		click_targets.extend(other.click_targets.iter().map(|(k, v)| (*k, v.clone())));
		clip_targets.extend(other.clip_targets.iter());
		vector_data.extend(other.vector_data.iter().map(|(id, data)| (*id, data.clone())));
	}
}

// TODO: Rename to "Graphical"
pub trait Render: BoundingBox + RenderComplexity {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams);

	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2, context: &mut RenderContext, _render_params: &RenderParams);

	/// The upstream click targets for each layer are collected during the render so that they do not have to be calculated for each click detection.
	fn add_upstream_click_targets(&self, _click_targets: &mut Vec<ClickTarget>) {}

	// TODO: Store all click targets in a vec which contains the AABB, click target, and path
	// fn add_click_targets(&self, click_targets: &mut Vec<([DVec2; 2], ClickTarget, Vec<NodeId>)>, current_path: Option<NodeId>) {}

	/// Recursively iterate over data in the render (including nested layer stacks upstream of a vector node, in the case of a boolean operation) to collect the footprints, click targets, and vector modify.
	fn collect_metadata(&self, _metadata: &mut RenderMetadata, _footprint: Footprint, _element_id: Option<NodeId>) {}

	fn contains_artboard(&self) -> bool {
		false
	}

	fn new_ids_from_hash(&mut self, _reference: Option<NodeId>) {}
}

impl Render for Graphic {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		match self {
			Graphic::Graphic(table) => table.render_svg(render, render_params),
			Graphic::Vector(table) => table.render_svg(render, render_params),
			Graphic::RasterCPU(table) => table.render_svg(render, render_params),
			Graphic::RasterGPU(_) => (),
			Graphic::Color(table) => table.render_svg(render, render_params),
			Graphic::Gradient(table) => table.render_svg(render, render_params),
		}
	}

	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2, context: &mut RenderContext, render_params: &RenderParams) {
		match self {
			Graphic::Graphic(table) => table.render_to_vello(scene, transform, context, render_params),
			Graphic::Vector(table) => table.render_to_vello(scene, transform, context, render_params),
			Graphic::RasterCPU(table) => table.render_to_vello(scene, transform, context, render_params),
			Graphic::RasterGPU(table) => table.render_to_vello(scene, transform, context, render_params),
			Graphic::Color(table) => table.render_to_vello(scene, transform, context, render_params),
			Graphic::Gradient(table) => table.render_to_vello(scene, transform, context, render_params),
		}
	}

	fn collect_metadata(&self, metadata: &mut RenderMetadata, footprint: Footprint, element_id: Option<NodeId>) {
		if let Some(element_id) = element_id {
			match self {
				Graphic::Graphic(_) => {
					metadata.upstream_footprints.insert(element_id, footprint);
				}
				Graphic::Vector(table) => {
					metadata.upstream_footprints.insert(element_id, footprint);
					// TODO: Find a way to handle more than the first row
					if let Some(row) = table.iter().next() {
						metadata.first_element_source_id.insert(element_id, *row.source_node_id);
						metadata.local_transforms.insert(element_id, *row.transform);
					}
				}
				Graphic::RasterCPU(table) => {
					metadata.upstream_footprints.insert(element_id, footprint);

					// TODO: Find a way to handle more than the first row
					if let Some(row) = table.iter().next() {
						metadata.local_transforms.insert(element_id, *row.transform);
					}
				}
				Graphic::RasterGPU(table) => {
					metadata.upstream_footprints.insert(element_id, footprint);

					// TODO: Find a way to handle more than the first row
					if let Some(row) = table.iter().next() {
						metadata.local_transforms.insert(element_id, *row.transform);
					}
				}
				Graphic::Color(table) => {
					metadata.upstream_footprints.insert(element_id, footprint);

					// TODO: Find a way to handle more than the first row
					if let Some(row) = table.iter().next() {
						metadata.local_transforms.insert(element_id, *row.transform);
					}
				}
				Graphic::Gradient(table) => {
					metadata.upstream_footprints.insert(element_id, footprint);

					// TODO: Find a way to handle more than the first row
					if let Some(row) = table.iter().next() {
						metadata.local_transforms.insert(element_id, *row.transform);
					}
				}
			}
		}

		match self {
			Graphic::Graphic(table) => table.collect_metadata(metadata, footprint, element_id),
			Graphic::Vector(table) => table.collect_metadata(metadata, footprint, element_id),
			Graphic::RasterCPU(table) => table.collect_metadata(metadata, footprint, element_id),
			Graphic::RasterGPU(table) => table.collect_metadata(metadata, footprint, element_id),
			Graphic::Color(table) => table.collect_metadata(metadata, footprint, element_id),
			Graphic::Gradient(table) => table.collect_metadata(metadata, footprint, element_id),
		}
	}

	fn add_upstream_click_targets(&self, click_targets: &mut Vec<ClickTarget>) {
		match self {
			Graphic::Graphic(table) => table.add_upstream_click_targets(click_targets),
			Graphic::Vector(table) => table.add_upstream_click_targets(click_targets),
			Graphic::RasterCPU(table) => table.add_upstream_click_targets(click_targets),
			Graphic::RasterGPU(table) => table.add_upstream_click_targets(click_targets),
			Graphic::Color(table) => table.add_upstream_click_targets(click_targets),
			Graphic::Gradient(table) => table.add_upstream_click_targets(click_targets),
		}
	}

	fn contains_artboard(&self) -> bool {
		match self {
			Graphic::Graphic(table) => table.contains_artboard(),
			Graphic::Vector(table) => table.contains_artboard(),
			Graphic::RasterCPU(table) => table.contains_artboard(),
			Graphic::RasterGPU(table) => table.contains_artboard(),
			Graphic::Color(table) => table.contains_artboard(),
			Graphic::Gradient(table) => table.contains_artboard(),
		}
	}

	fn new_ids_from_hash(&mut self, reference: Option<NodeId>) {
		match self {
			Graphic::Graphic(table) => table.new_ids_from_hash(reference),
			Graphic::Vector(table) => table.new_ids_from_hash(reference),
			Graphic::RasterCPU(_) => (),
			Graphic::RasterGPU(_) => (),
			Graphic::Color(_) => (),
			Graphic::Gradient(_) => (),
		}
	}
}

impl Render for Artboard {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		// Rectangle for the artboard
		if !render_params.hide_artboards {
			// Background
			render.leaf_tag("rect", |attributes| {
				attributes.push("fill", format!("#{}", self.background.to_rgb_hex_srgb_from_gamma()));
				if self.background.a() < 1. {
					attributes.push("fill-opacity", ((self.background.a() * 1000.).round() / 1000.).to_string());
				}
				attributes.push("x", self.location.x.min(self.location.x + self.dimensions.x).to_string());
				attributes.push("y", self.location.y.min(self.location.y + self.dimensions.y).to_string());
				attributes.push("width", self.dimensions.x.abs().to_string());
				attributes.push("height", self.dimensions.y.abs().to_string());
			});
		}

		// Artwork
		render.parent_tag(
			// SVG group tag
			"g",
			// Group tag attributes
			|attributes| {
				let matrix = format_transform_matrix(self.transform());
				if !matrix.is_empty() {
					attributes.push("transform", matrix);
				}

				if self.clip {
					let id = format!("artboard-{}", generate_uuid());
					let selector = format!("url(#{id})");

					write!(
						&mut attributes.0.svg_defs,
						r##"<clipPath id="{id}"><rect x="0" y="0" width="{}" height="{}"/></clipPath>"##,
						self.dimensions.x, self.dimensions.y,
					)
					.unwrap();
					attributes.push("clip-path", selector);
				}
			},
			// Artwork content
			|render| {
				let mut render_params = render_params.clone();
				render_params.artboard_background = Some(self.background);
				self.content.render_svg(render, &render_params);
			},
		);
	}

	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2, context: &mut RenderContext, render_params: &RenderParams) {
		use vello::peniko;

		// Render background
		let color = peniko::Color::new([self.background.r(), self.background.g(), self.background.b(), self.background.a()]);
		let [a, b] = [self.location.as_dvec2(), self.location.as_dvec2() + self.dimensions.as_dvec2()];
		let rect = kurbo::Rect::new(a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y));

		scene.push_layer(peniko::Fill::NonZero, peniko::Mix::Normal, 1., kurbo::Affine::new(transform.to_cols_array()), &rect);
		scene.fill(peniko::Fill::NonZero, kurbo::Affine::new(transform.to_cols_array()), color, None, &rect);
		scene.pop_layer();

		if self.clip {
			scene.push_clip_layer(peniko::Fill::NonZero, kurbo::Affine::new(transform.to_cols_array()), &rect);
		}
		// Since the content's transform is right multiplied in when rendering the content, we just need to right multiply by the artboard offset here.
		let child_transform = transform * DAffine2::from_translation(self.location.as_dvec2());
		let mut render_params = render_params.clone();
		render_params.artboard_background = Some(self.background);
		self.content.render_to_vello(scene, child_transform, context, &render_params);
		if self.clip {
			scene.pop_layer();
		}
	}

	fn collect_metadata(&self, metadata: &mut RenderMetadata, mut footprint: Footprint, element_id: Option<NodeId>) {
		if let Some(element_id) = element_id {
			let subpath = Subpath::new_rectangle(DVec2::ZERO, self.dimensions.as_dvec2());
			metadata.click_targets.insert(element_id, vec![ClickTarget::new_with_subpath(subpath, 0.).into()]);
			metadata.upstream_footprints.insert(element_id, footprint);
			metadata.local_transforms.insert(element_id, DAffine2::from_translation(self.location.as_dvec2()));
			if self.clip {
				metadata.clip_targets.insert(element_id);
			}
		}
		footprint.transform *= self.transform();
		self.content.collect_metadata(metadata, footprint, None);
	}

	fn add_upstream_click_targets(&self, click_targets: &mut Vec<ClickTarget>) {
		let subpath_rectangle = Subpath::new_rectangle(DVec2::ZERO, self.dimensions.as_dvec2());
		click_targets.push(ClickTarget::new_with_subpath(subpath_rectangle, 0.));
	}

	fn contains_artboard(&self) -> bool {
		true
	}
}

impl Render for Table<Artboard> {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		for artboard in self.iter() {
			artboard.element.render_svg(render, render_params);
		}
	}

	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2, context: &mut RenderContext, render_params: &RenderParams) {
		for row in self.iter() {
			row.element.render_to_vello(scene, transform, context, render_params);
		}
	}

	fn collect_metadata(&self, metadata: &mut RenderMetadata, footprint: Footprint, _element_id: Option<NodeId>) {
		for row in self.iter() {
			row.element.collect_metadata(metadata, footprint, *row.source_node_id);
		}
	}

	fn add_upstream_click_targets(&self, click_targets: &mut Vec<ClickTarget>) {
		for row in self.iter() {
			row.element.add_upstream_click_targets(click_targets);
		}
	}

	fn contains_artboard(&self) -> bool {
		self.iter().count() > 0
	}
}

impl Render for Table<Graphic> {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		let mut iter = self.iter().peekable();
		let mut mask_state = None;

		while let Some(row) = iter.next() {
			render.parent_tag(
				"g",
				|attributes| {
					let matrix = format_transform_matrix(*row.transform);
					if !matrix.is_empty() {
						attributes.push("transform", matrix);
					}

					let opacity = row.alpha_blending.opacity(render_params.for_mask);
					if opacity < 1. {
						attributes.push("opacity", opacity.to_string());
					}

					if row.alpha_blending.blend_mode != BlendMode::default() {
						attributes.push("style", row.alpha_blending.blend_mode.render());
					}

					let next_clips = iter.peek().is_some_and(|next_row| next_row.element.had_clip_enabled());

					if next_clips && mask_state.is_none() {
						let uuid = generate_uuid();
						let mask_type = if row.element.can_reduce_to_clip_path() { MaskType::Clip } else { MaskType::Mask };
						mask_state = Some((uuid, mask_type));
						let mut svg = SvgRender::new();
						row.element.render_svg(&mut svg, &render_params.for_clipper());

						write!(&mut attributes.0.svg_defs, r##"{}"##, svg.svg_defs).unwrap();
						mask_type.write_to_defs(&mut attributes.0.svg_defs, uuid, svg.svg.to_svg_string());
					} else if let Some((uuid, mask_type)) = mask_state {
						if !next_clips {
							mask_state = None;
						}

						let id = format!("mask-{uuid}");
						let selector = format!("url(#{id})");

						attributes.push(mask_type.to_attribute(), selector);
					}
				},
				|render| {
					row.element.render_svg(render, render_params);
				},
			);
		}
	}

	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2, context: &mut RenderContext, render_params: &RenderParams) {
		let mut iter = self.iter().peekable();
		let mut mask_element_and_transform = None;

		while let Some(row) = iter.next() {
			let transform = transform * *row.transform;
			let alpha_blending = *row.alpha_blending;

			let mut layer = false;

			let blend_mode = match render_params.render_mode {
				RenderMode::Outline => peniko::Mix::Normal,
				_ => alpha_blending.blend_mode.to_peniko(),
			};
			let mut bounds = RenderBoundingBox::None;

			let opacity = row.alpha_blending.opacity(render_params.for_mask);
			if opacity < 1. || (render_params.render_mode != RenderMode::Outline && alpha_blending.blend_mode != BlendMode::default()) {
				bounds = row.element.bounding_box(transform, true);

				if let RenderBoundingBox::Rectangle(bounds) = bounds {
					scene.push_layer(
						peniko::Fill::NonZero,
						peniko::BlendMode::new(blend_mode, peniko::Compose::SrcOver),
						opacity,
						kurbo::Affine::IDENTITY,
						&kurbo::Rect::new(bounds[0].x, bounds[0].y, bounds[1].x, bounds[1].y),
					);
					layer = true;
				}
			}

			let next_clips = iter.peek().is_some_and(|next_row| next_row.element.had_clip_enabled());
			if next_clips && mask_element_and_transform.is_none() {
				mask_element_and_transform = Some((row.element, transform));

				row.element.render_to_vello(scene, transform, context, render_params);
			} else if let Some((mask_element, transform_mask)) = mask_element_and_transform {
				if !next_clips {
					mask_element_and_transform = None;
				}
				if !layer {
					bounds = row.element.bounding_box(transform, true);
				}

				if let RenderBoundingBox::Rectangle(bounds) = bounds {
					let rect = kurbo::Rect::new(bounds[0].x, bounds[0].y, bounds[1].x, bounds[1].y);

					scene.push_layer(peniko::Fill::NonZero, peniko::Mix::Normal, 1., kurbo::Affine::IDENTITY, &rect);
					mask_element.render_to_vello(scene, transform_mask, context, &render_params.for_clipper());
					scene.push_layer(
						peniko::Fill::NonZero,
						peniko::BlendMode::new(peniko::Mix::Normal, peniko::Compose::SrcIn),
						1.,
						kurbo::Affine::IDENTITY,
						&rect,
					);
				}

				row.element.render_to_vello(scene, transform, context, render_params);

				if matches!(bounds, RenderBoundingBox::Rectangle(_)) {
					scene.pop_layer();
					scene.pop_layer();
				}
			} else {
				row.element.render_to_vello(scene, transform, context, render_params);
			}

			if layer {
				scene.pop_layer();
			}
		}
	}

	fn collect_metadata(&self, metadata: &mut RenderMetadata, footprint: Footprint, element_id: Option<NodeId>) {
		for row in self.iter() {
			if let Some(element_id) = row.source_node_id {
				let mut footprint = footprint;
				footprint.transform *= *row.transform;

				row.element.collect_metadata(metadata, footprint, Some(*element_id));
			}
		}

		if let Some(element_id) = element_id {
			let mut all_upstream_click_targets = Vec::new();

			for row in self.iter() {
				let mut new_click_targets = Vec::new();
				row.element.add_upstream_click_targets(&mut new_click_targets);

				for click_target in new_click_targets.iter_mut() {
					click_target.apply_transform(*row.transform)
				}

				all_upstream_click_targets.extend(new_click_targets);
			}

			metadata.click_targets.insert(element_id, all_upstream_click_targets.into_iter().map(|x| x.into()).collect());
		}
	}

	fn add_upstream_click_targets(&self, click_targets: &mut Vec<ClickTarget>) {
		for row in self.iter() {
			let mut new_click_targets = Vec::new();

			row.element.add_upstream_click_targets(&mut new_click_targets);

			for click_target in new_click_targets.iter_mut() {
				click_target.apply_transform(*row.transform)
			}

			click_targets.extend(new_click_targets);
		}
	}

	fn contains_artboard(&self) -> bool {
		self.iter().any(|row| row.element.contains_artboard())
	}

	fn new_ids_from_hash(&mut self, _reference: Option<NodeId>) {
		for row in self.iter_mut() {
			row.element.new_ids_from_hash(*row.source_node_id);
		}
	}
}

impl Render for Table<Vector> {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		for row in self.iter() {
			let multiplied_transform = *row.transform;
			let vector = &row.element;
			// Only consider strokes with non-zero weight, since default strokes with zero weight would prevent assigning the correct stroke transform
			let has_real_stroke = vector.style.stroke().filter(|stroke| stroke.weight() > 0.);
			let set_stroke_transform = has_real_stroke.map(|stroke| stroke.transform).filter(|transform| transform.matrix2.determinant() != 0.);
			let applied_stroke_transform = set_stroke_transform.unwrap_or(*row.transform);
			let applied_stroke_transform = render_params.alignment_parent_transform.unwrap_or(applied_stroke_transform);
			let element_transform = set_stroke_transform.map(|stroke_transform| multiplied_transform * stroke_transform.inverse());
			let element_transform = element_transform.unwrap_or(DAffine2::IDENTITY);
			let layer_bounds = vector.bounding_box().unwrap_or_default();
			let transformed_bounds = vector.bounding_box_with_transform(applied_stroke_transform).unwrap_or_default();

			let bounds_matrix = DAffine2::from_scale_angle_translation(layer_bounds[1] - layer_bounds[0], 0., layer_bounds[0]);
			let transformed_bounds_matrix = element_transform * DAffine2::from_scale_angle_translation(transformed_bounds[1] - transformed_bounds[0], 0., transformed_bounds[0]);

			let mut path = String::new();

			for mut bezpath in row.element.stroke_bezpath_iter() {
				bezpath.apply_affine(Affine::new(applied_stroke_transform.to_cols_array()));
				path.push_str(bezpath.to_svg().as_str());
			}

			let mask_type = if vector.style.stroke().map(|x| x.align) == Some(StrokeAlign::Inside) {
				MaskType::Clip
			} else {
				MaskType::Mask
			};

			let path_is_closed = vector.stroke_bezier_paths().all(|path| path.closed());
			let can_draw_aligned_stroke = path_is_closed && vector.style.stroke().is_some_and(|stroke| stroke.has_renderable_stroke() && stroke.align.is_not_centered());
			let can_use_paint_order = !(row.element.style.fill().is_none() || !row.element.style.fill().is_opaque() || mask_type == MaskType::Clip);

			let needs_separate_alignment_fill = can_draw_aligned_stroke && !can_use_paint_order;
			let wants_stroke_below = vector.style.stroke().map(|s| s.paint_order) == Some(PaintOrder::StrokeBelow);

			if needs_separate_alignment_fill && !wants_stroke_below {
				render.leaf_tag("path", |attributes| {
					attributes.push("d", path.clone());
					let matrix = format_transform_matrix(element_transform);
					if !matrix.is_empty() {
						attributes.push("transform", matrix);
					}
					let mut style = row.element.style.clone();
					style.clear_stroke();
					let fill_and_stroke = style.render(
						&mut attributes.0.svg_defs,
						element_transform,
						applied_stroke_transform,
						bounds_matrix,
						transformed_bounds_matrix,
						render_params,
					);
					attributes.push_val(fill_and_stroke);
				});
			}

			let push_id = needs_separate_alignment_fill.then_some({
				let id = format!("alignment-{}", generate_uuid());

				let mut element = row.element.clone();
				element.style.clear_stroke();
				element.style.set_fill(Fill::solid(Color::BLACK));

				let vector_row = Table::new_from_row(TableRow {
					element,
					alpha_blending: *row.alpha_blending,
					transform: *row.transform,
					source_node_id: None,
				});

				(id, mask_type, vector_row)
			});

			if vector.is_branching() {
				for mut face_path in vector.construct_faces().filter(|face| !(face.area() < 0.0)) {
					face_path.apply_affine(Affine::new(applied_stroke_transform.to_cols_array()));

					let face_d = face_path.to_svg();
					render.leaf_tag("path", |attributes| {
						attributes.push("d", face_d.clone());
						let matrix = format_transform_matrix(element_transform);
						if !matrix.is_empty() {
							attributes.push("transform", matrix);
						}
						let mut style = row.element.style.clone();
						style.clear_stroke();
						let fill_only = style.render(
							&mut attributes.0.svg_defs,
							element_transform,
							applied_stroke_transform,
							bounds_matrix,
							transformed_bounds_matrix,
							render_params,
						);
						attributes.push_val(fill_only);
					});
				}
			}

			render.leaf_tag("path", |attributes| {
				attributes.push("d", path.clone());
				let matrix = format_transform_matrix(element_transform);
				if !matrix.is_empty() {
					attributes.push("transform", matrix);
				}

				let defs = &mut attributes.0.svg_defs;
				if let Some((ref id, mask_type, ref vector_row)) = push_id {
					let mut svg = SvgRender::new();
					vector_row.render_svg(&mut svg, &render_params.for_alignment(applied_stroke_transform));
					let stroke = row.element.style.stroke().unwrap();
					let weight = stroke.effective_width() * max_scale(applied_stroke_transform);
					let quad = Quad::from_box(transformed_bounds).inflate(weight);
					let (x, y) = quad.top_left().into();
					let (width, height) = (quad.bottom_right() - quad.top_left()).into();

					write!(defs, r##"{}"##, svg.svg_defs).unwrap();
					let rect = format!(r##"<rect x="{x}" y="{y}" width="{width}" height="{height}" fill="white" />"##);

					match mask_type {
						MaskType::Clip => write!(defs, r##"<clipPath id="{id}">{}</clipPath>"##, svg.svg.to_svg_string()).unwrap(),
						MaskType::Mask => write!(
							defs,
							r##"<mask id="{id}" maskUnits="userSpaceOnUse" maskContentUnits="userSpaceOnUse" x="{x}" y="{y}" width="{width}" height="{height}">{}{}</mask>"##,
							rect,
							svg.svg.to_svg_string()
						)
						.unwrap(),
					}
				}

				let mut render_params = render_params.clone();
				render_params.aligned_strokes = can_draw_aligned_stroke;
				render_params.override_paint_order = can_draw_aligned_stroke && can_use_paint_order;

				let mut style = row.element.style.clone();
				if needs_separate_alignment_fill || vector.is_branching() {
					style.clear_fill();
				}

				let fill_and_stroke = style.render(defs, element_transform, applied_stroke_transform, bounds_matrix, transformed_bounds_matrix, &render_params);

				if let Some((id, mask_type, _)) = push_id {
					let selector = format!("url(#{id})");
					attributes.push(mask_type.to_attribute(), selector);
				}
				attributes.push_val(fill_and_stroke);

				let opacity = row.alpha_blending.opacity(render_params.for_mask);
				if opacity < 1. {
					attributes.push("opacity", opacity.to_string());
				}

				if row.alpha_blending.blend_mode != BlendMode::default() {
					attributes.push("style", row.alpha_blending.blend_mode.render());
				}
			});

			// When splitting passes and stroke is below, draw the fill after the stroke.
			if needs_separate_alignment_fill && wants_stroke_below {
				render.leaf_tag("path", |attributes| {
					attributes.push("d", path);
					let matrix = format_transform_matrix(element_transform);
					if !matrix.is_empty() {
						attributes.push("transform", matrix);
					}
					let mut style = row.element.style.clone();
					style.clear_stroke();
					let fill_and_stroke = style.render(
						&mut attributes.0.svg_defs,
						element_transform,
						applied_stroke_transform,
						bounds_matrix,
						transformed_bounds_matrix,
						render_params,
					);
					attributes.push_val(fill_and_stroke);
				});
			}
		}
	}

	fn render_to_vello(&self, scene: &mut Scene, parent_transform: DAffine2, _context: &mut RenderContext, render_params: &RenderParams) {
		use core_types::consts::LAYER_OUTLINE_STROKE_WEIGHT;
		use graphic_types::vector_types::vector::style::{GradientType, StrokeCap, StrokeJoin};
		use vello::kurbo::{Cap, Join};
		use vello::peniko;

		for row in self.iter() {
			use graphic_types::vector_types::vector;

			let multiplied_transform = parent_transform * *row.transform;
			let has_real_stroke = row.element.style.stroke().filter(|stroke| stroke.weight() > 0.);
			let set_stroke_transform = has_real_stroke.map(|stroke| stroke.transform).filter(|transform| transform.matrix2.determinant() != 0.);
			let mut applied_stroke_transform = set_stroke_transform.unwrap_or(multiplied_transform);
			let mut element_transform = set_stroke_transform
				.map(|stroke_transform| multiplied_transform * stroke_transform.inverse())
				.unwrap_or(DAffine2::IDENTITY);
			if let Some(alignment_transform) = render_params.alignment_parent_transform {
				applied_stroke_transform = alignment_transform;
				element_transform = if alignment_transform.matrix2.determinant() != 0. {
					multiplied_transform * alignment_transform.inverse()
				} else {
					multiplied_transform
				};
			}
			let layer_bounds = row.element.bounding_box().unwrap_or_default();

			let to_point = |p: DVec2| kurbo::Point::new(p.x, p.y);
			let mut path = kurbo::BezPath::new();
			for mut bezpath in row.element.stroke_bezpath_iter() {
				bezpath.apply_affine(Affine::new(applied_stroke_transform.to_cols_array()));
				for element in bezpath {
					path.push(element);
				}
			}

			// If we're using opacity or a blend mode, we need to push a layer
			let blend_mode = match render_params.render_mode {
				RenderMode::Outline => peniko::Mix::Normal,
				_ => row.alpha_blending.blend_mode.to_peniko(),
			};
			let mut layer = false;

			let opacity = row.alpha_blending.opacity(render_params.for_mask);
			if opacity < 1. || row.alpha_blending.blend_mode != BlendMode::default() {
				layer = true;
				let weight = row.element.style.stroke().as_ref().map_or(0., Stroke::effective_width);
				let quad = Quad::from_box(layer_bounds).inflate(weight * max_scale(applied_stroke_transform));
				let layer_bounds = quad.bounding_box();
				scene.push_layer(
					peniko::Fill::NonZero,
					peniko::BlendMode::new(blend_mode, peniko::Compose::SrcOver),
					opacity,
					kurbo::Affine::new(multiplied_transform.to_cols_array()),
					&kurbo::Rect::new(layer_bounds[0].x, layer_bounds[0].y, layer_bounds[1].x, layer_bounds[1].y),
				);
			}

			let can_draw_aligned_stroke =
				row.element.style.stroke().is_some_and(|stroke| stroke.has_renderable_stroke() && stroke.align.is_not_centered()) && row.element.stroke_bezier_paths().all(|path| path.closed());

			let use_layer = can_draw_aligned_stroke;
			let wants_stroke_below = row.element.style.stroke().is_some_and(|s| s.paint_order == vector::style::PaintOrder::StrokeBelow);

			// Closures to avoid duplicated fill/stroke drawing logic
			let do_fill_path = |scene: &mut Scene, path: &kurbo::BezPath| match row.element.style.fill() {
				Fill::Solid(color) => {
					let fill = peniko::Brush::Solid(peniko::Color::new([color.r(), color.g(), color.b(), color.a()]));
					scene.fill(peniko::Fill::NonZero, kurbo::Affine::new(element_transform.to_cols_array()), &fill, None, path);
				}
				Fill::Gradient(gradient) => {
					let mut stops = peniko::ColorStops::new();
					for (position, color, _) in gradient.stops.interpolated_samples() {
						stops.push(peniko::ColorStop {
							offset: position as f32,
							color: peniko::color::DynamicColor::from_alpha_color(peniko::Color::new([color.r(), color.g(), color.b(), color.a()])),
						});
					}

					let bounds = row.element.nonzero_bounding_box();
					let bound_transform = DAffine2::from_scale_angle_translation(bounds[1] - bounds[0], 0., bounds[0]);

					let inverse_parent_transform = if parent_transform.matrix2.determinant() != 0. {
						parent_transform.inverse()
					} else {
						Default::default()
					};
					let mod_points = inverse_parent_transform * multiplied_transform * bound_transform;

					let start = mod_points.transform_point2(gradient.start);
					let end = mod_points.transform_point2(gradient.end);

					let fill = peniko::Brush::Gradient(peniko::Gradient {
						kind: match gradient.gradient_type {
							GradientType::Linear => peniko::LinearGradientPosition {
								start: to_point(start),
								end: to_point(end),
							}
							.into(),
							GradientType::Radial => {
								let radius = start.distance(end);
								peniko::RadialGradientPosition {
									start_center: to_point(start),
									start_radius: 0.,
									end_center: to_point(start),
									end_radius: radius as f32,
								}
								.into()
							}
						},
						stops,
						interpolation_alpha_space: peniko::InterpolationAlphaSpace::Premultiplied,
						..Default::default()
					});
					let inverse_element_transform = if element_transform.matrix2.determinant() != 0. {
						element_transform.inverse()
					} else {
						Default::default()
					};
					let brush_transform = kurbo::Affine::new((inverse_element_transform * parent_transform).to_cols_array());
					scene.fill(peniko::Fill::NonZero, kurbo::Affine::new(element_transform.to_cols_array()), &fill, Some(brush_transform), path);
				}
				Fill::None => {}
			};

			let do_fill = |scene: &mut Scene| {
				if row.element.is_branching() {
					// For branching paths, fill each face separately
					for mut face_path in row.element.construct_faces().filter(|face| !(face.area() < 0.0)) {
						face_path.apply_affine(Affine::new(applied_stroke_transform.to_cols_array()));
						let mut kurbo_path = kurbo::BezPath::new();
						for element in face_path {
							kurbo_path.push(element);
						}
						do_fill_path(scene, &kurbo_path);
					}
				} else {
					// Simple fill of the entire path
					do_fill_path(scene, &path);
				}
			};

			let do_stroke = |scene: &mut Scene, width_scale: f64| {
				if let Some(stroke) = row.element.style.stroke() {
					let color = match stroke.color {
						Some(color) => peniko::Color::new([color.r(), color.g(), color.b(), color.a()]),
						None => peniko::Color::TRANSPARENT,
					};
					let cap = match stroke.cap {
						StrokeCap::Butt => Cap::Butt,
						StrokeCap::Round => Cap::Round,
						StrokeCap::Square => Cap::Square,
					};
					let join = match stroke.join {
						StrokeJoin::Miter => Join::Miter,
						StrokeJoin::Bevel => Join::Bevel,
						StrokeJoin::Round => Join::Round,
					};
					let dash_pattern = stroke.dash_lengths.iter().map(|l| l.max(0.)).collect();
					let stroke = kurbo::Stroke {
						width: stroke.weight * width_scale,
						miter_limit: stroke.join_miter_limit,
						join,
						start_cap: cap,
						end_cap: cap,
						dash_pattern,
						dash_offset: stroke.dash_offset,
					};

					if stroke.width > 0. {
						scene.stroke(&stroke, kurbo::Affine::new(element_transform.to_cols_array()), color, None, &path);
					}
				}
			};

			// Render the path
			match render_params.render_mode {
				RenderMode::Outline => {
					let outline_stroke = kurbo::Stroke {
						width: LAYER_OUTLINE_STROKE_WEIGHT,
						miter_limit: 4.,
						join: Join::Miter,
						start_cap: Cap::Butt,
						end_cap: Cap::Butt,
						dash_pattern: Default::default(),
						dash_offset: 0.,
					};

					let outline_color = black_or_white_for_best_contrast(render_params.artboard_background);
					let outline_color = peniko::Color::new([outline_color.r(), outline_color.g(), outline_color.b(), outline_color.a()]);

					scene.stroke(&outline_stroke, kurbo::Affine::new(element_transform.to_cols_array()), outline_color, None, &path);
				}
				_ => {
					if use_layer {
						let mut element = row.element.clone();
						element.style.clear_stroke();
						element.style.set_fill(Fill::solid(Color::BLACK));

						let vector_table = Table::new_from_row(TableRow {
							element,
							alpha_blending: *row.alpha_blending,
							transform: *row.transform,
							source_node_id: None,
						});

						let bounds = row.element.bounding_box_with_transform(multiplied_transform).unwrap_or(layer_bounds);
						let weight = row.element.style.stroke().as_ref().map_or(0., Stroke::effective_width);
						let quad = Quad::from_box(bounds).inflate(weight * max_scale(applied_stroke_transform));
						let bounds = quad.bounding_box();
						let rect = kurbo::Rect::new(bounds[0].x, bounds[0].y, bounds[1].x, bounds[1].y);

						let compose = if row.element.style.stroke().is_some_and(|x| x.align == StrokeAlign::Outside) {
							peniko::Compose::SrcOut
						} else {
							peniko::Compose::SrcIn
						};

						if wants_stroke_below {
							scene.push_layer(peniko::Fill::NonZero, peniko::Mix::Normal, 1., kurbo::Affine::IDENTITY, &rect);
							vector_table.render_to_vello(scene, parent_transform, _context, &render_params.for_alignment(applied_stroke_transform));
							scene.push_layer(peniko::Fill::NonZero, peniko::BlendMode::new(peniko::Mix::Normal, compose), 1., kurbo::Affine::IDENTITY, &rect);

							do_stroke(scene, 2.);

							scene.pop_layer();
							scene.pop_layer();

							do_fill(scene);
						} else {
							// Fill first (unclipped), then stroke (clipped) above
							do_fill(scene);

							scene.push_layer(peniko::Fill::NonZero, peniko::Mix::Normal, 1., kurbo::Affine::IDENTITY, &rect);
							vector_table.render_to_vello(scene, parent_transform, _context, &render_params.for_alignment(applied_stroke_transform));
							scene.push_layer(peniko::Fill::NonZero, peniko::BlendMode::new(peniko::Mix::Normal, compose), 1., kurbo::Affine::IDENTITY, &rect);

							do_stroke(scene, 2.);

							scene.pop_layer();
							scene.pop_layer();
						}
					} else {
						// Non-aligned strokes or open paths: default order behavior
						enum Op {
							Fill,
							Stroke,
						}

						let order = match row.element.style.stroke().is_some_and(|stroke| !stroke.paint_order.is_default()) {
							true => [Op::Stroke, Op::Fill],
							false => [Op::Fill, Op::Stroke], // Default
						};

						for operation in &order {
							match operation {
								Op::Fill => do_fill(scene),
								Op::Stroke => do_stroke(scene, 1.),
							}
						}
					}
				}
			}

			// If we pushed a layer for opacity or a blend mode, we need to pop it
			if layer {
				scene.pop_layer();
			}
		}
	}

	fn collect_metadata(&self, metadata: &mut RenderMetadata, mut footprint: Footprint, element_id: Option<NodeId>) {
		for row in self.iter() {
			let transform = *row.transform;
			let vector = row.element;

			if let Some(element_id) = element_id {
				let stroke_width = vector.style.stroke().as_ref().map_or(0., Stroke::effective_width);
				let filled = vector.style.fill() != &Fill::None;
				let fill = |mut subpath: Subpath<_>| {
					if filled {
						subpath.set_closed(true);
					}
					subpath
				};

				// For free-floating anchors, we need to add a click target for each
				let single_anchors_targets = vector.point_domain.ids().iter().filter_map(|&point_id| {
					if !vector.any_connected(point_id) {
						let anchor = vector.point_domain.position_from_id(point_id).unwrap_or_default();
						let point = FreePoint::new(point_id, anchor);

						Some(ClickTarget::new_with_free_point(point).into())
					} else {
						None
					}
				});

				let click_targets = vector
					.stroke_bezier_paths()
					.map(fill)
					.map(|subpath| ClickTarget::new_with_subpath(subpath, stroke_width).into())
					.chain(single_anchors_targets.into_iter())
					.collect::<Vec<_>>();

				metadata.click_targets.entry(element_id).or_insert(click_targets);
				// Store the full vector data including segment IDs for accurate segment modification
				metadata.vector_data.entry(element_id).or_insert_with(|| Arc::new(vector.clone()));
			}

			if let Some(upstream_nested_layers) = &vector.upstream_data {
				footprint.transform *= transform;
				upstream_nested_layers.collect_metadata(metadata, footprint, None);
			}
		}
	}

	fn add_upstream_click_targets(&self, click_targets: &mut Vec<ClickTarget>) {
		for row in self.iter() {
			let stroke_width = row.element.style.stroke().as_ref().map_or(0., Stroke::effective_width);
			let filled = row.element.style.fill() != &Fill::None;
			let fill = |mut subpath: Subpath<_>| {
				if filled {
					subpath.set_closed(true);
				}
				subpath
			};
			click_targets.extend(row.element.stroke_bezier_paths().map(fill).map(|subpath| {
				let mut click_target = ClickTarget::new_with_subpath(subpath, stroke_width);
				click_target.apply_transform(*row.transform);
				click_target
			}));

			// For free-floating anchors, we need to add a click target for each
			let single_anchors_targets = row.element.point_domain.ids().iter().filter_map(|&point_id| {
				if row.element.any_connected(point_id) {
					return None;
				}

				let anchor = row.element.point_domain.position_from_id(point_id).unwrap_or_default();
				let point = FreePoint::new(point_id, anchor);

				let mut click_target = ClickTarget::new_with_free_point(point);
				click_target.apply_transform(*row.transform);
				Some(click_target)
			});
			click_targets.extend(single_anchors_targets);
		}
	}

	fn new_ids_from_hash(&mut self, reference: Option<NodeId>) {
		for row in self.iter_mut() {
			row.element.vector_new_ids_from_hash(reference.map(|id| id.0).unwrap_or_default());
		}
	}
}

impl Render for Table<Raster<CPU>> {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		for row in self.iter() {
			let image = row.element;

			let transform = *row.transform;

			if image.data.is_empty() {
				continue;
			}

			if render_params.to_canvas() {
				let mut image_copy = image.clone();
				image_copy.data_mut().map_pixels(|p| p.to_unassociated_alpha());
				let id = *render.image_data.entry(image_copy.into_data()).or_insert_with(generate_uuid);

				render.parent_tag(
					"foreignObject",
					|attributes| {
						let mut transform_values = transform.to_scale_angle_translation();
						let size = DVec2::new(image.width as f64, image.height as f64);
						transform_values.0 /= size;

						let matrix = DAffine2::from_scale_angle_translation(transform_values.0, transform_values.1, transform_values.2);
						let matrix = format_transform_matrix(matrix);
						if !matrix.is_empty() {
							attributes.push("transform", matrix);
						}

						attributes.push("width", size.x.to_string());
						attributes.push("height", size.y.to_string());

						let opacity = row.alpha_blending.opacity(render_params.for_mask);
						if opacity < 1. {
							attributes.push("opacity", opacity.to_string());
						}

						if row.alpha_blending.blend_mode != BlendMode::default() {
							attributes.push("style", row.alpha_blending.blend_mode.render());
						}
					},
					|render| {
						render.leaf_tag(
							"img", // Must be a self-closing (void element) tag, so we can't use `div` or `span`, for example
							|attributes| {
								attributes.push("data-canvas-placeholder", id.to_string());
							},
						)
					},
				);
			} else {
				let base64_string = image.base64_string.clone().unwrap_or_else(|| {
					use base64::Engine;

					let output = image.to_png();
					let preamble = "data:image/png;base64,";
					let mut base64_string = String::with_capacity(preamble.len() + output.len() * 4);
					base64_string.push_str(preamble);
					base64::engine::general_purpose::STANDARD.encode_string(output, &mut base64_string);
					base64_string
				});

				render.leaf_tag("image", |attributes| {
					attributes.push("width", "1");
					attributes.push("height", "1");
					attributes.push("preserveAspectRatio", "none");
					attributes.push("href", base64_string);
					let matrix = format_transform_matrix(transform);
					if !matrix.is_empty() {
						attributes.push("transform", matrix);
					}

					let opacity = row.alpha_blending.opacity(render_params.for_mask);
					if opacity < 1. {
						attributes.push("opacity", opacity.to_string());
					}
					if row.alpha_blending.blend_mode != BlendMode::default() {
						attributes.push("style", row.alpha_blending.blend_mode.render());
					}
				});
			}
		}
	}

	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2, _: &mut RenderContext, render_params: &RenderParams) {
		use vello::peniko;

		for row in self.iter() {
			let image = &row.element;
			if image.data.is_empty() {
				continue;
			}

			let alpha_blending = *row.alpha_blending;
			let blend_mode = alpha_blending.blend_mode.to_peniko();

			let opacity = alpha_blending.opacity(render_params.for_mask);
			let mut layer = false;

			if (opacity < 1. || alpha_blending.blend_mode != BlendMode::default())
				&& let RenderBoundingBox::Rectangle(bounds) = self.bounding_box(transform, false)
			{
				let blending = peniko::BlendMode::new(blend_mode, peniko::Compose::SrcOver);
				let rect = kurbo::Rect::new(bounds[0].x, bounds[0].y, bounds[1].x, bounds[1].y);
				scene.push_layer(peniko::Fill::NonZero, blending, opacity, kurbo::Affine::IDENTITY, &rect);
				layer = true;
			}

			let image_brush = peniko::ImageBrush::new(peniko::ImageData {
				data: image.to_flat_u8().0.into(),
				format: peniko::ImageFormat::Rgba8,
				width: image.width,
				height: image.height,
				alpha_type: peniko::ImageAlphaType::Alpha,
			})
			.with_extend(peniko::Extend::Repeat);
			let image_transform = transform * *row.transform * DAffine2::from_scale(1. / DVec2::new(image.width as f64, image.height as f64));

			scene.draw_image(&image_brush, kurbo::Affine::new(image_transform.to_cols_array()));

			if layer {
				scene.pop_layer();
			}
		}
	}

	fn collect_metadata(&self, metadata: &mut RenderMetadata, footprint: Footprint, element_id: Option<NodeId>) {
		let Some(element_id) = element_id else { return };
		let subpath = Subpath::new_rectangle(DVec2::ZERO, DVec2::ONE);

		metadata.click_targets.insert(element_id, vec![ClickTarget::new_with_subpath(subpath, 0.).into()]);
		metadata.upstream_footprints.insert(element_id, footprint);
		// TODO: Find a way to handle more than one row of the raster table
		if let Some(raster) = self.iter().next() {
			metadata.local_transforms.insert(element_id, *raster.transform);
		}
	}

	fn add_upstream_click_targets(&self, click_targets: &mut Vec<ClickTarget>) {
		let subpath = Subpath::new_rectangle(DVec2::ZERO, DVec2::ONE);
		click_targets.push(ClickTarget::new_with_subpath(subpath, 0.));
	}
}

static LAZY_ARC_VEC_ZERO_U8: LazyLock<Arc<Vec<u8>>> = LazyLock::new(|| Arc::new(Vec::new()));

impl Render for Table<Raster<GPU>> {
	fn render_svg(&self, _render: &mut SvgRender, _render_params: &RenderParams) {
		log::warn!("tried to render texture as an svg");
	}

	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2, context: &mut RenderContext, _render_params: &RenderParams) {
		use vello::peniko;

		for row in self.iter() {
			let blend_mode = *row.alpha_blending;
			let mut layer = false;
			if blend_mode != Default::default()
				&& let RenderBoundingBox::Rectangle(bounds) = self.bounding_box(transform, true)
			{
				let blending = peniko::BlendMode::new(blend_mode.blend_mode.to_peniko(), peniko::Compose::SrcOver);
				let rect = kurbo::Rect::new(bounds[0].x, bounds[0].y, bounds[1].x, bounds[1].y);
				scene.push_layer(peniko::Fill::NonZero, blending, blend_mode.opacity, kurbo::Affine::IDENTITY, &rect);
				layer = true;
			}

			let width = row.element.data().width();
			let height = row.element.data().height();
			let image = peniko::ImageBrush::new(peniko::ImageData {
				data: peniko::Blob::new(LAZY_ARC_VEC_ZERO_U8.deref().clone()),
				format: peniko::ImageFormat::Rgba8,
				width,
				height,
				alpha_type: peniko::ImageAlphaType::Alpha,
			})
			.with_extend(peniko::Extend::Repeat);
			let image_transform = transform * *row.transform * DAffine2::from_scale(1. / DVec2::new(width as f64, height as f64));
			scene.draw_image(&image, kurbo::Affine::new(image_transform.to_cols_array()));
			context.resource_overrides.push((image, row.element.data().clone()));

			if layer {
				scene.pop_layer()
			}
		}
	}

	fn collect_metadata(&self, metadata: &mut RenderMetadata, footprint: Footprint, element_id: Option<NodeId>) {
		let Some(element_id) = element_id else { return };
		let subpath = Subpath::new_rectangle(DVec2::ZERO, DVec2::ONE);

		metadata.click_targets.insert(element_id, vec![ClickTarget::new_with_subpath(subpath, 0.).into()]);
		metadata.upstream_footprints.insert(element_id, footprint);
		// TODO: Find a way to handle more than one row of the raster table
		if let Some(raster) = self.iter().next() {
			metadata.local_transforms.insert(element_id, *raster.transform);
		}
	}

	fn add_upstream_click_targets(&self, click_targets: &mut Vec<ClickTarget>) {
		let subpath = Subpath::new_rectangle(DVec2::ZERO, DVec2::ONE);
		click_targets.push(ClickTarget::new_with_subpath(subpath, 0.));
	}
}

// Since colors and gradients are technically infinitely big, we have to implement
// workarounds for rendering them correctly in a way which still allows us
// to cache the intermediate render data (SVG string/Vello scene).
// For SVG, this is is achived by creating a truly giant rectangle.
// For Vello, we create a layer with a placeholder transform which we
// later replace with the current viewport transform before each render.
impl Render for Table<Color> {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		for row in self.iter() {
			render.leaf_tag("polyline", |attributes| {
				// Chrome doesn't like drawing centered rectangles bigger than ~20 million so we draw a polyline quad instead
				let max = u64::MAX;
				attributes.push("points", format!("{max},{max} -{max},{max} -{max},-{max} {max},-{max}"));

				let color = row.element;
				attributes.push("fill", format!("#{}", color.to_rgb_hex_srgb_from_gamma()));
				if color.a() < 1. {
					attributes.push("fill-opacity", ((color.a() * 1000.).round() / 1000.).to_string());
				}

				let opacity = row.alpha_blending.opacity(render_params.for_mask);
				if opacity < 1. {
					attributes.push("opacity", opacity.to_string());
				}

				if row.alpha_blending.blend_mode != BlendMode::default() {
					attributes.push("style", row.alpha_blending.blend_mode.render());
				}
			});
		}
	}

	fn render_to_vello(&self, scene: &mut Scene, _parent_transform: DAffine2, _context: &mut RenderContext, render_params: &RenderParams) {
		use vello::peniko;

		for row in self.iter() {
			let alpha_blending = *row.alpha_blending;
			let blend_mode = alpha_blending.blend_mode.to_peniko();
			let opacity = alpha_blending.opacity(render_params.for_mask);

			let color = row.element;
			let vello_color = peniko::Color::new([color.r(), color.g(), color.b(), color.a()]);

			let rect = kurbo::Rect::from_origin_size(kurbo::Point::ZERO, kurbo::Size::new(1., 1.));

			let mut layer = false;
			if opacity < 1. || alpha_blending.blend_mode != BlendMode::default() {
				let blending = peniko::BlendMode::new(blend_mode, peniko::Compose::SrcOver);
				scene.push_layer(peniko::Fill::NonZero, blending, opacity, kurbo::Affine::scale(f64::INFINITY), &rect);
				layer = true;
			}

			scene.fill(peniko::Fill::NonZero, kurbo::Affine::scale(f64::INFINITY), vello_color, None, &rect);

			if layer {
				scene.pop_layer();
			}
		}
	}
}

impl Render for Table<GradientStops> {
	// TODO: Fix infinite gradient rendering
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		for row in self.iter() {
			render.leaf_tag("rect", |attributes| {
				// Chrome doesn't like drawing centered rectangles bigger than ~20 million so we draw a polyline quad instead
				let max = u64::MAX;
				attributes.push("points", format!("{max},{max} -{max},{max} -{max},-{max} {max},-{max}"));

				let mut stop_string = String::new();
				for (position, color, original_midpoint) in row.element.interpolated_samples() {
					let _ = write!(stop_string, r##"<stop offset="{}" stop-color="#{}""##, position, color.to_rgb_hex_srgb_from_gamma());
					if color.a() < 1. {
						let _ = write!(stop_string, r#" stop-opacity="{}""#, color.a());
					}
					if let Some(midpoint) = original_midpoint {
						let _ = write!(stop_string, r#" graphite:midpoint="{}""#, (midpoint * 1000.).round() / 1000.);
					}
					stop_string.push_str(" />");
				}

				let gradient_transform = render_params.footprint.transform * *row.transform;
				let gradient_transform_matrix = format_transform_matrix(gradient_transform);
				let gradient_transform_attribute = if gradient_transform_matrix.is_empty() {
					String::new()
				} else {
					format!(r#" gradientTransform="{gradient_transform_matrix}""#)
				};

				let gradient_id = generate_uuid();
				let start = DVec2::ZERO;
				let end = DVec2::X;

				match GradientType::Radial {
					GradientType::Linear => {
						let (x1, y1) = (start.x, start.y);
						let (x2, y2) = (end.x, end.y);
						let _ = write!(
							&mut attributes.0.svg_defs,
							r#"<linearGradient id="{gradient_id}" gradientUnits="userSpaceOnUse" x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}"{gradient_transform_attribute}>{stop_string}</linearGradient>"#
						);
					}
					GradientType::Radial => {
						let (cx, cy) = (start.x, start.y);
						let r = start.distance(end);
						let _ = write!(
							&mut attributes.0.svg_defs,
							r#"<radialGradient id="{gradient_id}" gradientUnits="userSpaceOnUse" cx="{cx}" cy="{cy}" r="{r}"{gradient_transform_attribute}>{stop_string}</radialGradient>"#
						);
					}
				}

				attributes.push("fill", format!("url('#{gradient_id}')"));

				let opacity = row.alpha_blending.opacity(render_params.for_mask);
				if opacity < 1. {
					attributes.push("opacity", opacity.to_string());
				}

				if row.alpha_blending.blend_mode != BlendMode::default() {
					attributes.push("style", row.alpha_blending.blend_mode.render());
				}
			});
		}
	}

	// TODO: Fix infinite gradient rendering
	fn render_to_vello(&self, scene: &mut Scene, _parent_transform: DAffine2, _context: &mut RenderContext, render_params: &RenderParams) {
		use vello::peniko;

		for row in self.iter() {
			let alpha_blending = *row.alpha_blending;
			let blend_mode = alpha_blending.blend_mode.to_peniko();
			let opacity = alpha_blending.opacity(render_params.for_mask);

			let color = row.element.color.first().copied().unwrap_or(Color::MAGENTA);
			let vello_color = peniko::Color::new([color.r(), color.g(), color.b(), color.a()]);

			let rect = kurbo::Rect::from_origin_size(kurbo::Point::ZERO, kurbo::Size::new(1., 1.));

			let mut layer = false;
			if opacity < 1. || alpha_blending.blend_mode != BlendMode::default() {
				let blending = peniko::BlendMode::new(blend_mode, peniko::Compose::SrcOver);
				// See implemenation in `Table<Color>` for more detail
				scene.push_layer(peniko::Fill::NonZero, blending, opacity, kurbo::Affine::scale(f64::INFINITY), &rect);
				layer = true;
			}

			scene.fill(peniko::Fill::NonZero, kurbo::Affine::scale(f64::INFINITY), vello_color, None, &rect);

			if layer {
				scene.pop_layer();
			}
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SvgSegment {
	Slice(&'static str),
	String(String),
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
			});
		}
		result
	}
}

pub struct SvgRenderAttrs<'a>(&'a mut SvgRender);

impl SvgRenderAttrs<'_> {
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

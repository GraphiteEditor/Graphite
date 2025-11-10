```rust
use crate::render_ext::RenderExt;
use crate::to_peniko::BlendModeExt;
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};
use graphene_core::blending::BlendMode;
use graphene_core::bounds::BoundingBox;
use graphene_core::bounds::RenderBoundingBox;
use graphene_core::color::Color;
use graphene_core::gradient::GradientStops;
use graphene_core::gradient::GradientType;
use graphene_core::math::quad::Quad;
use graphene_core::raster::BitmapMut;
use graphene_core::raster::Image;
use graphene_core::raster_types::{CPU, GPU, Raster};
use graphene_core::render_complexity::RenderComplexity;
use graphene_core::subpath::Subpath;
use graphene_core::table::{Table, TableRow};
use graphene_core::transform::{Footprint, Transform};
use graphene_core::uuid::{NodeId, generate_uuid};
use graphene_core::vector::Vector;
use graphene_core::vector::click_target::{ClickTarget, FreePoint};
use graphene_core::vector::style::{Fill, PaintOrder, RenderMode, Stroke, StrokeAlign};
use graphene_core::{Artboard, Graphic};
use kurbo::Affine;
use num_traits::Zero;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::Deref;
use std::sync::{Arc, LazyLock};
#[cfg(feature = "vello")]
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
			.map(|size| format!("viewBox=\"0 0 {} {}\" width=\"{}\" height=\"{}\"", size.x, size.y, size.x, size.y))
			.unwrap_or_default();

		let matrix = format_transform_matrix(transform);
		let transform = if matrix.is_empty() { String::new() } else { format!(r#" transform="{matrix}""#) };

		let svg_header = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" {view_box}><defs>{defs}</defs><g{transform}>"#);
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
	#[cfg(feature = "vello")]
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
	pub click_targets: HashMap<NodeId, Vec<ClickTarget>>,
	pub clip_targets: HashSet<NodeId>,
}

impl RenderMetadata {
	pub fn apply_transform(&mut self, transform: DAffine2) {
		for value in self.upstream_footprints.values_mut() {
			value.transform = transform * value.transform;
		}
	}
}

// TODO: Rename to "Graphical"
pub trait Render: BoundingBox + RenderComplexity {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams);

	#[cfg(feature = "vello")]
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

	#[cfg(feature = "vello")]
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
					if let Some(row) = table.iter()
        ```
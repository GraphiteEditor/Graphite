use crate::render_ext::RenderExt;
use crate::to_peniko::BlendModeExt;
use core_types::CacheHash;
use core_types::blending::BlendMode;
use core_types::bounds::BoundingBox;
use core_types::bounds::RenderBoundingBox;
use core_types::color::Color;
use core_types::math::quad::Quad;
use core_types::render_complexity::RenderComplexity;
use core_types::table::{Table, TableRow};
use core_types::transform::Footprint;
use core_types::uuid::{NodeId, generate_uuid};
use core_types::{
	ATTR_BACKGROUND, ATTR_BLEND_MODE, ATTR_CLIP, ATTR_CLIPPING_MASK, ATTR_DIMENSIONS, ATTR_EDITOR_CLICK_TARGET, ATTR_EDITOR_LAYER_PATH, ATTR_EDITOR_MERGED_LAYERS, ATTR_EDITOR_TEXT_FRAME,
	ATTR_GRADIENT_TYPE, ATTR_LOCATION, ATTR_OPACITY, ATTR_OPACITY_FILL, ATTR_SPREAD_METHOD, ATTR_TRANSFORM,
};
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};
use graphene_hash::CacheHashWrapper;
use graphic_types::raster_types::{BitmapMut, CPU, GPU, Image, Raster};
use graphic_types::vector_types::gradient::{GradientStops, GradientType};
use graphic_types::vector_types::subpath::Subpath;
use graphic_types::vector_types::vector::click_target::{ClickTarget, FreePoint};
use graphic_types::vector_types::vector::style::{Fill, PaintOrder, RenderMode, Stroke, StrokeAlign};
use graphic_types::{Artboard, Graphic, Vector};
use kurbo::{Affine, Cap, Join, Shape};
use num_traits::Zero;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::ops::Deref;
use std::sync::{Arc, LazyLock};
use vector_types::gradient::GradientSpreadMethod;
use vello::*;

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
	pub image_data: HashMap<CacheHashWrapper<Image<Color>>, u64>,
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
		let svg_header = format!(
			r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:graphite="https://graphite.art" viewBox="{x} {y} {size_x} {size_y}"><defs>{defs}</defs>"#,
			defs = &self.svg_defs
		);
		self.svg_defs = String::new();
		self.svg.insert(0, svg_header.into());
		self.svg.push("</svg>".into());
	}

	/// Wraps the SVG with `<svg><g transform="...">...</g></svg>`, which allows for rotation
	pub fn wrap_with_transform(&mut self, transform: DAffine2, size: Option<DVec2>) {
		let view_box = size
			.map(|size| format!("viewBox=\"0 0 {} {}\" width=\"{}\" height=\"{}\"", size.x, size.y, size.x, size.y))
			.unwrap_or_default();

		let matrix = format_transform_matrix(transform);
		let transform = if matrix.is_empty() { String::new() } else { format!(r#" transform="{matrix}""#) };

		let svg_header = format!(
			r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:graphite="https://graphite.art" {view_box}><defs>{defs}</defs><g{transform}>"#,
			defs = &self.svg_defs
		);
		self.svg_defs = String::new();
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

pub struct SvgRenderOutput {
	pub svg: String,
	pub svg_defs: String,
	pub image_data: HashMap<CacheHashWrapper<Image<Color>>, u64>,
}

impl From<&SvgRenderOutput> for SvgRender {
	fn from(value: &SvgRenderOutput) -> Self {
		Self {
			svg: vec![value.svg.clone().into()],
			svg_defs: value.svg_defs.clone(),
			transform: DAffine2::IDENTITY,
			image_data: value.image_data.clone(),
			indent: 0,
		}
	}
}

impl From<SvgRender> for SvgRenderOutput {
	fn from(val: SvgRender) -> Self {
		Self {
			svg: val.svg.to_svg_string(),
			svg_defs: val.svg_defs,
			image_data: val.image_data,
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

#[derive(Default, Clone, Copy, Hash, graphene_hash::CacheHash)]
pub enum RenderOutputType {
	#[default]
	Svg,
	Vello,
}

/// Static state used whilst rendering
#[derive(Default, Clone, CacheHash)]
pub struct RenderParams {
	pub render_mode: RenderMode,
	pub footprint: Footprint,
	/// Ratio of physical pixels to logical pixels. `scale := physical_pixels / logical_pixels`
	/// Ignored when rendering to SVG.
	#[cache_hash(skip)]
	pub scale: f64,
	pub render_output_type: RenderOutputType,
	pub thumbnail: bool,
	/// Are we exporting
	pub for_export: bool,
	/// Are we generating a mask in this render pass? Used to see if fill should be multiplied with alpha.
	pub for_mask: bool,
	/// Are we generating a mask for alignment? Used to prevent unnecessary transforms in masks
	pub alignment_parent_transform: Option<DAffine2>,
	pub aligned_strokes: bool,
	pub override_paint_order: bool,
	pub artboard_background: Option<Color>,
	/// Viewport zoom level (document-space scale). Used to compute constant viewport-pixel stroke widths in Outline mode.
	pub viewport_zoom: f64,
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

fn axial_max_scale(transform: DAffine2) -> f64 {
	transform.x_axis.length().max(transform.y_axis.length())
}

fn axial_min_scale(transform: DAffine2) -> f64 {
	transform.x_axis.length().min(transform.y_axis.length())
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

fn to_point(p: DVec2) -> kurbo::Point {
	kurbo::Point::new(p.x, p.y)
}

fn get_outline_styles(render_params: &RenderParams) -> (kurbo::Stroke, peniko::Color) {
	use core_types::consts::LAYER_OUTLINE_STROKE_WEIGHT;

	let outline_stroke = kurbo::Stroke {
		width: LAYER_OUTLINE_STROKE_WEIGHT / if render_params.viewport_zoom > 0. { render_params.viewport_zoom } else { 1. },
		miter_limit: 4.,
		join: Join::Miter,
		start_cap: Cap::Butt,
		end_cap: Cap::Butt,
		dash_pattern: Default::default(),
		dash_offset: 0.,
	};

	let outline_color = black_or_white_for_best_contrast(render_params.artboard_background);
	let outline_color_peniko = peniko::Color::new([outline_color.r(), outline_color.g(), outline_color.b(), outline_color.a()]);

	(outline_stroke, outline_color_peniko)
}

fn draw_raster_outline(scene: &mut Scene, outline_transform: &DAffine2, render_params: &RenderParams) {
	use graphic_types::vector_types::vector::PointId;

	let (outline_stroke, outline_color_peniko) = get_outline_styles(render_params);

	let mut outline_path = Subpath::<PointId>::new_rectangle(DVec2::ZERO, DVec2::ONE).to_bezpath();
	outline_path.apply_affine(Affine::new(outline_transform.to_cols_array()));

	scene.stroke(&outline_stroke, Affine::IDENTITY, outline_color_peniko, None, &outline_path);
}

// TODO: Click targets can be removed from the render output, since the vector data is available in the vector modify data from Monitor nodes.
// This will require that the transform for child layers into that layer space be calculated, or it could be returned from the RenderOutput instead of click targets.
#[derive(Debug, Default, Clone, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RenderMetadata {
	pub upstream_footprints: HashMap<NodeId, Footprint>,
	pub local_transforms: HashMap<NodeId, DAffine2>,
	pub first_element_source_id: HashMap<NodeId, Option<NodeId>>,
	pub click_targets: HashMap<NodeId, Vec<Arc<ClickTarget>>>,
	/// Source-geometry outlines for hover/selection overlays, separate from `click_targets` so
	/// nodes with an `editor:click_target` override still outline the precise geometry.
	pub outlines: HashMap<NodeId, Vec<Arc<ClickTarget>>>,
	/// Per-layer text frame from row 0's `editor:text_frame` attribute.
	/// The Text tool composes this with `transform_to_viewport(layer)` to position its drag cage.
	pub text_frames: HashMap<NodeId, DAffine2>,
	pub clip_targets: HashSet<NodeId>,
	pub vector_data: HashMap<NodeId, Arc<Vector>>,
	pub backgrounds: Vec<Background>,
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
			outlines,
			text_frames,
			clip_targets,
			vector_data,
			backgrounds,
		} = self;
		upstream_footprints.extend(other.upstream_footprints.iter());
		local_transforms.extend(other.local_transforms.iter());
		first_element_source_id.extend(other.first_element_source_id.iter());
		click_targets.extend(other.click_targets.iter().map(|(k, v)| (*k, v.clone())));
		outlines.extend(other.outlines.iter().map(|(k, v)| (*k, v.clone())));
		text_frames.extend(other.text_frames.iter());
		clip_targets.extend(other.clip_targets.iter());
		vector_data.extend(other.vector_data.iter().map(|(id, data)| (*id, data.clone())));

		// TODO: Find a better non O(n^2) way to merge backgrounds
		for background in &other.backgrounds {
			if !backgrounds.contains(background) {
				backgrounds.push(background.clone());
			}
		}
	}
}

#[derive(Debug, Default, Clone, PartialEq, DynAny, serde::Serialize, serde::Deserialize)]
pub struct Background {
	pub location: DVec2,
	pub dimensions: DVec2,
}

// TODO: Rename to "Graphical"
pub trait Render: BoundingBox + RenderComplexity {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams);

	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2, context: &mut RenderContext, _render_params: &RenderParams);

	/// The upstream click targets for each layer are collected during the render so that they do not have to be calculated for each click detection.
	fn add_upstream_click_targets(&self, _click_targets: &mut Vec<ClickTarget>) {}

	/// Like `add_upstream_click_targets` but for visual outlines. `Table<Vector>` overrides this to ignore `editor:click_target` so outlines reflect the actual geometry.
	fn add_upstream_outline_targets(&self, outlines: &mut Vec<ClickTarget>) {
		self.add_upstream_click_targets(outlines);
	}

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
					// TODO: Find a way to handle more than the first item
					if !table.is_empty() {
						let layer_path: Table<NodeId> = table.attribute_cloned_or_default(ATTR_EDITOR_LAYER_PATH, 0);
						let layer = layer_path.iter_element_values().next_back().copied();
						let transform: DAffine2 = table.attribute_cloned_or_default(ATTR_TRANSFORM, 0);

						metadata.first_element_source_id.insert(element_id, layer);
						metadata.local_transforms.insert(element_id, transform);
					}
				}
				Graphic::RasterCPU(table) => {
					metadata.upstream_footprints.insert(element_id, footprint);

					// TODO: Find a way to handle more than the first item
					if !table.is_empty() {
						metadata.local_transforms.insert(element_id, table.attribute_cloned_or_default(ATTR_TRANSFORM, 0));
					}
				}
				Graphic::RasterGPU(table) => {
					metadata.upstream_footprints.insert(element_id, footprint);

					// TODO: Find a way to handle more than the first item
					if !table.is_empty() {
						metadata.local_transforms.insert(element_id, table.attribute_cloned_or_default(ATTR_TRANSFORM, 0));
					}
				}
				Graphic::Color(table) => {
					metadata.upstream_footprints.insert(element_id, footprint);

					// TODO: Find a way to handle more than the first item
					if !table.is_empty() {
						metadata.local_transforms.insert(element_id, table.attribute_cloned_or_default(ATTR_TRANSFORM, 0));
					}
				}
				Graphic::Gradient(table) => {
					metadata.upstream_footprints.insert(element_id, footprint);

					// TODO: Find a way to handle more than the first item
					if !table.is_empty() {
						metadata.local_transforms.insert(element_id, table.attribute_cloned_or_default(ATTR_TRANSFORM, 0));
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

	fn add_upstream_outline_targets(&self, outlines: &mut Vec<ClickTarget>) {
		match self {
			Graphic::Graphic(table) => table.add_upstream_outline_targets(outlines),
			Graphic::Vector(table) => table.add_upstream_outline_targets(outlines),
			Graphic::RasterCPU(table) => table.add_upstream_outline_targets(outlines),
			Graphic::RasterGPU(table) => table.add_upstream_outline_targets(outlines),
			Graphic::Color(table) => table.add_upstream_outline_targets(outlines),
			Graphic::Gradient(table) => table.add_upstream_outline_targets(outlines),
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

/// Reads the artboard metadata for the item at `index` from a `Table<Artboard>`.
fn read_artboard_attributes(table: &Table<Artboard>, index: usize) -> (DVec2, DVec2, Color, bool) {
	let location: DVec2 = table.attribute_cloned_or_default(ATTR_LOCATION, index);
	let dimensions: DVec2 = table.attribute_cloned_or_default(ATTR_DIMENSIONS, index);
	let background: Color = table.attribute_cloned_or_default(ATTR_BACKGROUND, index);
	let clip: bool = table.attribute_cloned_or_default(ATTR_CLIP, index);
	(location, dimensions, background, clip)
}

impl Render for Table<Artboard> {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		for index in 0..self.len() {
			let Some(content) = self.element(index).map(Artboard::as_graphic_table) else { continue };
			let (location, dimensions, background, clip) = read_artboard_attributes(self, index);

			let x = location.x.min(location.x + dimensions.x);
			let y = location.y.min(location.y + dimensions.y);
			let width = dimensions.x.abs();
			let height = dimensions.y.abs();

			// Background
			render.leaf_tag("rect", |attributes| {
				attributes.push("fill", format!("#{}", background.to_rgb_hex_srgb_from_gamma()));
				if background.a() < 1. {
					attributes.push("fill-opacity", ((background.a() * 1000.).round() / 1000.).to_string());
				}
				attributes.push("x", x.to_string());
				attributes.push("y", y.to_string());
				attributes.push("width", width.to_string());
				attributes.push("height", height.to_string());
			});

			// Artwork
			render.parent_tag(
				// SVG group tag
				"g",
				// Group tag attributes
				|attributes| {
					let matrix = format_transform_matrix(DAffine2::from_translation(location));
					if !matrix.is_empty() {
						attributes.push(ATTR_TRANSFORM, matrix);
					}

					if clip {
						let id = format!("artboard-{}", generate_uuid());
						let selector = format!("url(#{id})");

						write!(
							&mut attributes.0.svg_defs,
							r##"<clipPath id="{id}"><rect x="0" y="0" width="{}" height="{}" /></clipPath>"##,
							dimensions.x, dimensions.y,
						)
						.unwrap();
						attributes.push("clip-path", selector);
					}
				},
				// Artwork content
				|render| {
					let mut render_params = render_params.clone();
					render_params.artboard_background = Some(background);
					content.render_svg(render, &render_params);
				},
			);
		}
	}

	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2, context: &mut RenderContext, render_params: &RenderParams) {
		use vello::peniko;

		for index in 0..self.len() {
			let Some(content) = self.element(index).map(Artboard::as_graphic_table) else { continue };
			let (location, dimensions, background, clip) = read_artboard_attributes(self, index);

			let [a, b] = [location, location + dimensions];
			let rect = kurbo::Rect::new(a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y));

			let artboard_transform = kurbo::Affine::new(transform.to_cols_array());

			let color = peniko::Color::new([background.r(), background.g(), background.b(), background.a()]);
			scene.push_layer(peniko::Fill::NonZero, peniko::Mix::Normal, 1., artboard_transform, &rect);
			scene.fill(peniko::Fill::NonZero, artboard_transform, color, None, &rect);
			scene.pop_layer();

			if clip {
				scene.push_clip_layer(peniko::Fill::NonZero, kurbo::Affine::new(transform.to_cols_array()), &rect);
			}

			// Since the content's transform is right multiplied in when rendering the content, we just need to right multiply by the artboard offset here.
			let child_transform = transform * DAffine2::from_translation(location);
			let mut render_params = render_params.clone();
			render_params.artboard_background = Some(background);
			content.render_to_vello(scene, child_transform, context, &render_params);
			if clip {
				scene.pop_layer();
			}
		}
	}

	fn collect_metadata(&self, metadata: &mut RenderMetadata, footprint: Footprint, _element_id: Option<NodeId>) {
		for index in 0..self.len() {
			let Some(content) = self.element(index).map(Artboard::as_graphic_table) else { continue };
			let (location, dimensions, _background, clip) = read_artboard_attributes(self, index);

			let layer_path: Table<NodeId> = self.attribute_cloned_or_default(ATTR_EDITOR_LAYER_PATH, index);
			let element_id = layer_path.iter_element_values().next_back().copied();

			if let Some(element_id) = element_id {
				let subpath = Subpath::new_rectangle(DVec2::ZERO, dimensions);
				metadata.click_targets.insert(element_id, vec![ClickTarget::new_with_subpath(subpath, 0.).into()]);
				metadata.upstream_footprints.insert(element_id, footprint);
				metadata.local_transforms.insert(element_id, DAffine2::from_translation(location));
				if clip {
					metadata.clip_targets.insert(element_id);
				}
			}

			metadata.backgrounds.push(Background { location, dimensions });

			let mut child_footprint = footprint;
			child_footprint.transform *= DAffine2::from_translation(location);
			content.collect_metadata(metadata, child_footprint, None);
		}
	}

	fn add_upstream_click_targets(&self, click_targets: &mut Vec<ClickTarget>) {
		for index in 0..self.len() {
			let dimensions: DVec2 = self.attribute_cloned_or_default(ATTR_DIMENSIONS, index);
			let subpath_rectangle = Subpath::new_rectangle(DVec2::ZERO, dimensions);
			click_targets.push(ClickTarget::new_with_subpath(subpath_rectangle, 0.));
		}
	}

	fn contains_artboard(&self) -> bool {
		!self.is_empty()
	}
}

impl Render for Table<Graphic> {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		let mut mask_state = None;

		for index in 0..self.len() {
			let transform: DAffine2 = self.attribute_cloned_or_default(ATTR_TRANSFORM, index);
			let blend_mode: BlendMode = self.attribute_cloned_or_default(ATTR_BLEND_MODE, index);
			let opacity_attr: f64 = self.attribute_cloned_or(ATTR_OPACITY, index, 1.);
			let opacity_fill_attr: f64 = self.attribute_cloned_or(ATTR_OPACITY_FILL, index, 1.);
			let element = self.element(index).unwrap();

			render.parent_tag(
				"g",
				|attributes| {
					let matrix = format_transform_matrix(transform);
					if !matrix.is_empty() {
						attributes.push(ATTR_TRANSFORM, matrix);
					}

					let opacity = (opacity_attr * if render_params.for_mask { 1. } else { opacity_fill_attr }) as f32;
					if opacity < 1. {
						attributes.push("opacity", opacity.to_string());
					}

					if blend_mode != BlendMode::default() {
						attributes.push("style", blend_mode.render());
					}

					let next_clips = index + 1 < self.len() && self.element(index + 1).unwrap().had_clip_enabled();

					if next_clips && mask_state.is_none() {
						let uuid = generate_uuid();
						let mask_type = if element.can_reduce_to_clip_path() { MaskType::Clip } else { MaskType::Mask };
						mask_state = Some((uuid, mask_type));
						let mut svg = SvgRender::new();
						element.render_svg(&mut svg, &render_params.for_clipper());

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
					element.render_svg(render, render_params);
				},
			);
		}
	}

	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2, context: &mut RenderContext, render_params: &RenderParams) {
		let mut mask_element_and_transform = None;

		for index in 0..self.len() {
			let item_transform: DAffine2 = self.attribute_cloned_or_default(ATTR_TRANSFORM, index);
			let transform = transform * item_transform;
			let blend_mode_attr: BlendMode = self.attribute_cloned_or_default(ATTR_BLEND_MODE, index);
			let opacity_attr: f64 = self.attribute_cloned_or(ATTR_OPACITY, index, 1.);
			let opacity_fill_attr: f64 = self.attribute_cloned_or(ATTR_OPACITY_FILL, index, 1.);
			let element = self.element(index).unwrap();

			let mut layer = false;

			let blend_mode = match render_params.render_mode {
				RenderMode::Outline => peniko::Mix::Normal,
				_ => blend_mode_attr.to_peniko(),
			};
			let mut bounds = RenderBoundingBox::None;

			let opacity = (opacity_attr * if render_params.for_mask { 1. } else { opacity_fill_attr }) as f32;
			if opacity < 1. || (render_params.render_mode != RenderMode::Outline && blend_mode_attr != BlendMode::default()) {
				bounds = element.bounding_box(transform, true);

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

			let next_clips = index + 1 < self.len() && self.element(index + 1).unwrap().had_clip_enabled();
			if next_clips && mask_element_and_transform.is_none() {
				mask_element_and_transform = Some((element, transform));

				element.render_to_vello(scene, transform, context, render_params);
			} else if let Some((mask_element, transform_mask)) = mask_element_and_transform {
				if !next_clips {
					mask_element_and_transform = None;
				}
				if !layer {
					bounds = element.bounding_box(transform, true);
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

				element.render_to_vello(scene, transform, context, render_params);

				if matches!(bounds, RenderBoundingBox::Rectangle(_)) {
					scene.pop_layer();
					scene.pop_layer();
				}
			} else {
				element.render_to_vello(scene, transform, context, render_params);
			}

			if layer {
				scene.pop_layer();
			}
		}
	}

	fn collect_metadata(&self, metadata: &mut RenderMetadata, footprint: Footprint, element_id: Option<NodeId>) {
		for index in 0..self.len() {
			let item_transform: DAffine2 = self.attribute_cloned_or_default(ATTR_TRANSFORM, index);
			let layer_path: Table<NodeId> = self.attribute_cloned_or_default(ATTR_EDITOR_LAYER_PATH, index);
			let layer = layer_path.iter_element_values().next_back().copied();
			let element = self.element(index).unwrap();

			let mut footprint = footprint;
			footprint.transform *= item_transform;

			if let Some(element_id) = layer {
				element.collect_metadata(metadata, footprint, Some(element_id));
			} else {
				// Recurse through anonymous wrapper items to reach nested content with editor:layer_path tags
				element.collect_metadata(metadata, footprint, None);
			}
		}

		if let Some(element_id) = element_id {
			let mut all_upstream_click_targets = Vec::new();
			let mut all_upstream_outlines = Vec::new();

			for index in 0..self.len() {
				let item_transform: DAffine2 = self.attribute_cloned_or_default(ATTR_TRANSFORM, index);
				let element = self.element(index).unwrap();

				let mut new_click_targets = Vec::new();
				element.add_upstream_click_targets(&mut new_click_targets);

				for click_target in new_click_targets.iter_mut() {
					click_target.apply_transform(item_transform)
				}

				all_upstream_click_targets.extend(new_click_targets);

				let mut new_outlines = Vec::new();
				element.add_upstream_outline_targets(&mut new_outlines);
				for outline in new_outlines.iter_mut() {
					outline.apply_transform(item_transform)
				}
				all_upstream_outlines.extend(new_outlines);
			}

			metadata.click_targets.insert(element_id, all_upstream_click_targets.into_iter().map(|x| x.into()).collect());
			metadata.outlines.insert(element_id, all_upstream_outlines.into_iter().map(|x| x.into()).collect());
		}
	}

	fn add_upstream_click_targets(&self, click_targets: &mut Vec<ClickTarget>) {
		for index in 0..self.len() {
			let item_transform: DAffine2 = self.attribute_cloned_or_default(ATTR_TRANSFORM, index);
			let element = self.element(index).unwrap();
			let mut new_click_targets = Vec::new();

			element.add_upstream_click_targets(&mut new_click_targets);

			for click_target in new_click_targets.iter_mut() {
				click_target.apply_transform(item_transform)
			}

			click_targets.extend(new_click_targets);
		}
	}

	fn add_upstream_outline_targets(&self, outlines: &mut Vec<ClickTarget>) {
		for index in 0..self.len() {
			let item_transform: DAffine2 = self.attribute_cloned_or_default(ATTR_TRANSFORM, index);
			let element = self.element(index).unwrap();
			let mut new_outlines = Vec::new();

			element.add_upstream_outline_targets(&mut new_outlines);

			for outline in new_outlines.iter_mut() {
				outline.apply_transform(item_transform)
			}

			outlines.extend(new_outlines);
		}
	}

	fn contains_artboard(&self) -> bool {
		self.iter_element_values().any(|element| element.contains_artboard())
	}

	fn new_ids_from_hash(&mut self, _reference: Option<NodeId>) {
		let (elements, layers) = self.element_and_attribute_slices_mut::<Table<NodeId>>(ATTR_EDITOR_LAYER_PATH);
		for (element, layer) in elements.iter_mut().zip(layers.iter()) {
			element.new_ids_from_hash(layer.iter_element_values().next_back().copied());
		}
	}
}

impl Render for Table<Vector> {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		for index in 0..self.len() {
			let Some(vector) = self.element(index) else { continue };
			let multiplied_transform: DAffine2 = self.attribute_cloned_or_default(ATTR_TRANSFORM, index);
			let blend_mode_attr: BlendMode = self.attribute_cloned_or_default(ATTR_BLEND_MODE, index);
			let opacity_attr: f64 = self.attribute_cloned_or(ATTR_OPACITY, index, 1.);
			let opacity_fill_attr: f64 = self.attribute_cloned_or(ATTR_OPACITY_FILL, index, 1.);
			let clipping_mask_attr: bool = self.attribute_cloned_or_default(ATTR_CLIPPING_MASK, index);

			// Only consider strokes with non-zero weight, since default strokes with zero weight would prevent assigning the correct stroke transform
			let has_real_stroke = vector.style.stroke().filter(|stroke| stroke.weight() > 0.);
			let set_stroke_transform = has_real_stroke.map(|stroke| stroke.transform).filter(|transform| transform.matrix2.determinant() != 0.);
			let applied_stroke_transform = set_stroke_transform.unwrap_or(multiplied_transform);
			let applied_stroke_transform = render_params.alignment_parent_transform.unwrap_or(applied_stroke_transform);
			let element_transform = set_stroke_transform.map(|stroke_transform| multiplied_transform * stroke_transform.inverse());
			let element_transform = element_transform.unwrap_or(DAffine2::IDENTITY);
			let layer_bounds = vector.bounding_box().unwrap_or_default();
			let transformed_bounds = vector.bounding_box_with_transform(applied_stroke_transform).unwrap_or_default();

			let bounds_matrix = DAffine2::from_scale_angle_translation(layer_bounds[1] - layer_bounds[0], 0., layer_bounds[0]);
			let transformed_bounds_matrix = element_transform * DAffine2::from_scale_angle_translation(transformed_bounds[1] - transformed_bounds[0], 0., transformed_bounds[0]);

			let mut path = String::new();

			for mut bezpath in vector.stroke_bezpath_iter() {
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
			let can_use_paint_order = !(vector.style.fill().is_none() || !vector.style.fill().is_opaque() || mask_type == MaskType::Clip);

			let needs_separate_alignment_fill = can_draw_aligned_stroke && !can_use_paint_order;
			let wants_stroke_below = vector.style.stroke().map(|s| s.paint_order) == Some(PaintOrder::StrokeBelow);

			if needs_separate_alignment_fill && !wants_stroke_below {
				render.leaf_tag("path", |attributes| {
					attributes.push("d", path.clone());
					let matrix = format_transform_matrix(element_transform);
					if !matrix.is_empty() {
						attributes.push(ATTR_TRANSFORM, matrix);
					}
					let mut style = vector.style.clone();
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

				let mut cloned_vector = vector.clone();
				cloned_vector.style.clear_stroke();
				cloned_vector.style.set_fill(Fill::solid(Color::BLACK));

				let vector_item = Table::new_from_row(
					TableRow::new_from_element(cloned_vector)
						.with_attribute(ATTR_TRANSFORM, multiplied_transform)
						.with_attribute(ATTR_BLEND_MODE, blend_mode_attr)
						.with_attribute(ATTR_OPACITY, opacity_attr)
						.with_attribute(ATTR_OPACITY_FILL, opacity_fill_attr)
						.with_attribute(ATTR_CLIPPING_MASK, clipping_mask_attr),
				);

				(id, mask_type, vector_item)
			});

			let use_face_fill = vector.use_face_fill();
			if use_face_fill {
				for mut face_path in vector.construct_faces().filter(|face| face.area() >= 0.) {
					face_path.apply_affine(Affine::new(applied_stroke_transform.to_cols_array()));

					let face_d = face_path.to_svg();
					render.leaf_tag("path", |attributes| {
						attributes.push("d", face_d.clone());
						let matrix = format_transform_matrix(element_transform);
						if !matrix.is_empty() {
							attributes.push(ATTR_TRANSFORM, matrix);
						}
						let mut style = vector.style.clone();
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
					attributes.push(ATTR_TRANSFORM, matrix);
				}

				let defs = &mut attributes.0.svg_defs;
				if let Some((ref id, mask_type, ref vector_item)) = push_id {
					let mut svg = SvgRender::new();
					vector_item.render_svg(&mut svg, &render_params.for_alignment(applied_stroke_transform));
					let stroke = vector.style.stroke().unwrap();
					// `push_id` is only `Some` when `can_draw_aligned_stroke`, which is gated on `path_is_closed`
					let inflation = stroke.max_aabb_inflation(true) * axial_max_scale(applied_stroke_transform);
					let quad = Quad::from_box(transformed_bounds).inflate(inflation);
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

				let mut style = vector.style.clone();
				if needs_separate_alignment_fill || use_face_fill {
					style.clear_fill();
				}

				let fill_and_stroke = style.render(defs, element_transform, applied_stroke_transform, bounds_matrix, transformed_bounds_matrix, &render_params);

				if let Some((id, mask_type, _)) = push_id {
					let selector = format!("url(#{id})");
					attributes.push(mask_type.to_attribute(), selector);
				}
				attributes.push_val(fill_and_stroke);

				if vector.is_branching() && !use_face_fill {
					attributes.push("fill-rule", "evenodd");
				}

				let opacity = (opacity_attr * if render_params.for_mask { 1. } else { opacity_fill_attr }) as f32;
				if opacity < 1. {
					attributes.push("opacity", opacity.to_string());
				}

				if blend_mode_attr != BlendMode::default() {
					attributes.push("style", blend_mode_attr.render());
				}
			});

			// When splitting passes and stroke is below, draw the fill after the stroke.
			if needs_separate_alignment_fill && wants_stroke_below {
				render.leaf_tag("path", |attributes| {
					attributes.push("d", path);
					let matrix = format_transform_matrix(element_transform);
					if !matrix.is_empty() {
						attributes.push(ATTR_TRANSFORM, matrix);
					}
					let mut style = vector.style.clone();
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
		use graphic_types::vector_types::vector::style::{GradientType, StrokeCap, StrokeJoin};

		for index in 0..self.len() {
			use graphic_types::vector_types::vector;

			let Some(element) = self.element(index) else { continue };
			let item_transform: DAffine2 = self.attribute_cloned_or_default(ATTR_TRANSFORM, index);
			let blend_mode_attr: BlendMode = self.attribute_cloned_or_default(ATTR_BLEND_MODE, index);
			let opacity_attr: f64 = self.attribute_cloned_or(ATTR_OPACITY, index, 1.);
			let opacity_fill_attr: f64 = self.attribute_cloned_or(ATTR_OPACITY_FILL, index, 1.);
			let clip_attr: bool = self.attribute_cloned_or_default(ATTR_CLIPPING_MASK, index);
			let multiplied_transform = parent_transform * item_transform;
			let has_real_stroke = element.style.stroke().filter(|stroke| stroke.weight() > 0.);
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
			let layer_bounds = element.bounding_box().unwrap_or_default();

			let mut path = kurbo::BezPath::new();
			for mut bezpath in element.stroke_bezpath_iter() {
				bezpath.apply_affine(Affine::new(applied_stroke_transform.to_cols_array()));
				for element in bezpath {
					path.push(element);
				}
			}

			// If we're using opacity or a blend mode, we need to push a layer
			let blend_mode = match render_params.render_mode {
				RenderMode::Outline => peniko::Mix::Normal,
				_ => blend_mode_attr.to_peniko(),
			};
			let mut layer = false;

			// Whether the renderer will engage the stroke-alignment compositing trick (non-Center align on a fully closed path).
			// Used by both the blend-layer clip rect inflation below (as `max_aabb_inflation`'s `path_is_closed` arg, equivalent here since
			// the function ignores the arg for Center align) and the `SrcIn`/`SrcOut` aligned-stroke branch further down.
			let stroke = element.style.stroke();
			let can_draw_aligned_stroke = stroke.as_ref().is_some_and(|s| s.has_renderable_stroke() && s.align.is_not_centered()) && element.stroke_bezier_paths().all(|p| p.closed());

			let opacity = (opacity_attr * if render_params.for_mask { 1. } else { opacity_fill_attr }) as f32;
			if opacity < 1. || blend_mode_attr != BlendMode::default() {
				layer = true;
				// `max_aabb_inflation` is in `applied_stroke_transform`-space; `layer_bounds` is path-local and `push_layer` re-applies `multiplied_transform`.
				// Divide by the smaller axial scale to cover the stroke in both axes after Vello's transform. Skip on a degenerate transform.
				let axial_scale = axial_min_scale(applied_stroke_transform);
				let stroke_inflation = stroke.as_ref().map_or(0., |s| s.max_aabb_inflation(can_draw_aligned_stroke));
				let inflate_amount = if axial_scale > 0. { stroke_inflation / axial_scale } else { 0. };
				let quad = Quad::from_box(layer_bounds).inflate(inflate_amount);
				let layer_bounds = quad.bounding_box();
				scene.push_layer(
					peniko::Fill::NonZero,
					peniko::BlendMode::new(blend_mode, peniko::Compose::SrcOver),
					opacity,
					kurbo::Affine::new(multiplied_transform.to_cols_array()),
					&kurbo::Rect::new(layer_bounds[0].x, layer_bounds[0].y, layer_bounds[1].x, layer_bounds[1].y),
				);
			}

			let use_layer = can_draw_aligned_stroke;
			let wants_stroke_below = stroke.as_ref().is_some_and(|s| s.paint_order == vector::style::PaintOrder::StrokeBelow);

			// Closures to avoid duplicated fill/stroke drawing logic
			let do_fill_path = |scene: &mut Scene, path: &kurbo::BezPath, fill_rule: peniko::Fill| match element.style.fill() {
				Fill::Solid(color) => {
					let fill = peniko::Brush::Solid(peniko::Color::new([color.r(), color.g(), color.b(), color.a()]));
					scene.fill(fill_rule, kurbo::Affine::new(element_transform.to_cols_array()), &fill, None, path);
				}
				Fill::Gradient(gradient) => {
					let mut stops = peniko::ColorStops::new();
					for (position, color, _) in gradient.stops.interpolated_samples() {
						stops.push(peniko::ColorStop {
							offset: position as f32,
							color: peniko::color::DynamicColor::from_alpha_color(peniko::Color::new([color.r(), color.g(), color.b(), color.a()])),
						});
					}

					let bounds = element.nonzero_bounding_box();
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
						extend: match gradient.spread_method {
							GradientSpreadMethod::Pad => peniko::Extend::Pad,
							GradientSpreadMethod::Reflect => peniko::Extend::Reflect,
							GradientSpreadMethod::Repeat => peniko::Extend::Repeat,
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
					scene.fill(fill_rule, kurbo::Affine::new(element_transform.to_cols_array()), &fill, Some(brush_transform), path);
				}
				Fill::None => {}
			};

			// Branching vectors without regions (e.g. mesh grids) need face-by-face fill rendering.
			let use_face_fill = element.use_face_fill();
			let do_fill = |scene: &mut Scene| {
				if use_face_fill {
					for mut face_path in element.construct_faces().filter(|face| face.area() >= 0.) {
						face_path.apply_affine(Affine::new(applied_stroke_transform.to_cols_array()));
						let mut kurbo_path = kurbo::BezPath::new();
						for element in face_path {
							kurbo_path.push(element);
						}
						do_fill_path(scene, &kurbo_path, peniko::Fill::NonZero);
					}
				} else if element.is_branching() {
					do_fill_path(scene, &path, peniko::Fill::EvenOdd);
				} else {
					do_fill_path(scene, &path, peniko::Fill::NonZero);
				}
			};

			let do_stroke = |scene: &mut Scene, width_scale: f64| {
				if let Some(stroke) = element.style.stroke() {
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
					let (outline_stroke, outline_color_peniko) = get_outline_styles(render_params);

					scene.stroke(&outline_stroke, kurbo::Affine::new(element_transform.to_cols_array()), outline_color_peniko, None, &path);
				}
				_ => {
					if use_layer {
						let mut cloned_element = element.clone();
						cloned_element.style.clear_stroke();
						cloned_element.style.set_fill(Fill::solid(Color::BLACK));

						let vector_table = Table::new_from_row(
							TableRow::new_from_element(cloned_element)
								.with_attribute(ATTR_TRANSFORM, item_transform)
								.with_attribute(ATTR_BLEND_MODE, blend_mode_attr)
								.with_attribute(ATTR_OPACITY, opacity_attr)
								.with_attribute(ATTR_OPACITY_FILL, opacity_fill_attr)
								.with_attribute(ATTR_CLIPPING_MASK, clip_attr),
						);

						let bounds = element.bounding_box_with_transform(multiplied_transform).unwrap_or(layer_bounds);
						// This branch is gated on `can_draw_aligned_stroke`, which already requires every subpath is closed
						let inflation = element.style.stroke().as_ref().map_or(0., |stroke| stroke.max_aabb_inflation(true));
						let quad = Quad::from_box(bounds).inflate(inflation * axial_max_scale(applied_stroke_transform));
						let bounds = quad.bounding_box();
						let rect = kurbo::Rect::new(bounds[0].x, bounds[0].y, bounds[1].x, bounds[1].y);

						let compose = if element.style.stroke().is_some_and(|x| x.align == StrokeAlign::Outside) {
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

						let order = match element.style.stroke().is_some_and(|stroke| !stroke.paint_order.is_default()) {
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

	fn collect_metadata(&self, metadata: &mut RenderMetadata, footprint: Footprint, caller_element_id: Option<NodeId>) {
		// Aggregate all items' targets per element_id so multi-item tables (e.g. 'Text' node with "Separate Glyphs" active) produce hit areas for every glyph.
		// Targets are baked relative to item 0's transform since `Graphic::collect_metadata` records that as `local_transforms[element_id]`.
		let item_zero_transform: DAffine2 = if !self.is_empty() {
			self.attribute_cloned_or_default(ATTR_TRANSFORM, 0)
		} else {
			DAffine2::IDENTITY
		};
		let item_zero_inverse = if item_zero_transform.matrix2.determinant() != 0. {
			item_zero_transform.inverse()
		} else {
			DAffine2::IDENTITY
		};

		let mut accumulated_click_targets: HashMap<NodeId, Vec<Arc<ClickTarget>>> = HashMap::new();
		let mut accumulated_outlines: HashMap<NodeId, Vec<Arc<ClickTarget>>> = HashMap::new();

		for index in 0..self.len() {
			let Some(source) = self.element(index) else { continue };
			let transform: DAffine2 = self.attribute_cloned_or_default(ATTR_TRANSFORM, index);
			let layer_path: Table<NodeId> = self.attribute_cloned_or_default(ATTR_EDITOR_LAYER_PATH, index);
			let layer = layer_path.iter_element_values().next_back().copied();

			if let Some(element_id) = caller_element_id.or(layer) {
				// When recovering element_id from the item's editor:layer_path tag (because the caller
				// passed None), also store the transform metadata that Graphic::collect_metadata
				// normally provides but skipped due to the None element_id.
				if caller_element_id.is_none() {
					metadata.upstream_footprints.entry(element_id).or_insert(footprint);
					metadata.local_transforms.entry(element_id).or_insert(item_zero_transform);
				}

				// Use click-target override if the item provides one (e.g. 'Text' node's per-glyph bboxes)
				let click_target_vector = self.attribute::<Vector>(ATTR_EDITOR_CLICK_TARGET, index).unwrap_or(source);

				let item_relative_transform = item_zero_inverse * transform;

				let mut click_targets_unwrapped = Vec::new();
				extend_targets_from_vector(&mut click_targets_unwrapped, click_target_vector, item_relative_transform);
				accumulated_click_targets.entry(element_id).or_default().extend(click_targets_unwrapped.into_iter().map(Arc::new));

				// Outlines always use source geometry so the visual outline reflects actual letterforms
				let mut outlines_unwrapped = Vec::new();
				extend_targets_from_vector(&mut outlines_unwrapped, source, item_relative_transform);
				accumulated_outlines.entry(element_id).or_default().extend(outlines_unwrapped.into_iter().map(Arc::new));

				// Source geometry (not the click-target override) so editing tools work on letterforms.
				// Only item 0 is recorded since editing tools can only target a single item currently.
				metadata.vector_data.entry(element_id).or_insert_with(|| Arc::new(source.clone()));

				// Surface `editor:text_frame` for the Text tool's drag cage
				if let Some(&frame) = self.attribute::<DAffine2>(ATTR_EDITOR_TEXT_FRAME, index) {
					metadata.text_frames.entry(element_id).or_insert(frame);
				}
			}

			// If this item carries a snapshot of upstream graphic content (e.g. it was produced by Boolean Operation,
			// Flatten Path, Morph, or any other destructive merge), recurse into that snapshot so the editor can
			// surface the original child layers' click targets.
			let upstream_nested_layers = self.attribute_cloned_or_default::<Table<Graphic>>(ATTR_EDITOR_MERGED_LAYERS, index);
			if !upstream_nested_layers.is_empty() {
				let mut upstream_footprint = footprint;
				upstream_footprint.transform *= transform;
				upstream_nested_layers.collect_metadata(metadata, upstream_footprint, None);
			}
		}

		// Overwrite with the full accumulated set (not just item 0's contribution)
		for (element_id, targets) in accumulated_click_targets {
			metadata.click_targets.insert(element_id, targets);
		}
		for (element_id, targets) in accumulated_outlines {
			metadata.outlines.insert(element_id, targets);
		}
	}

	fn add_upstream_click_targets(&self, click_targets: &mut Vec<ClickTarget>) {
		for index in 0..self.len() {
			let Some(source) = self.element(index) else { continue };
			let transform: DAffine2 = self.attribute_cloned_or_default(ATTR_TRANSFORM, index);

			// Use click-target override geometry if the item provides one (e.g. 'Text' node's per-glyph bounding boxes)
			let vector = self.attribute::<Vector>(ATTR_EDITOR_CLICK_TARGET, index).unwrap_or(source);

			extend_targets_from_vector(click_targets, vector, transform);
		}
	}

	fn add_upstream_outline_targets(&self, outlines: &mut Vec<ClickTarget>) {
		// Source geometry only, ignoring `editor:click_target`, so outlines reflect actual letterforms
		for index in 0..self.len() {
			let Some(source) = self.element(index) else { continue };
			let transform: DAffine2 = self.attribute_cloned_or_default(ATTR_TRANSFORM, index);

			extend_targets_from_vector(outlines, source, transform);
		}
	}

	fn new_ids_from_hash(&mut self, reference: Option<NodeId>) {
		for vector in self.iter_element_values_mut() {
			vector.vector_new_ids_from_hash(reference.map(|id| id.0).unwrap_or_default());
		}
	}
}

/// Build one `CompoundPath` (non-zero fill rule, so holes like the inside of an "O" work
/// correctly) plus one `FreePoint` per disconnected anchor, apply the transform, and append.
fn extend_targets_from_vector(targets: &mut Vec<ClickTarget>, vector: &Vector, transform: DAffine2) {
	let stroke_width = vector.style.stroke().as_ref().map_or(0., Stroke::effective_width);
	let filled = vector.style.fill() != &Fill::None;
	let subpaths: Vec<Subpath<_>> = vector
		.stroke_bezier_paths()
		.map(|mut subpath| {
			if filled {
				subpath.set_closed(true);
			}
			subpath
		})
		.collect();
	if !subpaths.is_empty() {
		let mut click_target = ClickTarget::new_with_compound_path(subpaths, stroke_width);
		click_target.apply_transform(transform);
		targets.push(click_target);
	}

	for click_target in extend_free_point_targets(vector, transform) {
		targets.push(click_target);
	}
}

fn extend_free_point_targets(vector: &Vector, transform: DAffine2) -> impl Iterator<Item = ClickTarget> + '_ {
	vector.point_domain.ids().iter().filter_map(move |&point_id| {
		if vector.any_connected(point_id) {
			return None;
		}

		let anchor = vector.point_domain.position_from_id(point_id).unwrap_or_default();
		let mut click_target = ClickTarget::new_with_free_point(FreePoint::new(point_id, anchor));
		click_target.apply_transform(transform);
		Some(click_target)
	})
}

impl Render for Table<Raster<CPU>> {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		for index in 0..self.len() {
			let Some(image) = self.element(index) else { continue };

			let transform: DAffine2 = self.attribute_cloned_or_default(ATTR_TRANSFORM, index);
			let blend_mode_attr: BlendMode = self.attribute_cloned_or_default(ATTR_BLEND_MODE, index);
			let opacity_attr: f64 = self.attribute_cloned_or(ATTR_OPACITY, index, 1.);
			let opacity_fill_attr: f64 = self.attribute_cloned_or(ATTR_OPACITY_FILL, index, 1.);

			if image.data.is_empty() {
				continue;
			}

			if render_params.to_canvas() {
				let mut image_copy = image.clone();
				image_copy.data_mut().map_pixels(|p| p.to_unassociated_alpha());
				let id = *render.image_data.entry(CacheHashWrapper(image_copy.into_data())).or_insert_with(generate_uuid);

				render.parent_tag(
					"foreignObject",
					|attributes| {
						let mut transform_values = transform.to_scale_angle_translation();
						let size = DVec2::new(image.width as f64, image.height as f64);
						transform_values.0 /= size;

						let matrix = DAffine2::from_scale_angle_translation(transform_values.0, transform_values.1, transform_values.2);
						let matrix = format_transform_matrix(matrix);
						if !matrix.is_empty() {
							attributes.push(ATTR_TRANSFORM, matrix);
						}

						attributes.push("width", size.x.to_string());
						attributes.push("height", size.y.to_string());

						let opacity = (opacity_attr * if render_params.for_mask { 1. } else { opacity_fill_attr }) as f32;
						if opacity < 1. {
							attributes.push("opacity", opacity.to_string());
						}

						if blend_mode_attr != BlendMode::default() {
							attributes.push("style", blend_mode_attr.render());
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
						attributes.push(ATTR_TRANSFORM, matrix);
					}

					let opacity = (opacity_attr * if render_params.for_mask { 1. } else { opacity_fill_attr }) as f32;
					if opacity < 1. {
						attributes.push("opacity", opacity.to_string());
					}
					if blend_mode_attr != BlendMode::default() {
						attributes.push("style", blend_mode_attr.render());
					}
				});
			}
		}
	}

	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2, _: &mut RenderContext, render_params: &RenderParams) {
		for index in 0..self.len() {
			let Some(image) = self.element(index) else { continue };
			if image.data.is_empty() {
				continue;
			}

			let blend_mode_attr: BlendMode = self.attribute_cloned_or_default(ATTR_BLEND_MODE, index);
			let opacity_attr: f64 = self.attribute_cloned_or(ATTR_OPACITY, index, 1.);
			let opacity_fill_attr: f64 = self.attribute_cloned_or(ATTR_OPACITY_FILL, index, 1.);
			let blend_mode = blend_mode_attr.to_peniko();

			let opacity = (opacity_attr * if render_params.for_mask { 1. } else { opacity_fill_attr }) as f32;
			let mut layer = false;

			if (opacity < 1. || (render_params.render_mode != RenderMode::Outline && blend_mode_attr != BlendMode::default()))
				&& let RenderBoundingBox::Rectangle(bounds) = self.bounding_box(transform, false)
			{
				let blending = peniko::BlendMode::new(blend_mode, peniko::Compose::SrcOver);
				let rect = kurbo::Rect::new(bounds[0].x, bounds[0].y, bounds[1].x, bounds[1].y);
				scene.push_layer(peniko::Fill::NonZero, blending, opacity, kurbo::Affine::IDENTITY, &rect);
				layer = true;
			}

			let transform_attribute: DAffine2 = self.attribute_cloned_or_default(ATTR_TRANSFORM, index);

			if let RenderMode::Outline = render_params.render_mode {
				let outline_transform: DAffine2 = transform * transform_attribute;
				draw_raster_outline(scene, &outline_transform, render_params);

				if layer {
					scene.pop_layer();
				}

				continue;
			}

			let image_transform = transform * transform_attribute * DAffine2::from_scale(1. / DVec2::new(image.width as f64, image.height as f64));

			let image_brush = peniko::ImageBrush::new(peniko::ImageData {
				data: image.to_flat_u8().0.into(),
				format: peniko::ImageFormat::Rgba8,
				width: image.width,
				height: image.height,
				alpha_type: peniko::ImageAlphaType::Alpha,
			})
			.with_extend(peniko::Extend::Repeat);

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
		// TODO: Find a way to handle more than one item of the `Table<Raster<...>>`
		if !self.is_empty() {
			let transform: DAffine2 = self.attribute_cloned_or_default(ATTR_TRANSFORM, 0);
			metadata.local_transforms.insert(element_id, transform);

			// If this raster carries a snapshot of upstream graphic content (e.g. it was produced by Rasterize,
			// which destructively merges its inputs into pixels), recurse into that snapshot so the editor can
			// surface the original child layers' click targets (the same mechanism Boolean Operation uses).
			// The snapshot was captured before Rasterize shifted its input transforms to align with the rasterization
			// area, so the children are already in the coordinate space matching `footprint` here — we must NOT
			// multiply in `transform` (which is the rasterization area, not a layer-stack transform).
			let upstream_nested_layers = self.attribute_cloned_or_default::<Table<Graphic>>(ATTR_EDITOR_MERGED_LAYERS, 0);
			if !upstream_nested_layers.is_empty() {
				upstream_nested_layers.collect_metadata(metadata, footprint, None);
			}
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

	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2, context: &mut RenderContext, render_params: &RenderParams) {
		for index in 0..self.len() {
			let Some(raster) = self.element(index) else { continue };
			let blend_mode_attr: BlendMode = self.attribute_cloned_or_default(ATTR_BLEND_MODE, index);
			let opacity_attr: f64 = self.attribute_cloned_or(ATTR_OPACITY, index, 1.);
			let opacity_fill_attr: f64 = self.attribute_cloned_or(ATTR_OPACITY_FILL, index, 1.);
			let clip_attr: bool = self.attribute_cloned_or_default(ATTR_CLIPPING_MASK, index);
			let blend_mode = match render_params.render_mode {
				RenderMode::Outline => peniko::Mix::Normal,
				_ => blend_mode_attr.to_peniko(),
			};

			let mut layer = false;

			let opacity = (opacity_attr * if render_params.for_mask { 1. } else { opacity_fill_attr }) as f32;
			let any_nondefault = blend_mode_attr != BlendMode::default() || opacity < 1. || clip_attr;
			if (render_params.render_mode != RenderMode::Outline && any_nondefault)
				&& let RenderBoundingBox::Rectangle(bounds) = self.bounding_box(transform, true)
			{
				let blending = peniko::BlendMode::new(blend_mode, peniko::Compose::SrcOver);
				let rect = kurbo::Rect::new(bounds[0].x, bounds[0].y, bounds[1].x, bounds[1].y);
				scene.push_layer(peniko::Fill::NonZero, blending, opacity, kurbo::Affine::IDENTITY, &rect);
				layer = true;
			}

			let transform_attribute: DAffine2 = self.attribute_cloned_or_default(ATTR_TRANSFORM, index);

			if let RenderMode::Outline = render_params.render_mode {
				let outline_transform = transform * transform_attribute;
				draw_raster_outline(scene, &outline_transform, render_params);

				if layer {
					scene.pop_layer();
				}

				continue;
			}

			let width = raster.data().width();
			let height = raster.data().height();
			let image = peniko::ImageBrush::new(peniko::ImageData {
				data: peniko::Blob::new(LAZY_ARC_VEC_ZERO_U8.deref().clone()),
				format: peniko::ImageFormat::Rgba8,
				width,
				height,
				alpha_type: peniko::ImageAlphaType::Alpha,
			})
			.with_extend(peniko::Extend::Repeat);
			let image_transform = transform * transform_attribute * DAffine2::from_scale(1. / DVec2::new(width as f64, height as f64));
			scene.draw_image(&image, kurbo::Affine::new(image_transform.to_cols_array()));
			context.resource_overrides.push((image, raster.data().clone()));

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
		// TODO: Find a way to handle more than one item of the `Table<Raster<...>>`
		if !self.is_empty() {
			let transform: DAffine2 = self.attribute_cloned_or_default(ATTR_TRANSFORM, 0);
			metadata.local_transforms.insert(element_id, transform);

			// If this raster carries a snapshot of upstream graphic content (e.g. it was produced by Rasterize,
			// which destructively merges its inputs into pixels), recurse into that snapshot so the editor can
			// surface the original child layers' click targets (the same mechanism Boolean Operation uses).
			// The snapshot was captured before Rasterize shifted its input transforms to align with the rasterization
			// area, so the children are already in the coordinate space matching `footprint` here — we must NOT
			// multiply in `transform` (which is the rasterization area, not a layer-stack transform).
			let upstream_nested_layers = self.attribute_cloned_or_default::<Table<Graphic>>(ATTR_EDITOR_MERGED_LAYERS, 0);
			if !upstream_nested_layers.is_empty() {
				upstream_nested_layers.collect_metadata(metadata, footprint, None);
			}
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
		for (index, color) in self.iter_element_values().enumerate() {
			let blend_mode: BlendMode = self.attribute_cloned_or_default(ATTR_BLEND_MODE, index);
			let opacity_attr: f64 = self.attribute_cloned_or(ATTR_OPACITY, index, 1.);
			let opacity_fill_attr: f64 = self.attribute_cloned_or(ATTR_OPACITY_FILL, index, 1.);
			render.leaf_tag("polyline", |attributes| {
				// Stand-in for an infinite background. Chrome's SVG renderer keeps internal coordinates in f32 and loses
				// precision past ~2^24 (~16.7 million), causing tile-boundary artifacts that pop in and out during panning.
				// 1e7 stays under that limit while still being far larger than any practical document extent.
				const MAX: f64 = 1e7;
				attributes.push("points", format!("{MAX},{MAX} -{MAX},{MAX} -{MAX},-{MAX} {MAX},-{MAX}"));

				attributes.push("fill", format!("#{}", color.to_rgb_hex_srgb_from_gamma()));
				if color.a() < 1. {
					attributes.push("fill-opacity", ((color.a() * 1000.).round() / 1000.).to_string());
				}

				let opacity = (opacity_attr * if render_params.for_mask { 1. } else { opacity_fill_attr }) as f32;
				if opacity < 1. {
					attributes.push("opacity", opacity.to_string());
				}

				if blend_mode != BlendMode::default() {
					attributes.push("style", blend_mode.render());
				}
			});
		}
	}

	fn render_to_vello(&self, scene: &mut Scene, _parent_transform: DAffine2, _context: &mut RenderContext, render_params: &RenderParams) {
		use vello::peniko;

		for (index, color) in self.iter_element_values().enumerate() {
			let blend_mode_attr: BlendMode = self.attribute_cloned_or_default(ATTR_BLEND_MODE, index);
			let opacity_attr: f64 = self.attribute_cloned_or(ATTR_OPACITY, index, 1.);
			let opacity_fill_attr: f64 = self.attribute_cloned_or(ATTR_OPACITY_FILL, index, 1.);
			let blend_mode = blend_mode_attr.to_peniko();
			let opacity = (opacity_attr * if render_params.for_mask { 1. } else { opacity_fill_attr }) as f32;

			let vello_color = peniko::Color::new([color.r(), color.g(), color.b(), color.a()]);

			let rect = kurbo::Rect::from_origin_size(kurbo::Point::ZERO, kurbo::Size::new(1., 1.));

			let mut layer = false;
			if opacity < 1. || blend_mode_attr != BlendMode::default() {
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
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		// For thumbnails the gradient fills a finite rect at the footprint's document space bounds, with a 1-unit margin to cover the `as u32` truncation of `Footprint::resolution`.
		// The viewBox crops the overshoot. Canvas rendering keeps the polyline path since Chrome rejects rects larger than ~20 million.
		let thumbnail_rect = if render_params.thumbnail {
			let truncated_size = render_params.footprint.resolution.as_dvec2();
			let margin = DVec2::ONE;
			Some((render_params.footprint.transform.translation - margin / 2., truncated_size + margin))
		} else {
			None
		};

		for index in 0..self.len() {
			let Some(gradient) = self.element(index) else { continue };
			let transform: DAffine2 = self.attribute_cloned_or_default(ATTR_TRANSFORM, index);
			let blend_mode: BlendMode = self.attribute_cloned_or_default(ATTR_BLEND_MODE, index);
			let opacity_attr: f64 = self.attribute_cloned_or(ATTR_OPACITY, index, 1.);
			let opacity_fill_attr: f64 = self.attribute_cloned_or(ATTR_OPACITY_FILL, index, 1.);
			let spread_method: GradientSpreadMethod = self.attribute_cloned_or_default(ATTR_SPREAD_METHOD, index);
			let gradient_type: GradientType = self.attribute_cloned_or_default(ATTR_GRADIENT_TYPE, index);
			let tag = if thumbnail_rect.is_some() { "rect" } else { "polyline" };
			render.leaf_tag(tag, |attributes| {
				if let Some((min, size)) = thumbnail_rect {
					attributes.push("x", min.x.to_string());
					attributes.push("y", min.y.to_string());
					attributes.push("width", size.x.to_string());
					attributes.push("height", size.y.to_string());
				} else {
					// Stand-in for an infinite background. Chrome's SVG renderer keeps internal coordinates in f32 and loses
					// precision past ~2^24 (~16.7 million), causing tile-boundary artifacts that pop in and out during panning.
					// 1e7 stays under that limit while still being far larger than any practical document extent.
					const MAX: f64 = 1e7;
					attributes.push("points", format!("{MAX},{MAX} -{MAX},{MAX} -{MAX},-{MAX} {MAX},-{MAX}"));
				}

				let mut stop_string = String::new();
				for (position, color, original_midpoint) in gradient.interpolated_samples() {
					let _ = write!(stop_string, r##"<stop offset="{}" stop-color="#{}""##, position, color.to_rgb_hex_srgb_from_gamma());
					if color.a() < 1. {
						let _ = write!(stop_string, r#" stop-opacity="{}""#, color.a());
					}
					if let Some(midpoint) = original_midpoint {
						let _ = write!(stop_string, r#" graphite:midpoint="{}""#, (midpoint * 1000.).round() / 1000.);
					}
					stop_string.push_str(" />");
				}

				// render_thumbnail already added the footprint transform
				let gradient_transform = if render_params.thumbnail { transform } else { render_params.footprint.transform * transform };
				let gradient_transform_matrix = format_transform_matrix(gradient_transform);
				let gradient_transform_attribute = if gradient_transform_matrix.is_empty() {
					String::new()
				} else {
					format!(r#" gradientTransform="{gradient_transform_matrix}""#)
				};

				let gradient_id = generate_uuid();
				let spread_method_attribute = if spread_method == GradientSpreadMethod::Pad {
					String::new()
				} else {
					format!(r#" spreadMethod="{}""#, spread_method.svg_name())
				};

				// The unit gradient line is the +X unit vector in local space, before the item's transform is applied
				match gradient_type {
					GradientType::Linear => {
						let _ = write!(
							&mut attributes.0.svg_defs,
							r#"<linearGradient id="{gradient_id}" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="1" y2="0"{spread_method_attribute}{gradient_transform_attribute}>{stop_string}</linearGradient>"#
						);
					}
					GradientType::Radial => {
						let _ = write!(
							&mut attributes.0.svg_defs,
							r#"<radialGradient id="{gradient_id}" gradientUnits="userSpaceOnUse" cx="0" cy="0" r="1"{spread_method_attribute}{gradient_transform_attribute}>{stop_string}</radialGradient>"#
						);
					}
				}

				attributes.push("fill", format!("url('#{gradient_id}')"));

				let opacity = (opacity_attr * if render_params.for_mask { 1. } else { opacity_fill_attr }) as f32;
				if opacity < 1. {
					attributes.push("opacity", opacity.to_string());
				}

				if blend_mode != BlendMode::default() {
					attributes.push("style", blend_mode.render());
				}
			});
		}
	}

	fn render_to_vello(&self, scene: &mut Scene, parent_transform: DAffine2, _context: &mut RenderContext, render_params: &RenderParams) {
		use vello::peniko;

		if let RenderMode::Outline = render_params.render_mode {
			return;
		}

		for (((index, gradient), spread_method), gradient_type) in self
			.iter_element_values()
			.enumerate()
			.zip(self.iter_attribute_values_or_default::<GradientSpreadMethod>(ATTR_SPREAD_METHOD))
			.zip(self.iter_attribute_values_or_default::<GradientType>(ATTR_GRADIENT_TYPE))
		{
			let transform: DAffine2 = self.attribute_cloned_or_default(ATTR_TRANSFORM, index);
			let blend_mode_attr: BlendMode = self.attribute_cloned_or_default(ATTR_BLEND_MODE, index);
			let opacity_attr: f64 = self.attribute_cloned_or(ATTR_OPACITY, index, 1.);
			let opacity_fill_attr: f64 = self.attribute_cloned_or(ATTR_OPACITY_FILL, index, 1.);
			let gradient_transform = parent_transform * transform;

			let blend_mode = blend_mode_attr.to_peniko();
			let opacity = (opacity_attr * if render_params.for_mask { 1. } else { opacity_fill_attr }) as f32;

			let mut stops: peniko::ColorStops = peniko::ColorStops::new();
			for (position, color, _) in gradient.interpolated_samples() {
				stops.push(peniko::ColorStop {
					offset: position as f32,
					color: peniko::color::DynamicColor::from_alpha_color(peniko::Color::new([color.r(), color.g(), color.b(), color.a()])),
				})
			}

			let extend = match spread_method {
				GradientSpreadMethod::Pad => peniko::Extend::Pad,
				GradientSpreadMethod::Reflect => peniko::Extend::Reflect,
				GradientSpreadMethod::Repeat => peniko::Extend::Repeat,
			};

			// The unit gradient line is the +X unit vector in local space, before the item's transform is applied.
			// For radial, the unit-radius circle at the origin scales out to the line's length once the brush transform applies.
			let kind = match gradient_type {
				GradientType::Linear => peniko::LinearGradientPosition {
					start: to_point(DVec2::ZERO),
					end: to_point(DVec2::X),
				}
				.into(),
				GradientType::Radial => peniko::RadialGradientPosition {
					start_center: to_point(DVec2::ZERO),
					start_radius: 0.,
					end_center: to_point(DVec2::ZERO),
					end_radius: 1.,
				}
				.into(),
			};

			let fill = peniko::Brush::Gradient(peniko::Gradient {
				kind,
				stops,
				extend,
				interpolation_alpha_space: peniko::InterpolationAlphaSpace::Premultiplied,
				..Default::default()
			});
			let brush_transform = kurbo::Affine::new((gradient_transform).to_cols_array());
			let rect = kurbo::Rect::from_origin_size(kurbo::Point::ZERO, kurbo::Size::new(1., 1.));

			let mut layer = false;
			if opacity < 1. || blend_mode_attr != BlendMode::default() {
				let blending = peniko::BlendMode::new(blend_mode, peniko::Compose::SrcOver);
				// See implementation in `Table<Color>` for more detail
				scene.push_layer(peniko::Fill::NonZero, blending, opacity, kurbo::Affine::scale(f64::INFINITY), &rect);
				layer = true;
			}

			// Encode shape and brush manually instead of Scene.fill(), which would multiply brush_transform by the path transform
			scene.encoding_mut().encode_transform(vello_encoding::Transform::from_kurbo(&kurbo::Affine::scale(f64::INFINITY)));
			scene.encoding_mut().encode_fill_style(peniko::Fill::NonZero);
			scene.encoding_mut().encode_shape(&rect, true);

			scene.encoding_mut().encode_transform(vello_encoding::Transform::from_kurbo(&brush_transform));
			scene.encoding_mut().swap_last_path_tags();
			scene.encoding_mut().encode_brush(&fill, 1.);

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

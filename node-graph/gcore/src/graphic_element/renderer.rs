mod quad;
mod rect;

#[cfg(feature = "vello")]
use crate::consts::{LAYER_OUTLINE_STROKE_COLOR, LAYER_OUTLINE_STROKE_WEIGHT};
use crate::raster::image::ImageFrameTable;
use crate::raster::{BlendMode, Image};
use crate::transform::{Footprint, Transform};
use crate::uuid::{NodeId, generate_uuid};
use crate::vector::style::{Fill, Stroke, ViewMode};
use crate::vector::{PointId, VectorDataTable};
use crate::{Artboard, ArtboardGroupTable, Color, GraphicElement, GraphicGroupTable, RasterFrame};
use base64::Engine;
use bezier_rs::Subpath;
use dyn_any::DynAny;
use glam::{DAffine2, DMat2, DVec2};
use num_traits::Zero;
pub use quad::Quad;
pub use rect::Rect;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
#[cfg(feature = "vello")]
use vello::*;

/// Represents a clickable target for the layer
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ClickTarget {
	subpath: bezier_rs::Subpath<PointId>,
	stroke_width: f64,
	bounding_box: Option<[DVec2; 2]>,
}

impl ClickTarget {
	pub fn new(subpath: bezier_rs::Subpath<PointId>, stroke_width: f64) -> Self {
		let bounding_box = subpath.loose_bounding_box();
		Self { subpath, stroke_width, bounding_box }
	}

	pub fn subpath(&self) -> &bezier_rs::Subpath<PointId> {
		&self.subpath
	}

	pub fn bounding_box(&self) -> Option<[DVec2; 2]> {
		self.bounding_box
	}

	pub fn bounding_box_with_transform(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		self.bounding_box.map(|[a, b]| [transform.transform_point2(a), transform.transform_point2(b)])
	}

	pub fn apply_transform(&mut self, affine_transform: DAffine2) {
		self.subpath.apply_transform(affine_transform);
		self.update_bbox();
	}

	fn update_bbox(&mut self) {
		self.bounding_box = self.subpath.bounding_box();
	}

	/// Does the click target intersect the path
	pub fn intersect_path<It: Iterator<Item = bezier_rs::Bezier>>(&self, mut bezier_iter: impl FnMut() -> It, layer_transform: DAffine2) -> bool {
		// Check if the matrix is not invertible
		let mut layer_transform = layer_transform;
		if layer_transform.matrix2.determinant().abs() <= f64::EPSILON {
			layer_transform.matrix2 += DMat2::IDENTITY * 1e-4; // TODO: Is this the cleanest way to handle this?
		}

		let inverse = layer_transform.inverse();
		let mut bezier_iter = || bezier_iter().map(|bezier| bezier.apply_transformation(|point| inverse.transform_point2(point)));

		// Check if outlines intersect
		let outline_intersects = |path_segment: bezier_rs::Bezier| bezier_iter().any(|line| !path_segment.intersections(&line, None, None).is_empty());
		if self.subpath.iter().any(outline_intersects) {
			return true;
		}
		// Check if selection is entirely within the shape
		if self.subpath.closed() && bezier_iter().next().is_some_and(|bezier| self.subpath.contains_point(bezier.start)) {
			return true;
		}

		// Check if shape is entirely within selection
		let any_point_from_subpath = self.subpath.manipulator_groups().first().map(|group| group.anchor);
		any_point_from_subpath.is_some_and(|shape_point| bezier_iter().map(|bezier| bezier.winding(shape_point)).sum::<i32>() != 0)
	}

	/// Does the click target intersect the point (accounting for stroke size)
	pub fn intersect_point(&self, point: DVec2, layer_transform: DAffine2) -> bool {
		let target_bounds = [point - DVec2::splat(self.stroke_width / 2.), point + DVec2::splat(self.stroke_width / 2.)];
		let intersects = |a: [DVec2; 2], b: [DVec2; 2]| a[0].x <= b[1].x && a[1].x >= b[0].x && a[0].y <= b[1].y && a[1].y >= b[0].y;
		// This bounding box is not very accurate as it is the axis aligned version of the transformed bounding box. However it is fast.
		if !self
			.bounding_box
			.is_some_and(|loose| (loose[0] - loose[1]).abs().cmpgt(DVec2::splat(1e-4)).all() && intersects((layer_transform * Quad::from_box(loose)).bounding_box(), target_bounds))
		{
			return false;
		}

		// Allows for selecting lines
		// TODO: actual intersection of stroke
		let inflated_quad = Quad::from_box(target_bounds);
		self.intersect_path(|| inflated_quad.bezier_lines(), layer_transform)
	}

	/// Does the click target intersect the point (not accounting for stroke size)
	pub fn intersect_point_no_stroke(&self, point: DVec2) -> bool {
		// Check if the point is within the bounding box
		if self
			.bounding_box
			.is_some_and(|bbox| bbox[0].x <= point.x && point.x <= bbox[1].x && bbox[0].y <= point.y && point.y <= bbox[1].y)
		{
			// Check if the point is within the shape
			self.subpath.closed() && self.subpath.contains_point(point)
		} else {
			false
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
		let transform = if matrix.is_empty() { String::new() } else { format!(r#" transform="{}""#, matrix) };

		let svg_header = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" {}><defs>{defs}</defs><g{transform}>"#, view_box);
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
	#[cfg(feature = "wgpu")]
	pub resource_overrides: std::collections::HashMap<u64, alloc::sync::Arc<wgpu::Texture>>,
}

/// Static state used whilst rendering
#[derive(Default)]
pub struct RenderParams {
	pub view_mode: ViewMode,
	pub culling_bounds: Option<[DVec2; 2]>,
	pub thumbnail: bool,
	/// Don't render the rectangle for an artboard to allow exporting with a transparent background.
	pub hide_artboards: bool,
	/// Are we exporting? Causes the text above an artboard to be hidden.
	pub for_export: bool,
}

impl RenderParams {
	pub fn new(view_mode: ViewMode, culling_bounds: Option<[DVec2; 2]>, thumbnail: bool, hide_artboards: bool, for_export: bool) -> Self {
		Self {
			view_mode,
			culling_bounds,
			thumbnail,
			hide_artboards,
			for_export,
		}
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

pub fn to_transform(transform: DAffine2) -> usvg::Transform {
	let cols = transform.to_cols_array();
	usvg::Transform::from_row(cols[0] as f32, cols[1] as f32, cols[2] as f32, cols[3] as f32, cols[4] as f32, cols[5] as f32)
}

// TODO: Click targets can be removed from the render output, since the vector data is available in the vector modify data from Monitor nodes.
// This will require that the transform for child layers into that layer space be calculated, or it could be returned from the RenderOutput instead of click targets.
#[derive(Debug, Default, Clone, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RenderMetadata {
	pub upstream_footprints: HashMap<NodeId, Footprint>,
	pub local_transforms: HashMap<NodeId, DAffine2>,
	pub click_targets: HashMap<NodeId, Vec<ClickTarget>>,
	pub clip_targets: HashSet<NodeId>,
}

// TODO: Rename to "Graphical"
pub trait GraphicElementRendered {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams);

	#[cfg(feature = "vello")]
	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2, context: &mut RenderContext, _render_params: &RenderParams);
	fn bounding_box(&self, transform: DAffine2) -> Option<[DVec2; 2]>;

	// The upstream click targets for each layer are collected during the render so that they do not have to be calculated for each click detection
	fn add_upstream_click_targets(&self, _click_targets: &mut Vec<ClickTarget>) {}

	// TODO: Store all click targets in a vec which contains the AABB, click target, and path
	// fn add_click_targets(&self, click_targets: &mut Vec<([DVec2; 2], ClickTarget, Vec<NodeId>)>, current_path: Option<NodeId>) {}

	// Recursively iterate over data in the render (including groups upstream from vector data in the case of a boolean operation) to collect the footprints, click targets, and vector modify
	fn collect_metadata(&self, _metadata: &mut RenderMetadata, _footprint: Footprint, _element_id: Option<NodeId>) {}

	fn contains_artboard(&self) -> bool {
		false
	}

	fn new_ids_from_hash(&mut self, _reference: Option<NodeId>) {}

	fn to_graphic_element(&self) -> GraphicElement {
		GraphicElement::default()
	}
}

impl GraphicElementRendered for GraphicGroupTable {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		for instance in self.instances() {
			render.parent_tag(
				"g",
				|attributes| {
					let matrix = format_transform_matrix(*instance.transform);
					if !matrix.is_empty() {
						attributes.push("transform", matrix);
					}

					if instance.alpha_blending.opacity < 1. {
						attributes.push("opacity", instance.alpha_blending.opacity.to_string());
					}

					if instance.alpha_blending.blend_mode != BlendMode::default() {
						attributes.push("style", instance.alpha_blending.blend_mode.render());
					}
				},
				|render| {
					instance.instance.render_svg(render, render_params);
				},
			);
		}
	}

	#[cfg(feature = "vello")]
	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2, context: &mut RenderContext, render_params: &RenderParams) {
		for instance in self.instances() {
			let transform = transform * *instance.transform;
			let alpha_blending = *instance.alpha_blending;

			let mut layer = false;
			if let Some(bounds) = self.instances().filter_map(|element| element.instance.bounding_box(transform)).reduce(Quad::combine_bounds) {
				let blend_mode = match render_params.view_mode {
					ViewMode::Outline => peniko::Mix::Normal,
					_ => alpha_blending.blend_mode.into(),
				};

				if alpha_blending.opacity < 1. || (render_params.view_mode != ViewMode::Outline && alpha_blending.blend_mode != BlendMode::default()) {
					scene.push_layer(
						peniko::BlendMode::new(blend_mode, peniko::Compose::SrcOver),
						alpha_blending.opacity,
						kurbo::Affine::IDENTITY,
						&vello::kurbo::Rect::new(bounds[0].x, bounds[0].y, bounds[1].x, bounds[1].y),
					);
					layer = true;
				}
			}

			instance.instance.render_to_vello(scene, transform, context, render_params);

			if layer {
				scene.pop_layer();
			}
		}
	}

	fn bounding_box(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		self.instances()
			.filter_map(|element| element.instance.bounding_box(transform * *element.transform))
			.reduce(Quad::combine_bounds)
	}

	fn collect_metadata(&self, metadata: &mut RenderMetadata, footprint: Footprint, element_id: Option<NodeId>) {
		for instance in self.instances() {
			if let Some(element_id) = instance.source_node_id {
				let mut footprint = footprint;
				footprint.transform *= *instance.transform;

				instance.instance.collect_metadata(metadata, footprint, Some(*element_id));
			}
		}

		if let Some(graphic_group_id) = element_id {
			let mut all_upstream_click_targets = Vec::new();

			for instance in self.instances() {
				let mut new_click_targets = Vec::new();
				instance.instance.add_upstream_click_targets(&mut new_click_targets);

				for click_target in new_click_targets.iter_mut() {
					click_target.apply_transform(*instance.transform)
				}

				all_upstream_click_targets.extend(new_click_targets);
			}

			metadata.click_targets.insert(graphic_group_id, all_upstream_click_targets);
		}
	}

	fn add_upstream_click_targets(&self, click_targets: &mut Vec<ClickTarget>) {
		for instance in self.instances() {
			let mut new_click_targets = Vec::new();

			instance.instance.add_upstream_click_targets(&mut new_click_targets);

			for click_target in new_click_targets.iter_mut() {
				click_target.apply_transform(*instance.transform)
			}

			click_targets.extend(new_click_targets);
		}
	}

	fn contains_artboard(&self) -> bool {
		self.instances().any(|instance| instance.instance.contains_artboard())
	}

	fn new_ids_from_hash(&mut self, _reference: Option<NodeId>) {
		for instance in self.instances_mut() {
			instance.instance.new_ids_from_hash(*instance.source_node_id);
		}
	}

	fn to_graphic_element(&self) -> GraphicElement {
		GraphicElement::GraphicGroup(self.clone())
	}
}

impl GraphicElementRendered for VectorDataTable {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		for instance in self.instances() {
			let multiplied_transform = render.transform * *instance.transform;
			// Only consider strokes with non-zero weight, since default strokes with zero weight would prevent assigning the correct stroke transform
			let has_real_stroke = instance.instance.style.stroke().filter(|stroke| stroke.weight() > 0.);
			let set_stroke_transform = has_real_stroke.map(|stroke| stroke.transform).filter(|transform| transform.matrix2.determinant() != 0.);
			let applied_stroke_transform = set_stroke_transform.unwrap_or(*instance.transform);
			let element_transform = set_stroke_transform.map(|stroke_transform| multiplied_transform * stroke_transform.inverse());
			let element_transform = element_transform.unwrap_or(DAffine2::IDENTITY);
			let layer_bounds = instance.instance.bounding_box().unwrap_or_default();
			let transformed_bounds = instance.instance.bounding_box_with_transform(applied_stroke_transform).unwrap_or_default();

			let mut path = String::new();
			for subpath in instance.instance.stroke_bezier_paths() {
				let _ = subpath.subpath_to_svg(&mut path, applied_stroke_transform);
			}

			render.leaf_tag("path", |attributes| {
				attributes.push("d", path);
				let matrix = format_transform_matrix(element_transform);
				if !matrix.is_empty() {
					attributes.push("transform", matrix);
				}

				let defs = &mut attributes.0.svg_defs;

				let fill_and_stroke = instance
					.instance
					.style
					.render(render_params.view_mode, defs, element_transform, applied_stroke_transform, layer_bounds, transformed_bounds);
				attributes.push_val(fill_and_stroke);

				if instance.alpha_blending.opacity < 1. {
					attributes.push("opacity", instance.alpha_blending.opacity.to_string());
				}

				if instance.alpha_blending.blend_mode != BlendMode::default() {
					attributes.push("style", instance.alpha_blending.blend_mode.render());
				}
			});
		}
	}

	#[cfg(feature = "vello")]
	fn render_to_vello(&self, scene: &mut Scene, parent_transform: DAffine2, _: &mut RenderContext, render_params: &RenderParams) {
		use crate::vector::style::{GradientType, LineCap, LineJoin};
		use vello::kurbo::{Cap, Join};
		use vello::peniko;

		for instance in self.instances() {
			let multiplied_transform = parent_transform * *instance.transform;
			let has_real_stroke = instance.instance.style.stroke().filter(|stroke| stroke.weight() > 0.);
			let set_stroke_transform = has_real_stroke.map(|stroke| stroke.transform).filter(|transform| transform.matrix2.determinant() != 0.);
			let applied_stroke_transform = set_stroke_transform.unwrap_or(multiplied_transform);
			let element_transform = set_stroke_transform.map(|stroke_transform| multiplied_transform * stroke_transform.inverse());
			let element_transform = element_transform.unwrap_or(DAffine2::IDENTITY);
			let layer_bounds = instance.instance.bounding_box().unwrap_or_default();

			let to_point = |p: DVec2| kurbo::Point::new(p.x, p.y);
			let mut path = kurbo::BezPath::new();
			for subpath in instance.instance.stroke_bezier_paths() {
				subpath.to_vello_path(applied_stroke_transform, &mut path);
			}

			// If we're using opacity or a blend mode, we need to push a layer
			let blend_mode = match render_params.view_mode {
				ViewMode::Outline => peniko::Mix::Normal,
				_ => instance.alpha_blending.blend_mode.into(),
			};
			let mut layer = false;
			if instance.alpha_blending.opacity < 1. || instance.alpha_blending.blend_mode != BlendMode::default() {
				layer = true;
				scene.push_layer(
					peniko::BlendMode::new(blend_mode, peniko::Compose::SrcOver),
					instance.alpha_blending.opacity,
					kurbo::Affine::new(multiplied_transform.to_cols_array()),
					&kurbo::Rect::new(layer_bounds[0].x, layer_bounds[0].y, layer_bounds[1].x, layer_bounds[1].y),
				);
			}

			// Render the path
			match render_params.view_mode {
				ViewMode::Outline => {
					let outline_stroke = kurbo::Stroke {
						width: LAYER_OUTLINE_STROKE_WEIGHT,
						miter_limit: 4.,
						join: kurbo::Join::Miter,
						start_cap: kurbo::Cap::Butt,
						end_cap: kurbo::Cap::Butt,
						dash_pattern: Default::default(),
						dash_offset: 0.,
					};
					let outline_color = peniko::Color::new([
						LAYER_OUTLINE_STROKE_COLOR.r(),
						LAYER_OUTLINE_STROKE_COLOR.g(),
						LAYER_OUTLINE_STROKE_COLOR.b(),
						LAYER_OUTLINE_STROKE_COLOR.a(),
					]);

					scene.stroke(&outline_stroke, kurbo::Affine::new(element_transform.to_cols_array()), outline_color, None, &path);
				}
				_ => {
					match instance.instance.style.fill() {
						Fill::Solid(color) => {
							let fill = peniko::Brush::Solid(peniko::Color::new([color.r(), color.g(), color.b(), color.a()]));
							scene.fill(peniko::Fill::NonZero, kurbo::Affine::new(element_transform.to_cols_array()), &fill, None, &path);
						}
						Fill::Gradient(gradient) => {
							let mut stops = peniko::ColorStops::new();
							for &(offset, color) in &gradient.stops {
								stops.push(peniko::ColorStop {
									offset: offset as f32,
									color: peniko::color::DynamicColor::from_alpha_color(peniko::Color::new([color.r(), color.g(), color.b(), color.a()])),
								});
							}
							// Compute bounding box of the shape to determine the gradient start and end points
							let bounds = instance.instance.nonzero_bounding_box();
							let bound_transform = DAffine2::from_scale_angle_translation(bounds[1] - bounds[0], 0., bounds[0]);

							let inverse_parent_transform = (parent_transform.matrix2.determinant() != 0.).then(|| parent_transform.inverse()).unwrap_or_default();
							let mod_points = inverse_parent_transform * multiplied_transform * bound_transform;

							let start = mod_points.transform_point2(gradient.start);
							let end = mod_points.transform_point2(gradient.end);

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
											end_center: to_point(start),
											end_radius: radius as f32,
										}
									}
								},
								stops,
								..Default::default()
							});
							// Vello does `element_transform * brush_transform` internally. We don't want element_transform to have any impact so we need to left multiply by the inverse.
							// This makes the final internal brush transform equal to `parent_transform`, allowing you to stretch a gradient by transforming the parent folder.
							let inverse_element_transform = (element_transform.matrix2.determinant() != 0.).then(|| element_transform.inverse()).unwrap_or_default();
							let brush_transform = kurbo::Affine::new((inverse_element_transform * parent_transform).to_cols_array());
							scene.fill(peniko::Fill::NonZero, kurbo::Affine::new(element_transform.to_cols_array()), &fill, Some(brush_transform), &path);
						}
						Fill::None => {}
					};

					if let Some(stroke) = instance.instance.style.stroke() {
						let color = match stroke.color {
							Some(color) => peniko::Color::new([color.r(), color.g(), color.b(), color.a()]),
							None => peniko::Color::TRANSPARENT,
						};
						let cap = match stroke.line_cap {
							LineCap::Butt => Cap::Butt,
							LineCap::Round => Cap::Round,
							LineCap::Square => Cap::Square,
						};
						let join = match stroke.line_join {
							LineJoin::Miter => Join::Miter,
							LineJoin::Bevel => Join::Bevel,
							LineJoin::Round => Join::Round,
						};
						let stroke = kurbo::Stroke {
							width: stroke.weight,
							miter_limit: stroke.line_join_miter_limit,
							join,
							start_cap: cap,
							end_cap: cap,
							dash_pattern: stroke.dash_lengths.into(),
							dash_offset: stroke.dash_offset,
						};

						// Draw the stroke if it's visible
						if stroke.width > 0. {
							scene.stroke(&stroke, kurbo::Affine::new(element_transform.to_cols_array()), color, None, &path);
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

	fn bounding_box(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		self.instances()
			.flat_map(|instance| {
				let stroke_width = instance.instance.style.stroke().map(|s| s.weight()).unwrap_or_default();

				let miter_limit = instance.instance.style.stroke().map(|s| s.line_join_miter_limit).unwrap_or(1.);

				let scale = transform.decompose_scale();

				// We use the full line width here to account for different styles of line caps
				let offset = DVec2::splat(stroke_width * scale.x.max(scale.y) * miter_limit);

				instance.instance.bounding_box_with_transform(transform * *instance.transform).map(|[a, b]| [a - offset, b + offset])
			})
			.reduce(Quad::combine_bounds)
	}

	fn collect_metadata(&self, metadata: &mut RenderMetadata, mut footprint: Footprint, element_id: Option<NodeId>) {
		let instance_transform = self.transform();

		for instance in self.instances().map(|instance| instance.instance) {
			if let Some(element_id) = element_id {
				let stroke_width = instance.style.stroke().as_ref().map_or(0., Stroke::weight);
				let filled = instance.style.fill() != &Fill::None;
				let fill = |mut subpath: bezier_rs::Subpath<_>| {
					if filled {
						subpath.set_closed(true);
					}
					subpath
				};

				let click_targets = instance
					.stroke_bezier_paths()
					.map(fill)
					.map(|subpath| ClickTarget::new(subpath, stroke_width))
					.collect::<Vec<ClickTarget>>();

				metadata.click_targets.insert(element_id, click_targets);
			}

			if let Some(upstream_graphic_group) = &instance.upstream_graphic_group {
				footprint.transform *= instance_transform;
				upstream_graphic_group.collect_metadata(metadata, footprint, None);
			}
		}
	}

	fn add_upstream_click_targets(&self, click_targets: &mut Vec<ClickTarget>) {
		for instance in self.instances() {
			let stroke_width = instance.instance.style.stroke().as_ref().map_or(0., Stroke::weight);
			let filled = instance.instance.style.fill() != &Fill::None;
			let fill = |mut subpath: bezier_rs::Subpath<_>| {
				if filled {
					subpath.set_closed(true);
				}
				subpath
			};
			click_targets.extend(instance.instance.stroke_bezier_paths().map(fill).map(|subpath| {
				let mut click_target = ClickTarget::new(subpath, stroke_width);
				click_target.apply_transform(*instance.transform);
				click_target
			}));
		}
	}

	fn new_ids_from_hash(&mut self, reference: Option<NodeId>) {
		for instance in self.instances_mut() {
			instance.instance.vector_new_ids_from_hash(reference.map(|id| id.0).unwrap_or_default());
		}
	}

	fn to_graphic_element(&self) -> GraphicElement {
		GraphicElement::VectorData(self.clone())
	}
}

impl GraphicElementRendered for Artboard {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
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

		// Contents group (includes the artwork but not the background)
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
			// Artboard contents
			|render| {
				self.graphic_group.render_svg(render, render_params);
			},
		);
	}

	#[cfg(feature = "vello")]
	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2, context: &mut RenderContext, render_params: &RenderParams) {
		use vello::peniko;

		// Render background
		let color = peniko::Color::new([self.background.r(), self.background.g(), self.background.b(), self.background.a()]);
		let [a, b] = [self.location.as_dvec2(), self.location.as_dvec2() + self.dimensions.as_dvec2()];
		let rect = kurbo::Rect::new(a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y));
		let blend_mode = peniko::BlendMode::new(peniko::Mix::Clip, peniko::Compose::SrcOver);

		scene.push_layer(peniko::Mix::Normal, 1., kurbo::Affine::new(transform.to_cols_array()), &rect);
		scene.fill(peniko::Fill::NonZero, kurbo::Affine::new(transform.to_cols_array()), color, None, &rect);
		scene.pop_layer();

		if self.clip {
			scene.push_layer(blend_mode, 1., kurbo::Affine::new(transform.to_cols_array()), &rect);
		}
		// Since the graphic group's transform is right multiplied in when rendering the graphic group, we just need to right multiply by the offset here.
		let child_transform = transform * DAffine2::from_translation(self.location.as_dvec2());
		self.graphic_group.render_to_vello(scene, child_transform, context, render_params);
		if self.clip {
			scene.pop_layer();
		}
	}

	fn bounding_box(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		let artboard_bounds = (transform * Quad::from_box([self.location.as_dvec2(), self.location.as_dvec2() + self.dimensions.as_dvec2()])).bounding_box();
		if self.clip {
			Some(artboard_bounds)
		} else {
			[self.graphic_group.bounding_box(transform), Some(artboard_bounds)].into_iter().flatten().reduce(Quad::combine_bounds)
		}
	}

	fn collect_metadata(&self, metadata: &mut RenderMetadata, mut footprint: Footprint, element_id: Option<NodeId>) {
		if let Some(element_id) = element_id {
			let subpath = Subpath::new_rect(DVec2::ZERO, self.dimensions.as_dvec2());
			metadata.click_targets.insert(element_id, vec![ClickTarget::new(subpath, 0.)]);
			metadata.upstream_footprints.insert(element_id, footprint);
			metadata.local_transforms.insert(element_id, DAffine2::from_translation(self.location.as_dvec2()));
			if self.clip {
				metadata.clip_targets.insert(element_id);
			}
		}
		footprint.transform *= self.transform();
		self.graphic_group.collect_metadata(metadata, footprint, None);
	}

	fn add_upstream_click_targets(&self, click_targets: &mut Vec<ClickTarget>) {
		let subpath_rectangle = Subpath::new_rect(DVec2::ZERO, self.dimensions.as_dvec2());
		click_targets.push(ClickTarget::new(subpath_rectangle, 0.));
	}

	fn contains_artboard(&self) -> bool {
		true
	}
}

impl GraphicElementRendered for ArtboardGroupTable {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		for artboard in self.instances() {
			artboard.instance.render_svg(render, render_params);
		}
	}

	#[cfg(feature = "vello")]
	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2, context: &mut RenderContext, render_params: &RenderParams) {
		for instance in self.instances() {
			instance.instance.render_to_vello(scene, transform, context, render_params);
		}
	}

	fn bounding_box(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		self.instances().filter_map(|instance| instance.instance.bounding_box(transform)).reduce(Quad::combine_bounds)
	}

	fn collect_metadata(&self, metadata: &mut RenderMetadata, footprint: Footprint, _element_id: Option<NodeId>) {
		for instance in self.instances() {
			instance.instance.collect_metadata(metadata, footprint, *instance.source_node_id);
		}
	}

	fn add_upstream_click_targets(&self, click_targets: &mut Vec<ClickTarget>) {
		for instance in self.instances() {
			instance.instance.add_upstream_click_targets(click_targets);
		}
	}

	fn contains_artboard(&self) -> bool {
		self.instances().count() > 0
	}
}

impl GraphicElementRendered for ImageFrameTable<Color> {
	fn render_svg(&self, render: &mut SvgRender, _render_params: &RenderParams) {
		for instance in self.instances() {
			let transform = *instance.transform * render.transform;

			let image = &instance.instance;
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
				attributes.push("href", base64_string);
				let matrix = format_transform_matrix(transform);
				if !matrix.is_empty() {
					attributes.push("transform", matrix);
				}
				if instance.alpha_blending.opacity < 1. {
					attributes.push("opacity", instance.alpha_blending.opacity.to_string());
				}
				if instance.alpha_blending.blend_mode != BlendMode::default() {
					attributes.push("style", instance.alpha_blending.blend_mode.render());
				}
			});
		}
	}

	#[cfg(feature = "vello")]
	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2, _: &mut RenderContext, _render_params: &RenderParams) {
		use vello::peniko;

		for instance in self.instances() {
			let image = &instance.instance;
			if image.data.is_empty() {
				return;
			}
			let image = vello::peniko::Image::new(image.to_flat_u8().0.into(), peniko::Format::Rgba8, image.width, image.height).with_extend(peniko::Extend::Repeat);
			let transform = transform * *instance.transform * DAffine2::from_scale(1. / DVec2::new(image.width as f64, image.height as f64));

			scene.draw_image(&image, vello::kurbo::Affine::new(transform.to_cols_array()));
		}
	}

	fn bounding_box(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		self.instances()
			.flat_map(|instance| {
				let transform = transform * *instance.transform;
				(transform.matrix2.determinant() != 0.).then(|| (transform * Quad::from_box([DVec2::ZERO, DVec2::ONE])).bounding_box())
			})
			.reduce(Quad::combine_bounds)
	}

	fn collect_metadata(&self, metadata: &mut RenderMetadata, footprint: Footprint, element_id: Option<NodeId>) {
		let instance_transform = self.transform();

		let Some(element_id) = element_id else { return };
		let subpath = Subpath::new_rect(DVec2::ZERO, DVec2::ONE);

		metadata.click_targets.insert(element_id, vec![ClickTarget::new(subpath, 0.)]);
		metadata.upstream_footprints.insert(element_id, footprint);
		metadata.local_transforms.insert(element_id, instance_transform);
	}

	fn add_upstream_click_targets(&self, click_targets: &mut Vec<ClickTarget>) {
		let subpath = Subpath::new_rect(DVec2::ZERO, DVec2::ONE);
		click_targets.push(ClickTarget::new(subpath, 0.));
	}
}

impl GraphicElementRendered for RasterFrame {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		match self {
			RasterFrame::ImageFrame(image) => image.render_svg(render, render_params),
			RasterFrame::TextureFrame(_) => unimplemented!(),
		}
	}

	#[cfg(feature = "vello")]
	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2, context: &mut RenderContext, _render_params: &RenderParams) {
		use vello::peniko;

		let mut render_stuff = |image: vello::peniko::Image, blend_mode: crate::AlphaBlending| {
			let image_transform = transform * self.transform() * DAffine2::from_scale(1. / DVec2::new(image.width as f64, image.height as f64));
			let layer = blend_mode != Default::default();

			let Some(bounds) = self.bounding_box(transform) else { return };
			let blending = vello::peniko::BlendMode::new(blend_mode.blend_mode.into(), vello::peniko::Compose::SrcOver);

			if layer {
				let rect = vello::kurbo::Rect::new(bounds[0].x, bounds[0].y, bounds[1].x, bounds[1].y);
				scene.push_layer(blending, blend_mode.opacity, kurbo::Affine::IDENTITY, &rect);
			}
			scene.draw_image(&image, vello::kurbo::Affine::new(image_transform.to_cols_array()));
			if layer {
				scene.pop_layer()
			}
		};

		match self {
			RasterFrame::ImageFrame(image) => {
				for instance in image.instances() {
					let image = &instance.instance;
					if image.data.is_empty() {
						return;
					}

					let image = vello::peniko::Image::new(image.to_flat_u8().0.into(), peniko::Format::Rgba8, image.width, image.height).with_extend(peniko::Extend::Repeat);

					render_stuff(image, *instance.alpha_blending);
				}
			}
			RasterFrame::TextureFrame(image_texture) => {
				for instance in image_texture.instances() {
					let image =
						vello::peniko::Image::new(vec![].into(), peniko::Format::Rgba8, instance.instance.texture.width(), instance.instance.texture.height()).with_extend(peniko::Extend::Repeat);

					let id = image.data.id();
					context.resource_overrides.insert(id, instance.instance.texture.clone());

					render_stuff(image, *instance.alpha_blending);
				}
			}
		}
	}

	fn bounding_box(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		let transform = transform * self.transform();
		(transform.matrix2.determinant() != 0.).then(|| (transform * Quad::from_box([DVec2::ZERO, DVec2::ONE])).bounding_box())
	}

	fn collect_metadata(&self, metadata: &mut RenderMetadata, footprint: Footprint, element_id: Option<NodeId>) {
		let Some(element_id) = element_id else { return };

		let subpath = Subpath::new_rect(DVec2::ZERO, DVec2::ONE);
		metadata.click_targets.insert(element_id, vec![ClickTarget::new(subpath, 0.)]);
		metadata.upstream_footprints.insert(element_id, footprint);
		metadata.local_transforms.insert(element_id, self.transform());
	}

	fn add_upstream_click_targets(&self, click_targets: &mut Vec<ClickTarget>) {
		let subpath = Subpath::new_rect(DVec2::ZERO, DVec2::ONE);
		click_targets.push(ClickTarget::new(subpath, 0.));
	}
}

impl GraphicElementRendered for GraphicElement {
	fn render_svg(&self, render: &mut SvgRender, render_params: &RenderParams) {
		match self {
			GraphicElement::VectorData(vector_data) => vector_data.render_svg(render, render_params),
			GraphicElement::RasterFrame(raster) => raster.render_svg(render, render_params),
			GraphicElement::GraphicGroup(graphic_group) => graphic_group.render_svg(render, render_params),
		}
	}

	#[cfg(feature = "vello")]
	fn render_to_vello(&self, scene: &mut Scene, transform: DAffine2, context: &mut RenderContext, render_params: &RenderParams) {
		match self {
			GraphicElement::VectorData(vector_data) => vector_data.render_to_vello(scene, transform, context, render_params),
			GraphicElement::GraphicGroup(graphic_group) => graphic_group.render_to_vello(scene, transform, context, render_params),
			GraphicElement::RasterFrame(raster) => raster.render_to_vello(scene, transform, context, render_params),
		}
	}

	fn bounding_box(&self, transform: DAffine2) -> Option<[DVec2; 2]> {
		match self {
			GraphicElement::VectorData(vector_data) => vector_data.bounding_box(transform),
			GraphicElement::RasterFrame(raster) => raster.bounding_box(transform),
			GraphicElement::GraphicGroup(graphic_group) => graphic_group.bounding_box(transform),
		}
	}

	fn collect_metadata(&self, metadata: &mut RenderMetadata, footprint: Footprint, element_id: Option<NodeId>) {
		if let Some(element_id) = element_id {
			match self {
				GraphicElement::GraphicGroup(_) => {
					metadata.upstream_footprints.insert(element_id, footprint);
				}
				GraphicElement::VectorData(vector_data) => {
					metadata.upstream_footprints.insert(element_id, footprint);
					metadata.local_transforms.insert(element_id, vector_data.transform());
				}
				GraphicElement::RasterFrame(raster_frame) => {
					metadata.upstream_footprints.insert(element_id, footprint);
					metadata.local_transforms.insert(element_id, raster_frame.transform());
				}
			}
		}

		match self {
			GraphicElement::VectorData(vector_data) => vector_data.collect_metadata(metadata, footprint, element_id),
			GraphicElement::RasterFrame(raster) => raster.collect_metadata(metadata, footprint, element_id),
			GraphicElement::GraphicGroup(graphic_group) => graphic_group.collect_metadata(metadata, footprint, element_id),
		}
	}

	fn add_upstream_click_targets(&self, click_targets: &mut Vec<ClickTarget>) {
		match self {
			GraphicElement::VectorData(vector_data) => vector_data.add_upstream_click_targets(click_targets),
			GraphicElement::RasterFrame(raster) => raster.add_upstream_click_targets(click_targets),
			GraphicElement::GraphicGroup(graphic_group) => graphic_group.add_upstream_click_targets(click_targets),
		}
	}

	fn contains_artboard(&self) -> bool {
		match self {
			GraphicElement::VectorData(vector_data) => vector_data.contains_artboard(),
			GraphicElement::GraphicGroup(graphic_group) => graphic_group.contains_artboard(),
			GraphicElement::RasterFrame(raster) => raster.contains_artboard(),
		}
	}

	fn new_ids_from_hash(&mut self, reference: Option<NodeId>) {
		match self {
			GraphicElement::VectorData(vector_data) => vector_data.new_ids_from_hash(reference),
			GraphicElement::GraphicGroup(graphic_group) => graphic_group.new_ids_from_hash(reference),
			GraphicElement::RasterFrame(_) => (),
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

impl<P: Primitive> GraphicElementRendered for P {
	fn render_svg(&self, render: &mut SvgRender, _render_params: &RenderParams) {
		render.parent_tag("text", text_attributes, |render| render.leaf_node(format!("{self}")));
	}

	fn bounding_box(&self, _transform: DAffine2) -> Option<[DVec2; 2]> {
		None
	}

	#[cfg(feature = "vello")]
	fn render_to_vello(&self, _scene: &mut Scene, _transform: DAffine2, _context: &mut RenderContext, _render_params: &RenderParams) {}
}

impl GraphicElementRendered for Option<Color> {
	fn render_svg(&self, render: &mut SvgRender, _render_params: &RenderParams) {
		let Some(color) = self else {
			render.parent_tag("text", |_| {}, |render| render.leaf_node("Empty color"));
			return;
		};
		let color_info = format!("{:?} #{} {:?}", color, color.to_rgba_hex_srgb(), color.to_rgba8_srgb());

		render.leaf_tag("rect", |attributes| {
			attributes.push("width", "100");
			attributes.push("height", "100");
			attributes.push("y", "40");
			attributes.push("fill", format!("#{}", color.to_rgb_hex_srgb_from_gamma()));
			if color.a() < 1. {
				attributes.push("fill-opacity", ((color.a() * 1000.).round() / 1000.).to_string());
			}
		});
		render.parent_tag("text", text_attributes, |render| render.leaf_node(color_info))
	}

	fn bounding_box(&self, _transform: DAffine2) -> Option<[DVec2; 2]> {
		None
	}

	#[cfg(feature = "vello")]
	fn render_to_vello(&self, _scene: &mut Scene, _transform: DAffine2, _context: &mut RenderContext, _render_params: &RenderParams) {}
}

impl GraphicElementRendered for Vec<Color> {
	fn render_svg(&self, render: &mut SvgRender, _render_params: &RenderParams) {
		for (index, &color) in self.iter().enumerate() {
			render.leaf_tag("rect", |attributes| {
				attributes.push("width", "100");
				attributes.push("height", "100");
				attributes.push("x", (index * 120).to_string());
				attributes.push("y", "40");
				attributes.push("fill", format!("#{}", color.to_rgb_hex_srgb_from_gamma()));
				if color.a() < 1. {
					attributes.push("fill-opacity", ((color.a() * 1000.).round() / 1000.).to_string());
				}
			});
		}
	}

	fn bounding_box(&self, _transform: DAffine2) -> Option<[DVec2; 2]> {
		None
	}

	#[cfg(feature = "vello")]
	fn render_to_vello(&self, _scene: &mut Scene, _transform: DAffine2, _context: &mut RenderContext, _render_params: &RenderParams) {}
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

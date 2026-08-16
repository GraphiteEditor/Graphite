use std::ops::{Add, Mul, Sub};

use crate::renderer::{gradient_placement, singular_values, transform_is_invertible};
use crate::to_peniko::ToPenikoColor;
use core_types::{Color, color::SRGBA8};
use glam::{DAffine2, DMat2, DVec2, Vec4};
use image::ImageEncoder;
use kurbo::BezPath;
use vector_types::{
	gradient::MeshGradient,
	mesh_gradient::{MeshGradientEvaluator, MeshPatchEvaluator},
};
use vello::{Scene, peniko};

/// Maximum allowed geometry approximation error in viewport pixels.
pub(super) const MESH_POSITION_ERROR_TOLERANCE: f64 = 1.5;
/// Maximum allowed color approximation error per channel.
pub(super) const MESH_COLOR_ERROR_TOLERANCE: f32 = 0.5 / 255.;
/// Smallest subpatch dimension allowed in viewport pixels.
pub(super) const MESH_MINIMUM_SUBPATCH_SIZE: f64 = 4.;
/// Source padding in viewport pixels for displacement-map numerical error.
pub(super) const DISPLACEMENT_MAP_INFLATION_IN_VIEWPORT_PX: f64 = 5.;
/// Patch padding in viewport pixels for hiding anti-aliasing gaps.
pub(super) const PATCH_INFLATION_IN_VIEWPORT_PX: f64 = 1.;

/// Width and height of each generated displacement map.
const DISPLACEMENT_MAP_SIZE: u32 = 128;
/// Maximum local inflation applied to a subpatch clip.
const MESH_MAXIMUM_CLIP_INFLATION: f64 = 0.5;

// ===================
// Color approximation
// ===================

/// Returns adaptively sampled points that approximate a function with linear segments.
fn linear_approximated_points<T>(func: &impl Fn(f32) -> T, error: &impl Fn(T, T) -> f32, start: f32, end: f32, depth: usize) -> Vec<(f32, T)>
where
	T: Copy + Add<Output = T> + Sub<Output = T> + Mul<f32, Output = T>,
{
	// Maximum error allowed between a function and its linear approximation.
	const ERROR_TOLERANCE: f32 = 1. / 255.;
	// Relative positions sampled within each candidate interval.
	const SAMPLES: [f32; 3] = [0.25, 0.5, 0.75];
	// Maximum depth of adaptive interval subdivision.
	const MAX_DEPTH: usize = 8;

	let start_result = func(start);
	let end_result = func(end);
	let needs_split = SAMPLES.iter().any(|&sample| {
		let t = start + (end - start) * sample;
		error(start_result + (end_result - start_result) * sample, func(t)) > ERROR_TOLERANCE
	});

	if needs_split && depth < MAX_DEPTH {
		let mid = (start + end) / 2.;
		let mut points = linear_approximated_points(func, error, start, mid, depth + 1);
		points.extend(linear_approximated_points(func, error, mid, end, depth + 1).into_iter().skip(1));
		points
	} else {
		vec![(start, start_result), (end, end_result)]
	}
}

/// Returns a source-over-adjusted Bernstein weight for the indexed mask layer.
pub(super) fn eval_source_over_bezier_alpha(index: usize, time: f32) -> f32 {
	match index {
		0 => (1. - time).powi(3),
		1 => 3. * (1. - time).powi(2) / (time.powi(2) - 3. * time + 3.),
		2 => 3. * (1. - time) / (3. - 2. * time),
		_ => unreachable!(),
	}
}

/// Evaluates a cubic Bezier color curve at the given parameter.
pub(super) fn eval_cubic_bezier_color(control_points: [Vec4; 4], time: f32) -> Vec4 {
	let one_minus_t = 1. - time;
	control_points[0] * one_minus_t.powi(3) + control_points[1] * (3. * time * one_minus_t.powi(2)) + control_points[2] * (3. * time.powi(2) * one_minus_t) + control_points[3] * time.powi(3)
}

/// Quantizes gamma-encoded floating-point color channels into sRGBA8.
fn gamma_color_to_srgba8(color: [f32; 4]) -> SRGBA8 {
	let float_to_u8 = |x: f32| (x.clamp(0., 1.) * 255.).round() as u8;
	SRGBA8 {
		red: float_to_u8(color[0]),
		green: float_to_u8(color[1]),
		blue: float_to_u8(color[2]),
		alpha: float_to_u8(color[3]),
	}
}

// =====================
// SVG displacement maps
// =====================

/// Returns the displacements from a unit rectangle to bounding box of a coons patch.
/// The values are pairs of (original position, target position).
pub(super) fn unit_to_coons_bbox_displacements(patch_evaluator: &MeshPatchEvaluator, displacement_map_to_patch: &DAffine2, inflated_map_sizes: &[f64; 4]) -> Vec<(DVec2, DVec2)> {
	let [inflated_map_x, inflated_map_y, inflated_map_width, inflated_map_height] = inflated_map_sizes;

	let mut displacements: Vec<(DVec2, DVec2)> = vec![];
	// 81 samples of (uv, position) tuples in the patch.
	let inverse_seeds = {
		// Number of initial intervals sampled along each patch axis.
		const INITIAL_SUBDIVISIONS: usize = 8;
		let seed_count = (INITIAL_SUBDIVISIONS + 1).pow(2);
		let mut seeds = Vec::with_capacity(seed_count);
		for row in 0..=INITIAL_SUBDIVISIONS {
			let v = row as f64 / INITIAL_SUBDIVISIONS as f64;

			for column in 0..=INITIAL_SUBDIVISIONS {
				let u = column as f64 / INITIAL_SUBDIVISIONS as f64;
				let uv = DVec2::new(u, v);
				seeds.push((uv, patch_evaluator.eval_position(u, v)));
			}
		}
		seeds
	};

	for y in 0..DISPLACEMENT_MAP_SIZE {
		for x in 0..DISPLACEMENT_MAP_SIZE {
			// Adds 0.5 to evalute the center of the pixel
			let s = (x as f64 + 0.5) / DISPLACEMENT_MAP_SIZE as f64;
			let t = (y as f64 + 0.5) / DISPLACEMENT_MAP_SIZE as f64;

			// Position in the displaced result. This can be larger than [0, 1].
			let target_pos = DVec2::new(inflated_map_x + s * inflated_map_width, inflated_map_y + t * inflated_map_height);
			let target_mesh_pos = displacement_map_to_patch.transform_point2(target_pos);
			// Calculate the original position where the target position is projected from. This should be [0, 1].
			let initial_uv = inverse_seeds
				.iter()
				.min_by(|(_, first_position), (_, second_position)| first_position.distance_squared(target_mesh_pos).total_cmp(&second_position.distance_squared(target_mesh_pos)))
				.map(|(uv, _)| *uv)
				.unwrap_or(DVec2::splat(0.5));
			let source_pos = patch_evaluator.inverse_patch_position(target_mesh_pos, initial_uv);

			displacements.push((source_pos, target_pos));
		}
	}

	displacements
}

/// Collect pairs from a position in a source unit rectangle and a position in the target coons patch.
pub(super) fn displacements_to_map_png(displacements: &[(DVec2, DVec2)], scale: f64) -> Vec<u8> {
	let mut rgba16_bytes = Vec::with_capacity((DISPLACEMENT_MAP_SIZE * DISPLACEMENT_MAP_SIZE * 4 * size_of::<u16>() as u32) as usize);

	let encode_displacement = |source: f64, target: f64| {
		let max_channel = u16::MAX as f64;
		let ideal = (0.5 + (source - target) / scale) * max_channel;
		let minimum = ((0.5 - target / scale) * max_channel).ceil().max(0.);
		let maximum = ((0.5 + (1. - target) / scale) * max_channel).floor().min(max_channel);

		ideal.round().clamp(minimum, maximum) as u16
	};
	for displacement in displacements {
		let (source_pos, target_pos) = displacement;
		let red = encode_displacement(source_pos.x, target_pos.x);
		let green = encode_displacement(source_pos.y, target_pos.y);

		for channel in [red, green, 0, u16::MAX] {
			rgba16_bytes.extend_from_slice(&channel.to_ne_bytes());
		}
	}

	let mut displacement_map_png = Vec::new();
	::image::codecs::png::PngEncoder::new(&mut displacement_map_png)
		.write_image(&rgba16_bytes, DISPLACEMENT_MAP_SIZE, DISPLACEMENT_MAP_SIZE, ::image::ExtendedColorType::Rgba16)
		.expect("failed to encode displacement map as 16-bit PNG");

	displacement_map_png
}

// SVG gradient definitions

/// Returns an SVG gradient stop element for the given gamma-encoded color.
fn gradient_stop_element(offset: f32, opacity: f32, gamma_color: [f32; 4]) -> String {
	let offset = (offset.clamp(0., 1.) * 1_000_000.).round() / 1_000_000.;
	let opacity = (opacity.clamp(0., 1.) * 1000.).round() / 1000.;
	format!(
		r##"<stop offset="{offset}" stop-color="#{}" stop-opacity="{opacity}"/>"##,
		gamma_color_to_srgba8(gamma_color).to_rgb_hex(),
	)
}

/// Returns SVG gradient stops that approximate a scalar alpha function.
pub(super) fn alpha_func_to_gradient_stops_string(func: &impl Fn(f32) -> f32) -> String {
	let error_func = |a: f32, b: f32| (a - b).abs();
	linear_approximated_points(func, &error_func, 0., 1., 0)
		.into_iter()
		.map(|(arg, result)| gradient_stop_element(arg, result, Color::WHITE.to_gamma_srgb_channels()))
		.collect::<String>()
}

/// Returns SVG gradient stops that approximate a u-direction color curve.
pub(super) fn u_color_curves_to_gradient_stops_string(func: &impl Fn(f32) -> Vec4) -> String {
	let error_func = |a: Vec4, b: Vec4| (a - b).abs().max_element();
	linear_approximated_points(func, &error_func, 0., 1., 0)
		.into_iter()
		.map(|(arg, result)| gradient_stop_element(arg, 1., result.to_array()))
		.collect::<String>()
}

/// Encodes a scalar alpha curve as an opaque grayscale gradient for use by a luminance mask.
pub(super) fn u_alpha_curve_to_gradient_stops_string(func: &impl Fn(f32) -> f32) -> String {
	let error_func = |a: f32, b: f32| (a - b).abs();
	linear_approximated_points(func, &error_func, 0., 1., 0)
		.into_iter()
		.map(|(offset, alpha)| gradient_stop_element(offset, 1., [alpha, alpha, alpha, 1.]))
		.collect::<String>()
}

// ==============================
// Vello subdivision and geometry
// ==============================

pub(super) struct MeshSubpatch {
	corner_positions: [DVec2; 4],
	pub(super) patch_index: usize,
	uv_bounds: [DVec2; 2],
}

/// Recursively subdivides regions until their parallelogram approximation is within the position and color tolerances.
pub(super) fn subdivide_patches_adaptive(
	evaluator: &MeshGradientEvaluator,
	minimum_subpatch_size: f64,
	mesh_transform: DAffine2,
	parent_transform: DAffine2,
	position_error_tolerance: f64,
	color_error_tolerance: f32,
) -> Option<Vec<MeshSubpatch>> {
	if !minimum_subpatch_size.is_finite()
		|| minimum_subpatch_size < 0.
		|| !position_error_tolerance.is_finite()
		|| position_error_tolerance < 0.
		|| !color_error_tolerance.is_finite()
		|| color_error_tolerance < 0.
	{
		return None;
	}

	let samples = [0., 0.25, 0.5, 0.75, 1.];
	let mut subpatches = Vec::new();
	for (patch_index, patch) in evaluator.patch_evaluators().enumerate() {
		let mut pending = vec![(0., 0., 1.)];
		while let Some((u_start, v_start, stride)) = pending.pop() {
			let corner_uvs = [
				DVec2::new(u_start, v_start),
				DVec2::new(u_start + stride, v_start),
				DVec2::new(u_start, v_start + stride),
				DVec2::new(u_start + stride, v_start + stride),
			];
			let corner_positions = corner_uvs.map(|uv| mesh_transform.transform_point2(patch.eval_position(uv.x, uv.y)));
			let [top_left_pos, top_right_pos, bottom_left_pos, _bottom_right_pos] = corner_positions;

			let patch_to_viewport = parent_transform * mesh_transform;
			let [top_left, top_right, bottom_left, bottom_right] = corner_uvs.map(|uv| patch_to_viewport.transform_point2(patch.eval_position(uv.x, uv.y)));
			let u_size = top_left.distance(top_right).max(bottom_left.distance(bottom_right));
			let v_size = top_left.distance(bottom_left).max(top_right.distance(bottom_right));
			if !u_size.is_finite() || !v_size.is_finite() {
				return None;
			}
			let reached_minimum_size = u_size.max(v_size) <= minimum_subpatch_size;

			let mut within_tolerance = true;
			'error_samples: for &local_v in &samples {
				for &local_u in &samples {
					let u = u_start + local_u * stride;
					let v = v_start + local_v * stride;
					let expected_pos = mesh_transform.transform_point2(patch.eval_position(u, v));
					let expected_color = Vec4::from_array(patch.eval_color(u as f32, v as f32));
					// Approximate the position with the rendered parallelogram and the color by linearly interpolating its cubic top and bottom color curves.
					let approximated_pos = top_left_pos + (top_right_pos - top_left_pos) * local_u + (bottom_left_pos - top_left_pos) * local_v;
					let top_color = Vec4::from_array(patch.eval_color(u as f32, v_start as f32));
					let bottom_color = Vec4::from_array(patch.eval_color(u as f32, (v_start + stride) as f32));
					let approximated_color = top_color.lerp(bottom_color, local_v as f32);

					let position_error = parent_transform.transform_vector2(expected_pos - approximated_pos).length();
					let color_error = (expected_color - approximated_color).abs().max_element();
					if !position_error.is_finite() || !color_error.is_finite() {
						return None;
					}
					if position_error > position_error_tolerance || color_error > color_error_tolerance {
						within_tolerance = false;
						break 'error_samples;
					}
				}
			}

			if within_tolerance || reached_minimum_size {
				subpatches.push(MeshSubpatch {
					corner_positions,
					patch_index,
					uv_bounds: [DVec2::new(u_start, v_start), DVec2::new(u_start + stride, v_start + stride)],
				});
			} else {
				let half_stride = stride / 2.;
				pending.extend([
					(u_start + half_stride, v_start + half_stride, half_stride),
					(u_start, v_start + half_stride, half_stride),
					(u_start + half_stride, v_start, half_stride),
					(u_start, v_start, half_stride),
				]);
			}
		}
	}

	Some(subpatches)
}

/// Returns the affine approximation of a subpatch, rejecting folded or degenerate geometry.
pub(super) fn mesh_subpatch_transform(subpatch: &MeshSubpatch) -> Option<DAffine2> {
	let [top_left, top_right, bottom_left, _] = subpatch.corner_positions;
	let transform = DAffine2::from_cols(top_right - top_left, bottom_left - top_left, top_left);
	let determinant = transform.matrix2.determinant();
	(determinant.is_finite() && determinant > 0.).then_some(transform)
}

/// Returns the union of all patch boundary paths in mesh-local coordinates.
pub(super) fn mesh_boundary_path(mesh_gradient: &MeshGradient) -> BezPath {
	let mut mesh_boundary = BezPath::new();
	for patch in mesh_gradient.patches().flatten() {
		let [top, bottom, left, right] = patch.edges;
		let mut patch_boundary = BezPath::from_path_segments([top, right, bottom.reverse(), left.reverse()].into_iter());
		patch_boundary.close_path();
		mesh_boundary.extend(patch_boundary);
	}
	mesh_boundary
}

/// Returns the local clip and paint inflation needed to hide gaps around a subpatch.
fn mesh_subpatch_inflation(subpatch: &MeshSubpatch) -> (f64, f64) {
	let [top_left, top_right, bottom_left, _] = subpatch.corner_positions;
	let subpatch_transform = DAffine2::from_cols(top_right - top_left, bottom_left - top_left, top_left);
	let (_, smallest_scale) = singular_values(subpatch_transform);
	let clip_inflation = if smallest_scale.is_finite() && smallest_scale > f64::EPSILON {
		(1. / smallest_scale).min(MESH_MAXIMUM_CLIP_INFLATION)
	} else {
		0.
	};

	(clip_inflation, clip_inflation * 2.)
}

// ========================
// Vello brush construction
// ========================

/// Builds a Vello linear gradient brush from sRGBA8 color stops.
fn vello_linear_gradient(start: DVec2, end: DVec2, stop_values: impl IntoIterator<Item = (f32, SRGBA8)>) -> peniko::Brush {
	let mut stops = peniko::ColorStops::new();
	for (offset, color) in stop_values {
		stops.push(peniko::ColorStop {
			offset,
			color: peniko::color::DynamicColor::from_alpha_color(color.to_peniko_color()),
		});
	}

	peniko::Brush::Gradient(peniko::Gradient {
		kind: peniko::LinearGradientPosition {
			start: kurbo::Point::new(start.x, start.y),
			end: kurbo::Point::new(end.x, end.y),
		}
		.into(),
		stops,
		extend: peniko::Extend::Pad,
		interpolation_alpha_space: peniko::InterpolationAlphaSpace::Unpremultiplied,
		..Default::default()
	})
}

/// Returns brush transforms that preserve horizontal and vertical gradient bands when the subpatch is sheared.
fn vello_subpatch_brush_transforms(subpatch_to_device: DAffine2) -> Option<(kurbo::Affine, kurbo::Affine)> {
	if !transform_is_invertible(subpatch_to_device) {
		return None;
	}

	let device_to_subpatch = subpatch_to_device.inverse();
	let horizontal_gradient_to_device = gradient_placement(subpatch_to_device, vector_types::gradient::GradientForm::Linear);

	let vertical_axis = subpatch_to_device.matrix2.y_axis;
	let vertical_band_normal = subpatch_to_device.matrix2.x_axis.perp();
	let vertical_line = if vertical_band_normal.length_squared() > 0. {
		vertical_axis.project_onto(vertical_band_normal)
	} else {
		vertical_axis
	};
	let vertical_gradient_to_device = DAffine2 {
		matrix2: DMat2::from_cols(vertical_line.perp(), vertical_line),
		translation: subpatch_to_device.translation,
	};

	Some((
		kurbo::Affine::new((device_to_subpatch * horizontal_gradient_to_device).to_cols_array()),
		kurbo::Affine::new((device_to_subpatch * vertical_gradient_to_device).to_cols_array()),
	))
}

struct VelloSubpatchBrushes {
	top_color: peniko::Brush,
	bottom_color: peniko::Brush,
	color_weight: peniko::Brush,
}

/// Builds a vertical Vello alpha mask that approximates a scalar function.
fn vello_vertical_mask(func: &impl Fn(f32) -> f32, start: f32, end: f32) -> peniko::Brush {
	let remap_offset = |value: f32| (value - start) / (end - start);
	let error = |a: f32, b: f32| (a - b).abs();
	let stops = linear_approximated_points(func, &error, start, end, 0).into_iter().map(|(v, alpha)| {
		(
			remap_offset(v),
			SRGBA8 {
				red: 255,
				green: 255,
				blue: 255,
				alpha: (alpha.clamp(0., 1.) * 255.).round() as u8,
			},
		)
	});
	vello_linear_gradient(DVec2::new(0.5, 0.), DVec2::new(0.5, 1.), stops)
}

/// Builds the opaque RGB approximation for one subpatch.
fn vello_subpatch_color_brushes(patch_evaluator: &MeshPatchEvaluator, subpatch: &MeshSubpatch) -> VelloSubpatchBrushes {
	let [uv_min, uv_max] = subpatch.uv_bounds.map(|uv| uv.as_vec2());
	let remap_offset = |value: f32, start: f32, end: f32| (value - start) / (end - start);

	// Preserve each cubic horizontal RGB edge with adaptive gradient stops. Alpha is applied after the RGB field is complete.
	let [top_color, bottom_color] = [uv_min.y, uv_max.y].map(|v| {
		let curve = |u| Vec4::from_array(patch_evaluator.eval_color(u, v));
		let error = |a: Vec4, b: Vec4| (a - b).abs().max_element();
		let stops = linear_approximated_points(&curve, &error, uv_min.x, uv_max.x, 0).into_iter().map(|(u, mut color)| {
			color.w = 1.;
			(remap_offset(u, uv_min.x, uv_max.x), gamma_color_to_srgba8(color.to_array()))
		});
		vello_linear_gradient(DVec2::ZERO, DVec2::X, stops)
	});

	// Project the cubic color curve at the horizontal midpoint onto the line between its edge colors.
	// The resulting scalar curve is the vertical alpha mask that best reproduces the interior color there.
	let center_u = (uv_min.x + uv_max.x) / 2.;
	let top_center_color = Vec4::from_array(patch_evaluator.eval_color(center_u, uv_min.y)).truncate();
	let bottom_center_color = Vec4::from_array(patch_evaluator.eval_color(center_u, uv_max.y)).truncate();
	let color_axis = top_center_color - bottom_center_color;
	let color_axis_length_squared = color_axis.length_squared();
	let color_weight_func = |v| {
		if color_axis_length_squared > f32::EPSILON {
			let color = Vec4::from_array(patch_evaluator.eval_color(center_u, v)).truncate();
			((color - bottom_center_color).dot(color_axis) / color_axis_length_squared).clamp(0., 1.)
		} else {
			1. - remap_offset(v, uv_min.y, uv_max.y)
		}
	};
	let color_weight = vello_vertical_mask(&color_weight_func, uv_min.y, uv_max.y);

	VelloSubpatchBrushes {
		top_color,
		bottom_color,
		color_weight,
	}
}

/// Builds an opaque grayscale approximation of a subpatch's alpha field.
fn vello_subpatch_alpha_brushes(patch_evaluator: &MeshPatchEvaluator, subpatch: &MeshSubpatch) -> VelloSubpatchBrushes {
	let [uv_min, uv_max] = subpatch.uv_bounds.map(|uv| uv.as_vec2());
	let remap_offset = |value: f32| (value - uv_min.x) / (uv_max.x - uv_min.x);
	let opaque_grayscale = |alpha: f32| {
		let alpha = alpha.clamp(0., 1.);
		gamma_color_to_srgba8([alpha, alpha, alpha, 1.])
	};

	// This matches the color approximation used to decide adaptive subdivision: preserve the cubic
	// horizontal edge curves, then interpolate them linearly in the local v direction.
	let [top_color, bottom_color] = [uv_min.y, uv_max.y].map(|v| {
		let curve = |u| patch_evaluator.eval_color(u, v)[3];
		let error = |a: f32, b: f32| (a - b).abs();
		let stops = linear_approximated_points(&curve, &error, uv_min.x, uv_max.x, 0)
			.into_iter()
			.map(|(u, alpha)| (remap_offset(u), opaque_grayscale(alpha)));
		vello_linear_gradient(DVec2::ZERO, DVec2::X, stops)
	});
	let color_weight = vello_vertical_mask(&|v| 1. - v, 0., 1.);

	VelloSubpatchBrushes {
		top_color,
		bottom_color,
		color_weight,
	}
}

// =================
// Vello compositing
// =================

/// Paints `brush` through `mask` into an isolated source-over layer.
fn render_vello_masked_brush(
	scene: &mut Scene,
	subpatch_to_scene: kurbo::Affine,
	paint_rect: &kurbo::Rect,
	brush: &peniko::Brush,
	brush_transform: kurbo::Affine,
	mask: &peniko::Brush,
	mask_transform: kurbo::Affine,
) {
	scene.push_layer(peniko::Fill::NonZero, peniko::Mix::Normal, 1., subpatch_to_scene, paint_rect);
	scene.fill(peniko::Fill::NonZero, subpatch_to_scene, mask, Some(mask_transform), paint_rect);
	scene.push_layer(
		peniko::Fill::NonZero,
		peniko::BlendMode::new(peniko::Mix::Normal, peniko::Compose::SrcIn),
		1.,
		subpatch_to_scene,
		paint_rect,
	);
	scene.fill(peniko::Fill::NonZero, subpatch_to_scene, brush, Some(brush_transform), paint_rect);
	scene.pop_layer();
	scene.pop_layer();
}

/// Renders the weighted top and bottom brushes into an inflated subpatch.
fn render_vello_subpatch_brushes(scene: &mut Scene, subpatch: &MeshSubpatch, parent_transform: DAffine2, brushes: VelloSubpatchBrushes) {
	let Some(subpatch_to_parent) = mesh_subpatch_transform(subpatch) else { return };

	let subpatch_to_device = parent_transform * subpatch_to_parent;
	let Some((horizontal_brush_transform, vertical_brush_transform)) = vello_subpatch_brush_transforms(subpatch_to_device) else {
		return;
	};
	let subpatch_to_scene = kurbo::Affine::new(subpatch_to_device.to_cols_array());
	let (clip_inflation, paint_inflation) = mesh_subpatch_inflation(subpatch);
	let clip_rect = kurbo::Rect::new(-clip_inflation, -clip_inflation, 1. + clip_inflation, 1. + clip_inflation);
	let paint_rect = kurbo::Rect::new(-paint_inflation, -paint_inflation, 1. + paint_inflation, 1. + paint_inflation);

	scene.push_layer(peniko::Fill::NonZero, peniko::Mix::Normal, 1., subpatch_to_scene, &clip_rect);
	scene.fill(peniko::Fill::NonZero, subpatch_to_scene, &brushes.bottom_color, Some(horizontal_brush_transform), &paint_rect);
	render_vello_masked_brush(
		scene,
		subpatch_to_scene,
		&paint_rect,
		&brushes.top_color,
		horizontal_brush_transform,
		&brushes.color_weight,
		vertical_brush_transform,
	);
	scene.pop_layer();
}

/// Renders the opaque RGB field of one adaptively subdivided patch.
pub(super) fn render_vello_subpatch_color(scene: &mut Scene, patch_evaluator: &MeshPatchEvaluator, subpatch: &MeshSubpatch, parent_transform: DAffine2) {
	let brushes = vello_subpatch_color_brushes(patch_evaluator, subpatch);
	render_vello_subpatch_brushes(scene, subpatch, parent_transform, brushes);
}

/// Adds one inflated, opaque grayscale subpatch to the mesh-wide luminance mask.
pub(super) fn render_vello_subpatch_alpha(scene: &mut Scene, patch_evaluator: &MeshPatchEvaluator, subpatch: &MeshSubpatch, parent_transform: DAffine2) {
	let brushes = vello_subpatch_alpha_brushes(patch_evaluator, subpatch);
	render_vello_subpatch_brushes(scene, subpatch, parent_transform, brushes);
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn adaptive_subdivision_accounts_for_color_error() {
		let mesh = MeshGradient::default();
		let evaluator = mesh.evaluator().unwrap();
		let geometry_only = subdivide_patches_adaptive(&evaluator, 0.125, DAffine2::IDENTITY, DAffine2::IDENTITY, f64::MAX, f32::MAX).unwrap();
		let with_color = subdivide_patches_adaptive(&evaluator, 0.125, DAffine2::IDENTITY, DAffine2::IDENTITY, f64::MAX, 0.).unwrap();

		assert!(with_color.len() > geometry_only.len());
	}

	#[test]
	fn adaptive_subdivision_rejects_non_finite_transform() {
		let mesh = MeshGradient::default();
		let evaluator = mesh.evaluator().unwrap();
		let non_finite_transform = DAffine2::from_scale(DVec2::splat(f64::NAN));

		assert!(subdivide_patches_adaptive(&evaluator, 0.125, DAffine2::IDENTITY, non_finite_transform, 0.25, 0.01).is_none());
	}
}

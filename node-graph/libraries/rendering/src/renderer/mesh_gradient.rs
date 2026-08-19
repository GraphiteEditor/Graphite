use std::collections::VecDeque;
use std::fmt::Write;
use std::ops::{Add, Mul, Sub};

use crate::renderer::{gradient_placement, singular_values, transform_is_invertible};
use crate::to_peniko::ToPenikoColor;
use crate::{SvgRender, format_transform_matrix};
use base64::Engine;
use core_types::uuid::generate_uuid;
use core_types::{Color, color::SRGBA8};
use glam::{DAffine2, DMat2, DVec2, Vec2, Vec4};
use image::ImageEncoder;
use kurbo::{Affine, BezPath, Shape};
use vector_types::GradientInterpolation;
use vector_types::gradient::MeshPatch;
use vector_types::{
	gradient::GradientSpace,
	mesh_gradient::{MeshGradientEvaluator, MeshPatchEvaluator},
};
use vello::{Scene, peniko};

/// Maximum allowed geometry approximation error in viewport pixels.
pub(super) const MESH_POSITION_ERROR_TOLERANCE: f64 = 1.5;
/// Maximum allowed color approximation error per channel.
pub(super) const MESH_COLOR_ERROR_TOLERANCE: f32 = 2. / 255.;
/// Maximum subpatches one mesh may divide into, bounding what a color field the tolerance cannot reach can allocate.
/// A mesh with more patches than this still emits one subpatch each, since a patch cannot render without its own region.
pub(super) const MESH_MAXIMUM_SUBPATCHES: usize = 4096;
/// Smallest uv stride a region may refine to.
const MINIMUM_SUBPATCH_STRIDE: f64 = 1. / 4096.;
/// Patch padding size for hiding anti-aliasing gaps.
pub(super) const PATCH_INFLATION_SIZE: f64 = 1.;

/// Width and height of each generated displacement map.
const DISPLACEMENT_MAP_SIZE: usize = 128;
/// Fraction of the displacement map reserved as margin on each side to absorb floating-point error.
const DISPLACEMENT_MAP_MARGIN_PERCENTAGE: f64 = 0.02;
/// Exterior texels evaluated around the patch to cover displacement-map filtering.
const DISPLACEMENT_MAP_OUTSIDE_BUFFER_TEXELS: usize = 2;
/// Maximum local inflation applied to a subpatch clip.
const MESH_MAXIMUM_CLIP_INFLATION: f64 = 0.5;

// ===================
// Color approximation
// ===================

/// Returns adaptively sampled points that approximate a function with linear segments.
fn linear_approximation_points<T>(func: &impl Fn(f32) -> T, error: &impl Fn(T, T) -> f32, start: f32, end: f32, depth: usize) -> Vec<(f32, T)>
where
	T: Copy + Add<Output = T> + Sub<Output = T> + Mul<f32, Output = T>,
{
	// Maximum error allowed between a function and its linear approximation.
	const ERROR_TOLERANCE: f32 = 2. / 255.;
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
		let mut points = linear_approximation_points(func, error, start, mid, depth + 1);
		points.extend(linear_approximation_points(func, error, mid, end, depth + 1).into_iter().skip(1));
		points
	} else {
		vec![(start, start_result), (end, end_result)]
	}
}

/// Returns a source-over-adjusted Bernstein weight for the indexed mask layer.
pub(super) fn evaluate_source_over_bezier_alpha(index: usize, time: f32) -> f32 {
	match index {
		0 => (1. - time).powi(3),
		1 => 3. * (1. - time).powi(2) / (time.powi(2) - 3. * time + 3.),
		2 => 3. * (1. - time) / (3. - 2. * time),
		_ => unreachable!(),
	}
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

/// Maximum allowed error between the stacked SVG color layers and the true color surface, per channel.
const SVG_LAYER_ERROR_TOLERANCE: f32 = 2. / 255.;
/// Maximum number of bisections used when placing the v-direction layer rows.
const SVG_LAYER_MAX_DEPTH: usize = 8;
/// Maximum layers stacked per patch, bounding what a mesh the tolerance cannot reach is allowed to emit.
const SVG_LAYER_MAX_COUNT: usize = 64;

/// The v-direction weights that blend the stacked SVG color layers.
#[derive(Clone, Debug)]
pub(super) enum SvgMeshVLayers {
	/// Uses top-left color for the entire patch, no blend required.
	Stepped,
	/// The four Bezier control rows blended by the Bernstein basis, reproducing the bicubic surface.
	BicubicBernstein,
	/// Surface rows sampled at the given v values, blended linearly between adjacent rows.
	LinearRows(Vec<f32>),
}

impl SvgMeshVLayers {
	/// Chooses the appropriate layer scheme for the chosen color space and interpolation method.
	pub(super) fn new(evaluator: &MeshGradientEvaluator) -> Self {
		match (evaluator.interpolation_method(), evaluator.space()) {
			// A smooth gamma-sRGB surface can be reproduced from its four Bezier control rows using Bernstein source-over weights.
			(GradientInterpolation::Smooth, GradientSpace::RgbGamma) => Self::BicubicBernstein,
			// A bilinear gamma-sRGB surface is exactly two horizontal linear rows blended by one linear vertical mask.
			(GradientInterpolation::Linear, GradientSpace::RgbGamma) => Self::LinearRows(vec![0., 1.]),
			// Conversion from the interpolation color space to gamma sRGB makes the rendered surface nonlinear, so approximate it with adaptive rows.
			(GradientInterpolation::Smooth | GradientInterpolation::Linear, _) => Self::LinearRows(Self::adaptive_row_knots(evaluator)),
			(GradientInterpolation::Stepped, _) => Self::Stepped,
		}
	}

	/// Adaptively places v-direction row knots until linear blending approximates the color surface within tolerance.
	fn adaptive_row_knots(evaluator: &MeshGradientEvaluator) -> Vec<f32> {
		// Vec of (start, end, error)
		let mut intervals = vec![(0_f32, 1_f32, linear_row_interval_error(evaluator, 0., 1.))];
		let smallest_interval = 1. / (1_u32 << SVG_LAYER_MAX_DEPTH) as f32;
		// Refine the interval with the largest error first.
		// Only failing intervals split, so the result matches an exhaustive subdivision unless the budget runs out.
		// One row set shared by every patch keeps the mask gradients mesh-wide.
		while intervals.len() < SVG_LAYER_MAX_COUNT - 1 {
			let worst_interval_index = intervals
				.iter()
				.enumerate()
				.filter(|&(_, &(start, end, error))| error > SVG_LAYER_ERROR_TOLERANCE && end - start > smallest_interval)
				.max_by(|(_, first), (_, second)| first.2.total_cmp(&second.2))
				.map(|(index, _)| index);
			let Some(worst_interval_index) = worst_interval_index else { break };

			let (start, end, _) = intervals.swap_remove(worst_interval_index);
			let middle = (start + end) / 2.;
			intervals.push((start, middle, linear_row_interval_error(evaluator, start, middle)));
			intervals.push((middle, end, linear_row_interval_error(evaluator, middle, end)));
		}
		intervals.sort_by(|first, second| first.0.total_cmp(&second.0));
		std::iter::once(0.).chain(intervals.iter().map(|&(_, end, _)| end)).collect()
	}

	pub(super) fn layer_count(&self) -> usize {
		match self {
			Self::Stepped => 1,
			Self::BicubicBernstein => 4,
			Self::LinearRows(knots) => knots.len(),
		}
	}

	/// Returns the alpha the indexed layer needs for source-over compositing to reproduce its weight.
	pub(super) fn source_over_alpha(&self, index: usize, v: f32) -> f32 {
		match self {
			Self::Stepped => 0.,
			Self::BicubicBernstein => evaluate_source_over_bezier_alpha(index, v),
			// Layers are painted bottom-up, so everything below `index` is already covered wherever this layer is opaque.
			// One clamped ramp per layer therefore composites into a linear blend of the two nearest rows.
			Self::LinearRows(knots) => ((knots[index + 1] - v) / (knots[index + 1] - knots[index])).clamp(0., 1.),
		}
	}

	/// The v range the indexed layer's weight ramps across, or `None` when that weight is not a plain clamped ramp.
	pub(super) fn source_over_ramp(&self, index: usize) -> Option<[f32; 2]> {
		match self {
			Self::Stepped => None,
			Self::BicubicBernstein => None,
			Self::LinearRows(knots) => Some([knots[index], knots[index + 1]]),
		}
	}

	/// Returns the u-direction color curve painted by the indexed layer.
	pub(super) fn evaluate_layer_u_color(&self, patch_evaluator: &MeshPatchEvaluator, index: usize, u: f32) -> Vec4 {
		match self {
			Self::Stepped => Vec4::from_array(patch_evaluator.evaluate_color(0., 0.)),
			Self::BicubicBernstein => patch_evaluator.evaluate_bicubic_bezier_row(index, u).expect("Bicubic Bernstein layers should have the control points"),
			Self::LinearRows(knots) => Vec4::from_array(patch_evaluator.evaluate_color(u, knots[index])),
		}
	}
}

/// Returns the largest per-channel error of linearly blending the exact surface rows at an interval's ends.
fn linear_row_interval_error(evaluator: &MeshGradientEvaluator, start: f32, end: f32) -> f32 {
	// The rows are reproduced exactly, so error is sampled across u and between the rows in v.
	const U_SAMPLES: usize = 64;
	const V_SAMPLES: usize = 8;

	let mut worst_error = 0_f32;
	for patch in evaluator.patch_evaluators() {
		for u_step in 0..=U_SAMPLES {
			let u = u_step as f32 / U_SAMPLES as f32;
			let start_color = Vec4::from_array(patch.evaluate_color(u, start));
			let end_color = Vec4::from_array(patch.evaluate_color(u, end));
			for v_step in 1..V_SAMPLES {
				let sample = v_step as f32 / V_SAMPLES as f32;
				let expected = Vec4::from_array(patch.evaluate_color(u, start + (end - start) * sample));
				let approximated = start_color + (end_color - start_color) * sample;
				worst_error = worst_error.max((expected - approximated).abs().max_element());
			}
		}
	}

	worst_error
}

// =====================
// SVG displacement maps
// =====================

pub(super) struct DisplacementMapSamples {
	/// Displacement-map region in local patch coordinates, including its margin. [x, y, width, height]
	pub region: [f64; 4],
	/// Row-major target-to-source displacement samples over `region`.
	pub displacements: Vec<DVec2>,
}

/// Returns the displacement-map region covering `patch_extent` plus the margin reserved on each side, as `(min, size)`.
fn displacement_map_region(patch_extent: DVec2) -> (DVec2, DVec2) {
	let margin = DISPLACEMENT_MAP_MARGIN_PERCENTAGE / (1. - 2. * DISPLACEMENT_MAP_MARGIN_PERCENTAGE);
	(-margin * patch_extent, (1. + 2. * margin) * patch_extent)
}

/// Returns target-to-source displacement samples mapping local patch-bounding-box positions to source UVs.
/// `patch_extent` is the patch bounding box measured in the local space, so both the sampled region and the source UVs
/// span it rather than a unit square.
pub(super) fn coons_bbox_to_source_displacements(patch_evaluator: &MeshPatchEvaluator, local_to_patch_bbox: &DAffine2, patch_extent: DVec2, boundary: &BezPath) -> DisplacementMapSamples {
	let size = DISPLACEMENT_MAP_SIZE;
	let (map_min, map_size) = displacement_map_region(patch_extent);
	let target_positions = |index: usize| {
		let x = index % size;
		let y = index / size;
		let image_uv = DVec2::new((x as f64 + 0.5) / size as f64, (y as f64 + 0.5) / size as f64);
		let local_position = map_min + image_uv * map_size;
		(local_position, local_to_patch_bbox.transform_point2(local_position))
	};

	// 81 samples of (uv, position) tuples in the patch
	let inverse_seeds = {
		// Number of initial intervals sampled along each patch axis
		const INITIAL_SUBDIVISIONS: usize = 8;
		let seed_count = (INITIAL_SUBDIVISIONS + 1).pow(2);
		let mut seeds = Vec::with_capacity(seed_count);
		for row in 0..=INITIAL_SUBDIVISIONS {
			let v = row as f64 / INITIAL_SUBDIVISIONS as f64;

			for column in 0..=INITIAL_SUBDIVISIONS {
				let u = column as f64 / INITIAL_SUBDIVISIONS as f64;
				let uv = DVec2::new(u, v);
				seeds.push((uv, patch_evaluator.evaluate_position(u, v)));
			}
		}
		seeds
	};
	let initial_uv_from_seeds = |target_position| {
		inverse_seeds
			.iter()
			.min_by(|(_, first_position), (_, second_position)| first_position.distance_squared(target_position).total_cmp(&second_position.distance_squared(target_position)))
			.map(|(uv, _)| *uv)
			.unwrap_or(DVec2::splat(0.5))
	};

	let inside_patch = (0..size * size)
		.map(|index| {
			let (_, target_position_in_mesh) = target_positions(index);
			boundary.contains(kurbo::Point::new(target_position_in_mesh.x, target_position_in_mesh.y))
		})
		.collect::<Vec<_>>();

	let buffer = DISPLACEMENT_MAP_OUTSIDE_BUFFER_TEXELS as isize;
	// The target region on the displacement map that requires source position
	let sampled_region = (0..size * size)
		.map(|index| {
			let x = (index % size) as isize;
			let y = (index / size) as isize;
			inside_patch[index]
				|| (-buffer..=buffer).any(|dy| {
					(-buffer..=buffer).any(|dx| {
						if dx.abs() + dy.abs() > buffer {
							return false;
						}

						let neighbor_x = x + dx;
						let neighbor_y = y + dy;
						neighbor_x >= 0 && neighbor_x < size as isize && neighbor_y >= 0 && neighbor_y < size as isize && inside_patch[neighbor_y as usize * size + neighbor_x as usize]
					})
				})
		})
		.collect::<Vec<_>>();

	let mut inverse_uvs = vec![None::<DVec2>; size * size];
	let mut attempted = vec![false; size * size];
	let mut reseed_attempted = vec![false; size * size];
	let mut inside_queue = VecDeque::new();
	let mut outside_queue = VecDeque::new();

	// Seed the first interior texel from the coarse inverse samples.
	if let Some(index) = inside_patch.iter().position(|&inside| inside) {
		let (_, target_position_in_mesh) = target_positions(index);
		attempted[index] = true;
		if let Some(uv) = patch_evaluator.try_inverse_patch_position(target_position_in_mesh, initial_uv_from_seeds(target_position_in_mesh)) {
			inverse_uvs[index] = Some(uv);
			inside_queue.push_back(index);
		}
	}

	let neighbors = |x: isize, y: isize| [(0, -1), (-1, 0), (1, 0), (0, 1), (-1, -1), (-1, 1), (1, -1), (1, 1)].into_iter().map(move |(dx, dy)| (x + dx, y + dy));
	let out_of_map_range = |x: isize, y: isize| x < 0 || x >= size as isize || y < 0 || y >= size as isize;

	// Resolve the patch interior first, deferring successfully inverted exterior texels until it is complete.
	loop {
		while let Some(index) = inside_queue.pop_front() {
			let initial_uv = inverse_uvs[index].expect("Only successfully inverted texels should be queued");
			let x = (index % size) as isize;
			let y = (index / size) as isize;

			for (neighbor_x, neighbor_y) in neighbors(x, y) {
				if out_of_map_range(neighbor_x, neighbor_y) {
					continue;
				}

				let neighbor_index = neighbor_y as usize * size + neighbor_x as usize;

				if attempted[neighbor_index] || !sampled_region[neighbor_index] {
					continue;
				}

				attempted[neighbor_index] = true;
				let (_, target_position_in_mesh) = target_positions(neighbor_index);
				if let Some(uv) = patch_evaluator.try_inverse_patch_position(target_position_in_mesh, initial_uv) {
					inverse_uvs[neighbor_index] = Some(uv);
					if inside_patch[neighbor_index] {
						inside_queue.push_back(neighbor_index);
					} else {
						outside_queue.push_back(neighbor_index);
					}
				}
			}
		}

		// Rasterizing the patch at the displacement-map resolution can split its interior into disconnected regions.
		// If any interior texels remain unresolved, restart the inverse search using the initial UV seeds.
		let next_seed = inverse_uvs
			.iter()
			.enumerate()
			.find(|(index, result)| inside_patch[*index] && result.is_none() && !reseed_attempted[*index])
			.map(|(index, _)| index);

		let Some(next_seed) = next_seed else { break };

		reseed_attempted[next_seed] = true;
		attempted[next_seed] = true;

		let (_, target_position_in_mesh) = target_positions(next_seed);
		let initial_uv = initial_uv_from_seeds(target_position_in_mesh);

		if let Some(uv) = patch_evaluator.try_inverse_patch_position(target_position_in_mesh, initial_uv) {
			inverse_uvs[next_seed] = Some(uv);
			inside_queue.push_back(next_seed);
		}
	}

	// Continue only through the exterior filtering region after every reachable interior texel is resolved.
	while let Some(index) = outside_queue.pop_front() {
		let initial_uv = inverse_uvs[index].expect("Only successfully inverted texels should be queued");
		let x = (index % size) as isize;
		let y = (index / size) as isize;

		for (neighbor_x, neighbor_y) in neighbors(x, y) {
			if out_of_map_range(neighbor_x, neighbor_y) {
				continue;
			}

			let neighbor_index = neighbor_y as usize * size + neighbor_x as usize;

			if attempted[neighbor_index] || !sampled_region[neighbor_index] || inside_patch[neighbor_index] {
				continue;
			}

			attempted[neighbor_index] = true;
			let (_, target_position_in_mesh) = target_positions(neighbor_index);
			if let Some(uv) = patch_evaluator.try_inverse_patch_position(target_position_in_mesh, initial_uv) {
				inverse_uvs[neighbor_index] = Some(uv);
				outside_queue.push_back(neighbor_index);
			}
		}
	}

	// As a fallback, fill unresolved buffer texels using the source UV of the nearest resolved interior texel
	let resolved_inside_samples = (0..size * size)
		.filter_map(|index| (inside_patch[index]).then(|| inverse_uvs[index].map(|uv| (index, uv))).flatten())
		.collect::<Vec<_>>();
	for index in 0..size * size {
		if !sampled_region[index] || inside_patch[index] || inverse_uvs[index].is_some() {
			continue;
		}

		let x = index % size;
		let y = index / size;

		let nearest_uv = resolved_inside_samples
			.iter()
			.min_by_key(|(candidate, _)| {
				let candidate_x = candidate % size;
				let candidate_y = candidate / size;
				let dx = x.abs_diff(candidate_x);
				let dy = y.abs_diff(candidate_y);
				dx * dx + dy * dy
			})
			.map(|(_, uv)| uv.clamp(DVec2::ZERO, DVec2::ONE));

		if let Some(uv) = nearest_uv {
			inverse_uvs[index] = Some(uv);
		}
	}

	let displacements = inverse_uvs
		.into_iter()
		.enumerate()
		.map(|(index, inverse_uv)| {
			let (target_position, _) = target_positions(index);
			// For positions outside the buffer, use zero displacement rather than estimating from a non-converged numerical source.
			// This prevents unexpected jumps in the displacement that would increase the quantization scale.
			let source_position = inverse_uv.map(|uv| uv.clamp(DVec2::ZERO, DVec2::ONE) * patch_extent).unwrap_or(target_position);

			source_position - target_position
		})
		.collect();
	DisplacementMapSamples {
		displacements,
		region: [map_min.x, map_min.y, map_size.x, map_size.y],
	}
}

/// Encodes target-to-source displacement samples as an RGBA8 PNG for feDisplacementMap.
pub(super) fn displacements_to_map_png(displacements: &[DVec2], scale: f64) -> Option<Vec<u8>> {
	let mut rgba8_bytes = Vec::with_capacity(DISPLACEMENT_MAP_SIZE * DISPLACEMENT_MAP_SIZE * 4);

	let encode_displacement = |displacement: DVec2| {
		let max_channel = u8::MAX as f64;
		let encoded = (DVec2::splat(0.5) + displacement / scale) * max_channel;
		(encoded.x.round().clamp(0., max_channel) as u8, encoded.y.round().clamp(0., max_channel) as u8)
	};

	for displacement in displacements {
		let (red, green) = encode_displacement(*displacement);
		rgba8_bytes.extend_from_slice(&[red, green, 0, u8::MAX]);
	}

	let mut displacement_map_png = Vec::new();
	::image::codecs::png::PngEncoder::new(&mut displacement_map_png)
		.write_image(&rgba8_bytes, DISPLACEMENT_MAP_SIZE as u32, DISPLACEMENT_MAP_SIZE as u32, ::image::ExtendedColorType::Rgba8)
		.ok()?;

	Some(displacement_map_png)
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
pub(super) fn alpha_curve_to_gradient_stops_string(func: &impl Fn(f32) -> f32) -> String {
	let error_func = |a: f32, b: f32| (a - b).abs();
	linear_approximation_points(func, &error_func, 0., 1., 0)
		.into_iter()
		.map(|(arg, result)| gradient_stop_element(arg, result, Color::WHITE.to_gamma_srgb_channels()))
		.collect::<String>()
}

/// Encodes a u-direction color curve as adaptively sampled SVG gradient stops.
pub(super) fn u_color_curve_to_gradient_stops_string(func: &impl Fn(f32) -> Vec4) -> String {
	let error_func = |a: Vec4, b: Vec4| (a - b).abs().max_element();
	linear_approximation_points(func, &error_func, 0., 1., 0)
		.into_iter()
		.map(|(argument, result)| gradient_stop_element(argument, 1., result.to_array()))
		.collect::<String>()
}

/// Encodes the two stops a clamped ramp needs, for a gradient placed across the range it ramps over.
pub(super) fn clamped_ramp_gradient_stops_string() -> String {
	let white = Color::WHITE.to_gamma_srgb_channels();
	format!("{}{}", gradient_stop_element(0., 1., white), gradient_stop_element(1., 0., white))
}

/// Encodes a scalar alpha curve as an opaque grayscale gradient for use by a luminance mask.
pub(super) fn u_alpha_curve_to_gradient_stops_string(func: &impl Fn(f32) -> f32) -> String {
	let error_func = |a: f32, b: f32| (a - b).abs();
	linear_approximation_points(func, &error_func, 0., 1., 0)
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

/// One region of a patch's uv square, kept alongside the error of approximating it with a single parallelogram.
struct PendingRegion {
	patch_index: usize,
	uv_start: DVec2,
	stride: f64,
	corner_positions: [DVec2; 4],
	/// Error as a multiple of the tolerances, so position and color rank on one scale. At most 1 is within tolerance.
	error: f64,
}

/// How far an error overruns its tolerance. A zero tolerance admits only a zero error.
fn tolerance_overrun(error: f64, tolerance: f64) -> f64 {
	if tolerance > 0. {
		error / tolerance
	} else if error > 0. {
		f64::INFINITY
	} else {
		0.
	}
}

/// Measures how far the rendered approximation of one region goes from the patch it covers.
/// `None` when the patch evaluates to a non-finite value there, which no amount of subdivision repairs.
fn measure_region(
	patch: &MeshPatchEvaluator,
	patch_index: usize,
	uv_start: DVec2,
	stride: f64,
	mesh_transform: DAffine2,
	parent_transform: DAffine2,
	position_error_tolerance: f64,
	color_error_tolerance: f32,
) -> Option<PendingRegion> {
	const SAMPLES: [f64; 5] = [0., 0.25, 0.5, 0.75, 1.];

	let corner_positions = [DVec2::ZERO, DVec2::new(stride, 0.), DVec2::new(0., stride), DVec2::splat(stride)]
		.map(|offset| uv_start + offset)
		.map(|uv| mesh_transform.transform_point2(patch.evaluate_position(uv.x, uv.y)));
	let [top_left_pos, top_right_pos, bottom_left_pos, _bottom_right_pos] = corner_positions;

	let color_weight_func = subpatch_color_weight(patch, uv_start.as_vec2(), (uv_start + DVec2::splat(stride)).as_vec2());

	let mut error = 0_f64;
	for &local_v in &SAMPLES {
		for &local_u in &SAMPLES {
			let u = uv_start.x + local_u * stride;
			let v = uv_start.y + local_v * stride;
			let expected_pos = mesh_transform.transform_point2(patch.evaluate_position(u, v));
			let expected_color = Vec4::from_array(patch.evaluate_color(u as f32, v as f32));
			// Approximate the position with the rendered parallelogram, then the color and alpha with the two
			// passes that actually paint them: the color pass blends the edge rows by the projected weight,
			// while the alpha pass ramps between them linearly.
			let approximated_pos = top_left_pos + (top_right_pos - top_left_pos) * local_u + (bottom_left_pos - top_left_pos) * local_v;
			let top_color = Vec4::from_array(patch.evaluate_color(u as f32, uv_start.y as f32));
			let bottom_color = Vec4::from_array(patch.evaluate_color(u as f32, (uv_start.y + stride) as f32));
			let approximated_color = bottom_color.lerp(top_color, color_weight_func(v as f32));
			let approximated_alpha = top_color.w + (bottom_color.w - top_color.w) * local_v as f32;

			let position_error = parent_transform.transform_vector2(expected_pos - approximated_pos).length();
			let color_error = (expected_color.truncate() - approximated_color.truncate())
				.abs()
				.max_element()
				.max((expected_color.w - approximated_alpha).abs());
			if !position_error.is_finite() || !color_error.is_finite() {
				return None;
			}

			error = error
				.max(tolerance_overrun(position_error, position_error_tolerance))
				.max(tolerance_overrun(color_error as f64, color_error_tolerance as f64));
		}
	}

	Some(PendingRegion {
		patch_index,
		uv_start,
		stride,
		corner_positions,
		error,
	})
}

/// Subdivides the patches until every region's parallelogram approximation is within the position and color tolerances, or the subpatch budget runs out.
pub(super) fn subdivide_patches_adaptive(
	evaluator: &MeshGradientEvaluator,
	mesh_transform: DAffine2,
	parent_transform: DAffine2,
	position_error_tolerance: f64,
	color_error_tolerance: f32,
) -> Option<Vec<MeshSubpatch>> {
	if !position_error_tolerance.is_finite() || position_error_tolerance < 0. || !color_error_tolerance.is_finite() || color_error_tolerance < 0. {
		return None;
	}

	let patches = evaluator.patch_evaluators().collect::<Vec<_>>();
	let measure = |patch_index: usize, uv_start, stride| {
		measure_region(
			patches[patch_index],
			patch_index,
			uv_start,
			stride,
			mesh_transform,
			parent_transform,
			position_error_tolerance,
			color_error_tolerance,
		)
	};

	let mut regions = (0..patches.len()).map(|patch_index| measure(patch_index, DVec2::ZERO, 1.)).collect::<Option<Vec<_>>>()?;

	// Every patch owes at least its own root region, so the cap bounds the refinement on top of that rather than the total
	let budget = MESH_MAXIMUM_SUBPATCHES.max(regions.len());
	while regions.len() + 3 <= budget {
		let worst = regions
			.iter()
			.enumerate()
			.filter(|(_, region)| region.error > 1. && region.stride > MINIMUM_SUBPATCH_STRIDE)
			.max_by(|(_, first), (_, second)| first.error.total_cmp(&second.error))
			.map(|(index, _)| index);
		let Some(worst) = worst else { break };

		let region = regions.swap_remove(worst);
		let half_stride = region.stride / 2.;
		for offset in [DVec2::ZERO, DVec2::new(half_stride, 0.), DVec2::new(0., half_stride), DVec2::splat(half_stride)] {
			regions.push(measure(region.patch_index, region.uv_start + offset, half_stride)?);
		}
	}

	Some(
		regions
			.into_iter()
			.map(|region| MeshSubpatch {
				corner_positions: region.corner_positions,
				patch_index: region.patch_index,
				uv_bounds: [region.uv_start, region.uv_start + DVec2::splat(region.stride)],
			})
			.collect(),
	)
}

/// Returns the affine approximation of a subpatch, rejecting folded or degenerate geometry.
pub(super) fn mesh_subpatch_transform(subpatch: &MeshSubpatch) -> Option<DAffine2> {
	let [top_left, top_right, bottom_left, _] = subpatch.corner_positions;
	let transform = DAffine2::from_cols(top_right - top_left, bottom_left - top_left, top_left);
	let determinant = transform.matrix2.determinant();
	(determinant.is_finite() && determinant != 0.).then_some(transform)
}

/// Returns the local clip and paint inflation needed to hide gaps around a transformed subpatch.
fn mesh_subpatch_inflation(subpatch_to_scene: DAffine2) -> (f64, f64) {
	let (_, smallest_scale) = singular_values(subpatch_to_scene);
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
	let stops = linear_approximation_points(func, &error, start, end, 0).into_iter().map(|(v, alpha)| {
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

/// Returns the weight the color pass blends a region's two edge rows with, as a function of v.
///
/// It projects the color curve at the region's horizontal midpoint onto the line between its edge colors, so however
/// unevenly a color space paces its path along v, the blend follows that pacing and only has to cover the deviation
/// off that line. The subdivision's error model reads the same weight as the brush that paints the region, so the
/// refinement never pays for a coarser approximation than it actually draws.
fn subpatch_color_weight(patch_evaluator: &MeshPatchEvaluator, uv_min: Vec2, uv_max: Vec2) -> impl Fn(f32) -> f32 + use<'_> {
	let center_u = (uv_min.x + uv_max.x) / 2.;
	let top_center_color = Vec4::from_array(patch_evaluator.evaluate_color(center_u, uv_min.y)).truncate();
	let bottom_center_color = Vec4::from_array(patch_evaluator.evaluate_color(center_u, uv_max.y)).truncate();
	let color_axis = top_center_color - bottom_center_color;
	let color_axis_length_squared = color_axis.length_squared();

	move |v| {
		if color_axis_length_squared > f32::EPSILON {
			let color = Vec4::from_array(patch_evaluator.evaluate_color(center_u, v)).truncate();
			((color - bottom_center_color).dot(color_axis) / color_axis_length_squared).clamp(0., 1.)
		} else {
			(uv_max.y - v) / (uv_max.y - uv_min.y)
		}
	}
}

/// Builds the opaque RGB approximation for one subpatch.
fn vello_subpatch_color_brushes(patch_evaluator: &MeshPatchEvaluator, subpatch: &MeshSubpatch) -> VelloSubpatchBrushes {
	let [uv_min, uv_max] = subpatch.uv_bounds.map(|uv| uv.as_vec2());
	let remap_offset = |value: f32, start: f32, end: f32| (value - start) / (end - start);

	// Preserve each cubic horizontal RGB edge with adaptive gradient stops. Alpha is applied after the RGB field is complete.
	let [top_color, bottom_color] = [uv_min.y, uv_max.y].map(|v| {
		let curve = |u| Vec4::from_array(patch_evaluator.evaluate_color(u, v));
		let error = |a: Vec4, b: Vec4| (a - b).abs().max_element();
		let stops = linear_approximation_points(&curve, &error, uv_min.x, uv_max.x, 0).into_iter().map(|(u, mut color)| {
			color.w = 1.;
			(remap_offset(u, uv_min.x, uv_max.x), gamma_color_to_srgba8(color.to_array()))
		});
		vello_linear_gradient(DVec2::ZERO, DVec2::X, stops)
	});

	let color_weight_func = subpatch_color_weight(patch_evaluator, uv_min, uv_max);
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

	// This matches the color approximation used to decide adaptive subdivision: preserve the
	// horizontal edge curves, then interpolate them linearly in the local v direction.
	let [top_color, bottom_color] = [uv_min.y, uv_max.y].map(|v| {
		let curve = |u| patch_evaluator.evaluate_color(u, v)[3];
		let error = |a: f32, b: f32| (a - b).abs();
		let stops = linear_approximation_points(&curve, &error, uv_min.x, uv_max.x, 0)
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
	let (clip_inflation, paint_inflation) = mesh_subpatch_inflation(subpatch_to_device);
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

// ============
// SVG renderer
// ============

pub(super) struct SvgMeshPatchRenderer<'mesh, 'field> {
	mesh_evaluator: &'mesh MeshGradientEvaluator,
	v_layers: SvgMeshVLayers,
	alpha_mask_gradient_ids: Vec<String>,
	parent_transform: DAffine2,
	mesh_transform: DAffine2,
	mesh_transparency_field: Option<&'field mut String>,
}

impl<'mesh, 'field> SvgMeshPatchRenderer<'mesh, 'field> {
	pub(super) fn new(
		render: &mut SvgRender,
		mesh_evaluator: &'mesh MeshGradientEvaluator,
		parent_transform: DAffine2,
		mesh_transform: DAffine2,
		mesh_transparency_field: Option<&'field mut String>,
	) -> Self {
		// The layer stack is what carries the color space: gamma sRGB uses the bicubic Bernstein stack,
		// while a nonlinear space stacks approximated rows so the compositor's linear blend still lands on the true surface.
		let v_layers = SvgMeshVLayers::new(mesh_evaluator);

		// The v-direction mask to simulate 2D interpolation
		let alpha_mask_gradient_ids = Self::render_alpha_mask_gradient(render, &v_layers);

		Self {
			mesh_evaluator,
			v_layers,
			alpha_mask_gradient_ids,
			parent_transform,
			mesh_transform,
			mesh_transparency_field,
		}
	}

	/// Define N-1 alpha functions from the v-direction layer weights and write them as approximated linear gradients, then return the ids.
	/// They compensate for attenuation accumulated through source-over compositing,
	/// making the final weights of the N color layers equal the layer scheme's weights.
	/// The v-direction masks encode only those weights with no patch specific color data, so they can be shared by all patches.
	fn render_alpha_mask_gradient(render: &mut SvgRender, v_layers: &SvgMeshVLayers) -> Vec<String> {
		let alpha_mask_gradient_group_id = generate_uuid();
		(0..v_layers.layer_count() - 1)
			.map(|i| {
				let id = format!("mg-ag{i}-{alpha_mask_gradient_group_id}");
				match v_layers.source_over_ramp(i) {
					// Linear interpolation mask to blend i-th and (i+1)-th u direction gradients
					Some([start, end]) => write!(
						&mut render.svg_defs,
						r##"<linearGradient id="{id}" x1="0.5" y1="{start}" x2="0.5" y2="{end}" gradientUnits="userSpaceOnUse">{}</linearGradient>"##,
						clamped_ramp_gradient_stops_string(),
					),
					// 4 Bernstein base functions for the v direction
					None => write!(
						&mut render.svg_defs,
						r##"<linearGradient id="{id}" x1="0.5" y1="0" x2="0.5" y2="1" gradientUnits="userSpaceOnUse">{}</linearGradient>"##,
						alpha_curve_to_gradient_stops_string(&|t| v_layers.source_over_alpha(i, t)),
					),
				}
				.unwrap();

				id
			})
			.collect::<Vec<_>>()
	}

	fn render_alpha_mask(&self, render: &mut SvgRender, patch_unique_id: u64, map_region: [f64; 4]) -> Vec<String> {
		let [map_x, map_y, map_width, map_height] = map_region;
		self.alpha_mask_gradient_ids
			.iter()
			.enumerate()
			.map(|(i, gradient_id)| {
				let mask_id = format!("mg-am{i}-{patch_unique_id}");
				write!(
					&mut render.svg_defs,
					r##"<mask
					id="{mask_id}"
					x="{map_x}"
					y="{map_y}"
					width="{map_width}"
					height="{map_height}"
					maskUnits="userSpaceOnUse"
					maskContentUnits="userSpaceOnUse"
					mask-type="alpha">
						<rect
						x="{map_x}"
						y="{map_y}"
						width="{map_width}"
						height="{map_height}"
						fill="url(#{gradient_id})"/>
					</mask>"##,
				)
				.unwrap();
				mask_id
			})
			.collect::<Vec<_>>()
	}

	pub(super) fn render_patch(&mut self, render: &mut SvgRender, patch: &MeshPatch) {
		let unique_id = generate_uuid();
		let Some(patch_evaluator) = self.mesh_evaluator.patch_evaluator(patch.index) else { return };

		// Construct a closed path of the patch boundary for calculating the bounding box and create a clipping mask
		let mut patch_boundary_path = patch.boundary_path();
		let bounds = patch_boundary_path.bounding_box();
		let bounds_min = DVec2::new(bounds.x0, bounds.y0);
		let bounds_max = DVec2::new(bounds.x1, bounds.y1);
		let bounds_size = bounds_max - bounds_min;
		if !bounds_size.is_finite() || bounds_size.x <= f64::EPSILON || bounds_size.y <= f64::EPSILON {
			return;
		}
		// Encode the deformation in a local patch-bounding-box space so patch translation and scaling do not consume PNG channel precision.
		// That space has to reach the output through a uniform scale, since Firefox as of version 154 has a bug
		// that converts `feDisplacementMap`'s `scale` into one isotropic filter-space length instead of one length per axis,
		// so a local space that reaches the output non-uniformly displaces both axes by the wrong amount there.
		let mesh_to_output = (self.parent_transform * self.mesh_transform).matrix2;
		let output_scales = DVec2::new(mesh_to_output.x_axis.length(), mesh_to_output.y_axis.length());
		if !output_scales.is_finite() || output_scales.min_element() <= f64::EPSILON {
			return;
		}
		let local_axes = DVec2::new(bounds_size.y * output_scales.y / output_scales.x, bounds_size.y);
		let patch_extent = bounds_size / local_axes;
		let local_to_patch_bbox = DAffine2::from_cols(DVec2::new(local_axes.x, 0.), DVec2::new(0., local_axes.y), bounds_min);
		let local_to_output = self.parent_transform * self.mesh_transform * local_to_patch_bbox;
		let (_, smallest_output_scale) = singular_values(local_to_output);
		if !smallest_output_scale.is_finite() || smallest_output_scale <= f64::EPSILON {
			return;
		}

		let DisplacementMapSamples { displacements, region } = coons_bbox_to_source_displacements(patch_evaluator, &local_to_patch_bbox, patch_extent, &patch_boundary_path);
		let [map_x, map_y, map_width, map_height] = region;
		// feDisplacementMap decodes each channel as scale * (channel - 0.5).
		// Twice the largest absolute component is therefore the smallest scale that covers every displacement and maximizes quantization precision.
		let max_displacement = displacements.iter().map(|displacement| displacement.abs().max_element()).fold(0_f64, f64::max);
		// Keep the scale nonzero when all displacements are zero.
		let scale = (max_displacement * 2.).max(f64::EPSILON);

		let Some(displacement_map_png) = displacements_to_map_png(&displacements, scale) else { return };
		let preamble = "data:image/png;base64,";
		let mut displacement_map_data_url = String::with_capacity(preamble.len() + displacement_map_png.len() * 4 / 3 + 4);
		displacement_map_data_url.push_str(preamble);
		base64::engine::general_purpose::STANDARD.encode_string(displacement_map_png, &mut displacement_map_data_url);

		let v_alpha_mask_ids = self.render_alpha_mask(render, unique_id, region);

		let extent_x = patch_extent.x;
		let u_color_curves_gradient_ids = (0..self.v_layers.layer_count())
			.map(|i| {
				let u_color_curve = |u| self.v_layers.evaluate_layer_u_color(patch_evaluator, i, u);
				let stops = u_color_curve_to_gradient_stops_string(&u_color_curve);
				let id = format!("mg-cg{i}-{unique_id}");

				write!(
					&mut render.svg_defs,
					r##"<linearGradient id="{id}" x1="0" y1="0.5" x2="{extent_x}" y2="0.5" gradientUnits="userSpaceOnUse">{stops}</linearGradient>"##,
				)
				.unwrap();

				id
			})
			.collect::<Vec<_>>();

		write!(
			&mut render.svg_defs,
			r##"<filter
			id="fd{unique_id}"
			x="{map_x}"
			y="{map_y}"
			width="{map_width}"
			height="{map_height}"
			filterUnits="userSpaceOnUse"
			primitiveUnits="userSpaceOnUse"
			color-interpolation-filters="sRGB">
				<feImage
				href="{displacement_map_data_url}"
				x="{map_x}"
				y="{map_y}"
				width="{map_width}"
				height="{map_height}"
				preserveAspectRatio="none"
				result="gmmap{unique_id}"/>
				<feDisplacementMap
					x="{map_x}"
					y="{map_y}"
					width="{map_width}"
					height="{map_height}"
					in="SourceGraphic"
					in2="gmmap{unique_id}"
				scale="{scale}"
				xChannelSelector="R"
				yChannelSelector="G"/>
		</filter>"##
		)
		.unwrap();

		// Add a centered stroke to expand the patch along its boundary normal and hide antialiasing gaps between patches.
		let patch_clip_stroke_width = 2. * PATCH_INFLATION_SIZE / smallest_output_scale;
		patch_boundary_path.apply_affine(Affine::new(local_to_patch_bbox.inverse().to_cols_array()));
		let patch_boundary_d = patch_boundary_path.to_svg();

		write!(
			&mut render.svg_defs,
			r##"<mask
			id="mc{unique_id}"
			x="{map_x}"
			y="{map_y}"
			width="{map_width}"
			height="{map_height}"
			maskUnits="userSpaceOnUse"
			maskContentUnits="userSpaceOnUse"
			mask-type="alpha">
				<path d="{patch_boundary_d}" fill="#fff" stroke="#fff" stroke-width="{patch_clip_stroke_width}" stroke-linejoin="round"/>
			</mask>"##
		)
		.unwrap();

		let patch_transform_str = format_transform_matrix(self.mesh_transform * local_to_patch_bbox);
		render.parent_tag(
			"g",
			|attributes| {
				attributes.push("transform", patch_transform_str.clone());
			},
			|render| {
				render.parent_tag(
					"g",
					|attributes| {
						attributes.push("mask", format!("url(#mc{unique_id})"));
					},
					|render| {
						render.parent_tag(
							"g",
							|attributes| {
								attributes.push("style", "isolation:isolate");
								attributes.push("filter", format!("url(#fd{unique_id})"));
							},
							|render| {
								u_color_curves_gradient_ids.iter().enumerate().rev().for_each(|(i, gradient_id)| {
									render.leaf_tag("rect", |attributes| {
										attributes.push("x", map_x.to_string());
										attributes.push("y", map_y.to_string());
										attributes.push("width", map_width.to_string());
										attributes.push("height", map_height.to_string());
										attributes.push("fill", format!("url(#{gradient_id})"));
										if let Some(mask_id) = v_alpha_mask_ids.get(i) {
											attributes.push("mask", format!("url(#{mask_id})"));
										}
									});
								});
							},
						);
					},
				);
			},
		);

		self.collect_transparency_field(render, patch, unique_id, patch_transform_str, patch_extent, &v_alpha_mask_ids);
	}

	fn collect_transparency_field(&mut self, render: &mut SvgRender, patch: &MeshPatch, patch_unique_id: u64, patch_transform: String, patch_extent: DVec2, v_alpha_mask_ids: &[String]) -> Option<()> {
		let mesh_transparency_field = self.mesh_transparency_field.as_deref_mut()?;
		let patch_evaluator = self.mesh_evaluator.patch_evaluator(patch.index)?;
		let (map_min, map_size) = displacement_map_region(patch_extent);
		let (map_x, map_y, map_width, map_height) = (map_min.x, map_min.y, map_size.x, map_size.y);
		let extent_x = patch_extent.x;

		// Keep transparency as an opaque grayscale field until every patch has been assembled into one mesh-wide luminance mask.
		let u_transparency_curves_gradient_ids: Vec<String> = (0..self.v_layers.layer_count())
			.map(|i| {
				// Only takes alpha value
				let u_alpha_curve = |t| self.v_layers.evaluate_layer_u_color(patch_evaluator, i, t).w;
				let stops = u_alpha_curve_to_gradient_stops_string(&u_alpha_curve);
				let id = format!("mg-cag{i}-{patch_unique_id}");

				write!(
					&mut render.svg_defs,
					r##"<linearGradient id="{id}" x1="0" y1="0.5" x2="{extent_x}" y2="0.5" gradientUnits="userSpaceOnUse">{stops}</linearGradient>"##,
				)
				.unwrap();

				id
			})
			.collect();

		let mut patch_transparency_field = String::new();
		for (i, gradient_id) in u_transparency_curves_gradient_ids.iter().enumerate().rev() {
			let mask = match v_alpha_mask_ids.get(i) {
				Some(mask_id) => format!(r##" mask="url(#{mask_id})""##),
				None => String::new(),
			};
			write!(
				patch_transparency_field,
				r##"<rect x="{map_x}" y="{map_y}" width="{map_width}" height="{map_height}" fill="url(#{gradient_id})"{mask}/>"##,
			)
			.unwrap();
		}

		write!(
			mesh_transparency_field,
			r##"<g transform="{patch_transform}" mask="url(#mc{patch_unique_id})"><g style="isolation:isolate" filter="url(#fd{patch_unique_id})">{patch_transparency_field}</g></g>"##,
		)
		.unwrap();

		Some(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use vector_types::gradient::MeshGradient;

	/// Builds a mesh whose corners cycle through the given colors.
	fn mesh_with_corner_colors(colors: [Color; 4]) -> MeshGradient {
		let mut mesh = MeshGradient::default();
		for corner_index in 0..mesh.size() {
			mesh.set_corner_color(corner_index, colors[corner_index % colors.len()]).unwrap();
		}
		mesh
	}

	#[test]
	fn stacked_oklab_rows_reproduce_the_color_surface() {
		let mesh = mesh_with_corner_colors([Color::BLACK, Color::WHITE, Color::BLUE, Color::YELLOW]);
		let evaluator = mesh.evaluator(GradientSpace::OkLab, GradientInterpolation::Smooth).unwrap();
		let layers = SvgMeshVLayers::new(&evaluator);

		let mut worst_error = 0_f32;
		for patch in evaluator.patch_evaluators() {
			for u_step in 0..=256 {
				let u = u_step as f32 / 256.;
				for v_step in 0..=256 {
					let v = v_step as f32 / 256.;

					let mut composited = layers.evaluate_layer_u_color(patch, layers.layer_count() - 1, u);
					for index in (0..layers.layer_count() - 1).rev() {
						let alpha = layers.source_over_alpha(index, v);
						composited = composited.lerp(layers.evaluate_layer_u_color(patch, index, u), alpha);
					}

					let expected = Vec4::from_array(patch.evaluate_color(u, v));
					worst_error = worst_error.max((expected - composited).abs().max_element());
				}
			}
		}

		assert!(worst_error <= SVG_LAYER_ERROR_TOLERANCE, "the stack deviated by {} of 1/255", worst_error * 255.);
	}

	#[test]
	fn oklab_row_weights_stay_a_partition_of_unity() {
		let mesh = mesh_with_corner_colors([Color::BLACK, Color::WHITE, Color::BLUE, Color::YELLOW]);
		let evaluator = mesh.evaluator(GradientSpace::OkLab, GradientInterpolation::Smooth).unwrap();
		let layers = SvgMeshVLayers::new(&evaluator);

		for v_step in 0..=64 {
			let v = v_step as f32 / 64.;
			let mut remaining = 1_f32;
			let mut total = 0_f32;
			for index in 0..layers.layer_count() - 1 {
				let alpha = layers.source_over_alpha(index, v);
				assert!((0. ..=1.).contains(&alpha), "a source-over alpha must stay in range, got {alpha} at v={v}");
				total += remaining * alpha;
				remaining -= remaining * alpha;
			}
			total += remaining;

			assert!((total - 1.).abs() < 1e-5, "the weights must sum to one, got {total} at v={v}");
		}
	}

	#[test]
	fn adaptive_subdivision_accounts_for_color_error() {
		let mesh = MeshGradient::default();
		let evaluator = mesh.evaluator(GradientSpace::RgbGamma, GradientInterpolation::Smooth).unwrap();
		let geometry_only = subdivide_patches_adaptive(&evaluator, DAffine2::IDENTITY, DAffine2::IDENTITY, f64::MAX, f32::MAX).unwrap();
		let with_color = subdivide_patches_adaptive(&evaluator, DAffine2::IDENTITY, DAffine2::IDENTITY, f64::MAX, 0.).unwrap();

		assert!(with_color.len() > geometry_only.len());
	}

	#[test]
	fn adaptive_subdivision_rejects_non_finite_transform() {
		let mesh = MeshGradient::default();
		let evaluator = mesh.evaluator(GradientSpace::RgbGamma, GradientInterpolation::Smooth).unwrap();
		let non_finite_transform = DAffine2::from_scale(DVec2::splat(f64::NAN));

		assert!(subdivide_patches_adaptive(&evaluator, DAffine2::IDENTITY, non_finite_transform, 0.25, 0.01).is_none());
	}
}

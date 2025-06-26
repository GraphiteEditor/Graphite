use super::algorithms::bezpath_algorithms::{self, position_on_bezpath, sample_polyline_on_bezpath, split_bezpath, tangent_on_bezpath};
use super::algorithms::offset_subpath::offset_subpath;
use super::algorithms::spline::{solve_spline_first_handle_closed, solve_spline_first_handle_open};
use super::misc::{CentroidType, MergeByDistanceAlgorithm, PointSpacingType, dvec2_to_point, point_to_dvec2};
use crate::modification::VectorDataExt;
use bezier_rs::{Join, ManipulatorGroup, Subpath};
use glam::{DAffine2, DVec2};
use graphene_core::color::Color;
use graphene_core::context::{CloneVarArgs, Context, Ctx, ExtractAll, OwnedContextImpl};
use graphene_core::gradient::{Gradient, GradientStops};
use graphene_core::instances::{Instance, InstanceMut, Instances};
use graphene_core::registry::types::{Angle, Fraction, IntegerCount, Length, Multiplier, Percentage, PixelLength, PixelSize, SeedValue};
use graphene_core::transform::{Footprint, Transform};
use graphene_vector::style::{Fill, PaintOrder, Stroke, StrokeAlign, StrokeCap, StrokeJoin};
use graphene_vector::{FillId, PointDomain, PointId, RegionId, SegmentDomain, SegmentId, StrokeId, VectorData, VectorDataTable};
use kurbo::{Affine, BezPath, DEFAULT_ACCURACY, ParamCurve, PathEl, PathSeg, Point, Shape};
use log::warn;
use rand::{Rng, SeedableRng};
use std::collections::hash_map::DefaultHasher;
use std::f64::consts::PI;
use std::f64::consts::TAU;
use std::hash::{Hash, Hasher};

#[node_macro::node(category("Vector: Modifier"), path(graphene_core::vector))]
async fn round_corners(
	_: impl Ctx,
	source: VectorDataTable,
	#[hard_min(0.)]
	#[default(10.)]
	radius: PixelLength,
	#[range((0., 1.))]
	#[hard_min(0.)]
	#[hard_max(1.)]
	#[default(0.5)]
	roundness: f64,
	#[default(100.)] edge_length_limit: Percentage,
	#[range((0., 180.))]
	#[hard_min(0.)]
	#[hard_max(180.)]
	#[default(5.)]
	min_angle_threshold: Angle,
) -> VectorDataTable {
	let mut result_table = VectorDataTable::default();

	for source in source.instance_ref_iter() {
		let source_transform = *source.transform;
		let source_transform_inverse = source_transform.inverse();
		let source = source.instance;

		let upstream_graphic_group = source.upstream_graphic_group.clone();

		// Flip the roundness to help with user intuition
		let roundness = 1. - roundness;
		// Convert 0-100 to 0-0.5
		let edge_length_limit = edge_length_limit * 0.005;

		let mut result = VectorData {
			style: source.style.clone(),
			..Default::default()
		};

		// Grab the initial point ID as a stable starting point
		let mut initial_point_id = source.point_domain.ids().first().copied().unwrap_or(PointId::generate());

		for mut subpath in source.stroke_bezier_paths() {
			subpath.apply_transform(source_transform);

			// End if not enough points for corner rounding
			if subpath.manipulator_groups().len() < 3 {
				result.append_subpath(subpath, false);
				continue;
			}

			let groups = subpath.manipulator_groups();
			let mut new_groups = Vec::new();
			let is_closed = subpath.closed();

			for i in 0..groups.len() {
				// Skip first and last points for open paths
				if !is_closed && (i == 0 || i == groups.len() - 1) {
					new_groups.push(groups[i]);
					continue;
				}

				// Not the prettiest, but it makes the rest of the logic more readable
				let prev_idx = if i == 0 { if is_closed { groups.len() - 1 } else { 0 } } else { i - 1 };
				let curr_idx = i;
				let next_idx = if i == groups.len() - 1 { if is_closed { 0 } else { i } } else { i + 1 };

				let prev = groups[prev_idx].anchor;
				let curr = groups[curr_idx].anchor;
				let next = groups[next_idx].anchor;

				let dir1 = (curr - prev).normalize_or(DVec2::X);
				let dir2 = (next - curr).normalize_or(DVec2::X);

				let theta = PI - dir1.angle_to(dir2).abs();

				// Skip near-straight corners
				if theta > PI - min_angle_threshold.to_radians() {
					new_groups.push(groups[curr_idx]);
					continue;
				}

				// Calculate L, with limits to avoid extreme values
				let distance_along_edge = radius / (theta / 2.).sin();
				let distance_along_edge = distance_along_edge.min(edge_length_limit * (curr - prev).length().min((next - curr).length())).max(0.01);

				// Find points on each edge at distance L from corner
				let p1 = curr - dir1 * distance_along_edge;
				let p2 = curr + dir2 * distance_along_edge;

				// Add first point (coming into the rounded corner)
				new_groups.push(ManipulatorGroup {
					anchor: p1,
					in_handle: None,
					out_handle: Some(curr - dir1 * distance_along_edge * roundness),
					id: initial_point_id.next_id(),
				});

				// Add second point (coming out of the rounded corner)
				new_groups.push(ManipulatorGroup {
					anchor: p2,
					in_handle: Some(curr + dir2 * distance_along_edge * roundness),
					out_handle: None,
					id: initial_point_id.next_id(),
				});
			}

			// One subpath for each shape
			let mut rounded_subpath = Subpath::new(new_groups, is_closed);
			rounded_subpath.apply_transform(source_transform_inverse);
			result.append_subpath(rounded_subpath, false);
		}

		result.upstream_graphic_group = upstream_graphic_group;

		result_table.push(Instance {
			instance: result,
			transform: source_transform,
			alpha_blending: Default::default(),
			source_node_id: None,
		});
	}

	result_table
}

#[node_macro::node(name("Merge by Distance"), category("Vector: Modifier"), path(graphene_core::vector))]
pub fn merge_by_distance(
	_: impl Ctx,
	vector_data: VectorDataTable,
	#[default(0.1)]
	#[hard_min(0.0001)]
	distance: PixelLength,
	algorithm: MergeByDistanceAlgorithm,
) -> VectorDataTable {
	let mut result_table = VectorDataTable::default();

	match algorithm {
		MergeByDistanceAlgorithm::Spatial => {
			for mut vector_data_instance in vector_data.instance_iter() {
				vector_data_instance.instance.merge_by_distance_spatial(vector_data_instance.transform, distance);
				result_table.push(vector_data_instance);
			}
		}
		MergeByDistanceAlgorithm::Topological => {
			for mut vector_data_instance in vector_data.instance_iter() {
				vector_data_instance.instance.merge_by_distance_topological(distance);
				result_table.push(vector_data_instance);
			}
		}
	}

	result_table
}

#[node_macro::node(category("Vector: Modifier"), path(graphene_core::vector))]
async fn box_warp(_: impl Ctx, vector_data: VectorDataTable, #[expose] rectangle: VectorDataTable) -> VectorDataTable {
	let Some((target, target_transform)) = rectangle.get(0).map(|rect| (rect.instance, rect.transform)) else {
		return vector_data;
	};

	let mut result_table = VectorDataTable::default();

	for mut vector_data_instance in vector_data.instance_iter() {
		let vector_data_transform = vector_data_instance.transform;
		let vector_data = vector_data_instance.instance;

		// Get the bounding box of the source vector data
		let source_bbox = vector_data.bounding_box_with_transform(vector_data_transform).unwrap_or([DVec2::ZERO, DVec2::ONE]);

		// Extract first 4 points from target shape to form the quadrilateral
		// Apply the target's transform to get points in world space
		let target_points: Vec<DVec2> = target.point_domain.positions().iter().map(|&p| target_transform.transform_point2(p)).take(4).collect();

		// If we have fewer than 4 points, use the corners of the source bounding box
		// This handles the degenerative case
		let dst_corners = if target_points.len() >= 4 {
			[target_points[0], target_points[1], target_points[2], target_points[3]]
		} else {
			warn!("Target shape has fewer than 4 points. Using source bounding box instead.");
			[
				source_bbox[0],
				DVec2::new(source_bbox[1].x, source_bbox[0].y),
				source_bbox[1],
				DVec2::new(source_bbox[0].x, source_bbox[1].y),
			]
		};

		// Apply the warp
		let mut result = vector_data.clone();

		// Precompute source bounding box size for normalization
		let source_size = source_bbox[1] - source_bbox[0];

		// Transform points
		for (_, position) in result.point_domain.positions_mut() {
			// Get the point in world space
			let world_pos = vector_data_transform.transform_point2(*position);

			// Normalize coordinates within the source bounding box
			let t = ((world_pos - source_bbox[0]) / source_size).clamp(DVec2::ZERO, DVec2::ONE);

			// Apply bilinear interpolation
			*position = bilinear_interpolate(t, &dst_corners);
		}

		// Transform handles in bezier curves
		for (_, handles, _, _) in result.handles_mut() {
			*handles = handles.apply_transformation(|pos| {
				// Get the handle in world space
				let world_pos = vector_data_transform.transform_point2(pos);

				// Normalize coordinates within the source bounding box
				let t = ((world_pos - source_bbox[0]) / source_size).clamp(DVec2::ZERO, DVec2::ONE);

				// Apply bilinear interpolation
				bilinear_interpolate(t, &dst_corners)
			});
		}

		result.style.set_stroke_transform(DAffine2::IDENTITY);

		// Add this to the table and reset the transform since we've applied it directly to the points
		vector_data_instance.instance = result;
		vector_data_instance.transform = DAffine2::IDENTITY;
		vector_data_instance.source_node_id = None;
		result_table.push(vector_data_instance);
	}

	result_table
}

// Interpolate within a quadrilateral using normalized coordinates (0-1)
fn bilinear_interpolate(t: DVec2, quad: &[DVec2; 4]) -> DVec2 {
	let tl = quad[0]; // Top-left
	let tr = quad[1]; // Top-right
	let br = quad[2]; // Bottom-right
	let bl = quad[3]; // Bottom-left

	// Bilinear interpolation
	tl * (1. - t.x) * (1. - t.y) + tr * t.x * (1. - t.y) + br * t.x * t.y + bl * (1. - t.x) * t.y
}

/// Automatically constructs tangents (Bézier handles) for anchor points in a vector path.
#[node_macro::node(category("Vector: Modifier"), name("Auto-Tangents"), path(graphene_core::vector))]
async fn auto_tangents(
	_: impl Ctx,
	source: VectorDataTable,
	/// The amount of spread for the auto-tangents, from 0 (sharp corner) to 1 (full spread).
	#[default(0.5)]
	#[range((0., 1.))]
	spread: f64,
	/// If active, existing non-zero handles won't be affected.
	#[default(true)]
	preserve_existing: bool,
) -> VectorDataTable {
	let mut result_table = VectorDataTable::default();

	for source in source.instance_ref_iter() {
		let transform = *source.transform;
		let alpha_blending = *source.alpha_blending;
		let source_node_id = *source.source_node_id;
		let source = source.instance;

		let mut result = VectorData {
			style: source.style.clone(),
			..Default::default()
		};

		for mut subpath in source.stroke_bezier_paths() {
			subpath.apply_transform(transform);

			let groups = subpath.manipulator_groups();
			if groups.len() < 2 {
				// Not enough points for softening or handle removal
				result.append_subpath(subpath, true);
				continue;
			}

			let mut new_groups = Vec::with_capacity(groups.len());
			let is_closed = subpath.closed();

			for i in 0..groups.len() {
				let curr = &groups[i];

				if preserve_existing {
					// Check if this point has handles that are meaningfully different from the anchor
					let has_handles = (curr.in_handle.is_some() && !curr.in_handle.unwrap().abs_diff_eq(curr.anchor, 1e-5))
						|| (curr.out_handle.is_some() && !curr.out_handle.unwrap().abs_diff_eq(curr.anchor, 1e-5));

					// If the point already has handles, or if it's an endpoint of an open path, keep it as is.
					if has_handles || (!is_closed && (i == 0 || i == groups.len() - 1)) {
						new_groups.push(*curr);
						continue;
					}
				}

				// If spread is 0, remove handles for this point, making it a sharp corner.
				if spread == 0. {
					new_groups.push(ManipulatorGroup {
						anchor: curr.anchor,
						in_handle: None,
						out_handle: None,
						id: curr.id,
					});
					continue;
				}

				// Get previous and next points for auto-tangent calculation
				let prev_idx = if i == 0 { if is_closed { groups.len() - 1 } else { i } } else { i - 1 };
				let next_idx = if i == groups.len() - 1 { if is_closed { 0 } else { i } } else { i + 1 };

				let prev = groups[prev_idx].anchor;
				let curr_pos = curr.anchor;
				let next = groups[next_idx].anchor;

				// Calculate directions from current point to adjacent points
				let dir_prev = (prev - curr_pos).normalize_or_zero();
				let dir_next = (next - curr_pos).normalize_or_zero();

				// Check if we have valid directions (e.g., points are not coincident)
				if dir_prev.length_squared() < 1e-5 || dir_next.length_squared() < 1e-5 {
					// Fallback: keep the original manipulator group (which has no active handles here)
					new_groups.push(*curr);
					continue;
				}

				// Calculate handle direction (colinear, pointing along the line from prev to next)
				// Original logic: (dir_prev - dir_next) is equivalent to (prev - curr) - (next - curr) = prev - next
				// The handle_dir will be along the line connecting prev and next, or perpendicular if they are coincident.
				let mut handle_dir = (dir_prev - dir_next).try_normalize().unwrap_or_else(|| dir_prev.perp());

				// Ensure consistent orientation of the handle_dir
				// This makes the `+ handle_dir` for in_handle and `- handle_dir` for out_handle consistent
				if dir_prev.dot(handle_dir) < 0. {
					handle_dir = -handle_dir;
				}

				// Calculate handle lengths: 1/3 of distance to adjacent points, scaled by spread
				let in_length = (curr_pos - prev).length() / 3. * spread;
				let out_length = (next - curr_pos).length() / 3. * spread;

				// Create new manipulator group with calculated auto-tangents
				new_groups.push(ManipulatorGroup {
					anchor: curr_pos,
					in_handle: Some(curr_pos + handle_dir * in_length),
					out_handle: Some(curr_pos - handle_dir * out_length),
					id: curr.id,
				});
			}

			let mut softened_subpath = Subpath::new(new_groups, is_closed);
			softened_subpath.apply_transform(transform.inverse());
			result.append_subpath(softened_subpath, true);
		}

		result_table.push(Instance {
			instance: result,
			transform,
			alpha_blending,
			source_node_id,
		});
	}

	result_table
}

// TODO: Fix issues and reenable
// #[node_macro::node(category("Vector"), path(graphene_core::vector))]
// async fn subdivide(
// 	_: impl Ctx,
// 	source: VectorDataTable,
// 	#[default(1.)]
// 	#[hard_min(1.)]
// 	#[soft_max(8.)]
// 	subdivisions: f64,
// ) -> VectorDataTable {
// 	fn subdivide_once(subpath: &Subpath<PointId>) -> Subpath<PointId> {
// 		let original_groups = subpath.manipulator_groups();
// 		let mut new_groups = Vec::new();
// 		let is_closed = subpath.closed();
// 		let mut last_in_handle = None;

// 		for i in 0..original_groups.len() {
// 			let start_idx = i;
// 			let end_idx = (i + 1) % original_groups.len();

// 			// Skip the last segment for open paths
// 			if !is_closed && end_idx == 0 {
// 				break;
// 			}

// 			let current_bezier = original_groups[start_idx].to_bezier(&original_groups[end_idx]);

// 			// Create modified start point with original ID, but updated in_handle & out_handle
// 			let mut start_point = original_groups[start_idx];
// 			let [first, _] = current_bezier.split(TValue::Euclidean(0.5));
// 			start_point.out_handle = first.handle_start();
// 			start_point.in_handle = last_in_handle;
// 			if new_groups.contains(&start_point) {
// 				debug!("start_point already in");
// 			} else {
// 				new_groups.push(start_point);
// 			}

// 			// Add midpoint
// 			let [first, second] = current_bezier.split(TValue::Euclidean(0.5));

// 			let new_point = ManipulatorGroup {
// 				anchor: first.end,
// 				in_handle: first.handle_end(),
// 				out_handle: second.handle_start(),
// 				id: start_point.id.generate_from_hash(u64::MAX),
// 			};
// 			if new_groups.contains(&new_point) {
// 				debug!("new_point already in");
// 			} else {
// 				new_groups.push(new_point);
// 			}

// 			last_in_handle = second.handle_end();
// 		}

// 		// Handle the final point for open paths
// 		if !is_closed && !original_groups.is_empty() {
// 			let mut last_point = *original_groups.last().unwrap();
// 			last_point.in_handle = last_in_handle;
// 			if new_groups.contains(&last_point) {
// 				debug!("last_point already in");
// 			} else {
// 				new_groups.push(last_point);
// 			}
// 		} else if is_closed && !new_groups.is_empty() {
// 			// Update the first point's in_handle for closed paths
// 			new_groups[0].in_handle = last_in_handle;
// 		}

// 		Subpath::new(new_groups, is_closed)
// 	}

// 	let mut result_table = VectorDataTable::default();

// 	for source_vector_data in source.instances() {
// 		let source_transform = *source_vector_data.transform;
// 		let source_vector_data = source_vector_data.instance;

// 		let subdivisions = subdivisions as usize;

//		let mut result = VectorData {
//			style: source_vector_data.style.clone(),
//			..Default::default()
//		};

// 		for mut subpath in source_vector_data.stroke_bezier_paths() {
// 			subpath.apply_transform(source_transform);

// 			if subpath.manipulator_groups().len() < 2 {
// 				// Not enough points to subdivide
// 				result.append_subpath(subpath, true);
// 				continue;
// 			}

// 			// Apply subdivisions recursively
// 			let mut current_subpath = subpath;
// 			for _ in 0..subdivisions {
// 				current_subpath = subdivide_once(&current_subpath);
// 			}

// 			current_subpath.apply_transform(source_transform.inverse());
// 			result.append_subpath(current_subpath, true);
// 		}

// 		let pushed = result_table.push(result);
// 		*pushed.transform = source_transform;
// 	}

// 	result_table
// }

#[node_macro::node(category("Vector: Modifier"), path(graphene_core::vector))]
async fn bounding_box(_: impl Ctx, vector_data: VectorDataTable) -> VectorDataTable {
	let mut result_table = VectorDataTable::default();

	for mut vector_data_instance in vector_data.instance_iter() {
		let vector_data = vector_data_instance.instance;

		let mut result = vector_data
			.bounding_box_rect()
			.map(|bbox| {
				let mut vector_data = VectorData::default();
				vector_data.append_bezpath(bbox.to_path(DEFAULT_ACCURACY));
				vector_data
			})
			.unwrap_or_default();

		result.style = vector_data.style.clone();
		result.style.set_stroke_transform(DAffine2::IDENTITY);

		vector_data_instance.instance = result;
		vector_data_instance.source_node_id = None;
		result_table.push(vector_data_instance);
	}

	result_table
}

#[node_macro::node(category("Vector: Measure"), path(graphene_core::vector))]
async fn dimensions(_: impl Ctx, vector_data: VectorDataTable) -> DVec2 {
	vector_data
		.instance_ref_iter()
		.filter_map(|vector_data| vector_data.instance.bounding_box_with_transform(*vector_data.transform))
		.reduce(|[acc_top_left, acc_bottom_right], [top_left, bottom_right]| [acc_top_left.min(top_left), acc_bottom_right.max(bottom_right)])
		.map(|[top_left, bottom_right]| bottom_right - top_left)
		.unwrap_or_default()
}

/// Converts a coordinate value into a vector anchor point.
///
/// This is useful in conjunction with nodes that repeat it, followed by the "Points to Polyline" node to string together a path of the points.
#[node_macro::node(category("Vector"), name("Coordinate to Point"), path(graphene_core::vector))]
async fn position_to_point(_: impl Ctx, coordinate: DVec2) -> VectorDataTable {
	let mut result_table = VectorDataTable::default();

	let mut point_domain = PointDomain::new();
	point_domain.push(PointId::generate(), coordinate);

	result_table.push(Instance {
		instance: VectorData { point_domain, ..Default::default() },
		..Default::default()
	});

	result_table
}

/// Creates a polyline from a series of vector points, replacing any existing segments and regions that may already exist.
#[node_macro::node(category("Vector"), name("Points to Polyline"), path(graphene_core::vector))]
async fn points_to_polyline(_: impl Ctx, mut points: VectorDataTable, #[default(true)] closed: bool) -> VectorDataTable {
	for instance in points.instance_mut_iter() {
		let mut segment_domain = SegmentDomain::new();

		let points_count = instance.instance.point_domain.ids().len();

		if points_count > 2 {
			(0..points_count - 1).for_each(|i| {
				segment_domain.push(SegmentId::generate(), i, i + 1, bezier_rs::BezierHandles::Linear, StrokeId::generate());
			});

			if closed {
				segment_domain.push(SegmentId::generate(), points_count - 1, 0, bezier_rs::BezierHandles::Linear, StrokeId::generate());

				instance
					.instance
					.region_domain
					.push(RegionId::generate(), segment_domain.ids()[0]..=*segment_domain.ids().last().unwrap(), FillId::generate());
			}
		}

		instance.instance.segment_domain = segment_domain;
	}

	points
}

#[node_macro::node(category("Vector: Modifier"), path(graphene_core::vector), properties("offset_path_properties"))]
async fn offset_path(_: impl Ctx, vector_data: VectorDataTable, distance: f64, join: StrokeJoin, #[default(4.)] miter_limit: f64) -> VectorDataTable {
	let mut result_table = VectorDataTable::default();

	for mut vector_data_instance in vector_data.instance_iter() {
		let vector_data_transform = vector_data_instance.transform;
		let vector_data = vector_data_instance.instance;

		let subpaths = vector_data.stroke_bezier_paths();
		let mut result = VectorData {
			style: vector_data.style.clone(),
			..Default::default()
		};
		result.style.set_stroke_transform(DAffine2::IDENTITY);

		// Perform operation on all subpaths in this shape.
		for mut subpath in subpaths {
			subpath.apply_transform(vector_data_transform);

			// Taking the existing stroke data and passing it to Bezier-rs to generate new paths.
			let mut subpath_out = offset_subpath(
				&subpath,
				-distance,
				match join {
					StrokeJoin::Miter => Join::Miter(Some(miter_limit)),
					StrokeJoin::Bevel => Join::Bevel,
					StrokeJoin::Round => Join::Round,
				},
			);

			subpath_out.apply_transform(vector_data_transform.inverse());

			// One closed subpath, open path.
			result.append_subpath(subpath_out, false);
		}

		vector_data_instance.instance = result;
		vector_data_instance.source_node_id = None;
		result_table.push(vector_data_instance);
	}

	result_table
}

#[node_macro::node(category("Vector: Modifier"), path(graphene_core::vector))]
async fn solidify_stroke(_: impl Ctx, vector_data: VectorDataTable) -> VectorDataTable {
	let mut result_table = VectorDataTable::default();

	for mut vector_data_instance in vector_data.instance_iter() {
		let vector_data = vector_data_instance.instance;

		let stroke = vector_data.style.stroke().clone().unwrap_or_default();
		let bezpaths = vector_data.stroke_bezpath_iter();
		let mut result = VectorData::default();

		// Taking the existing stroke data and passing it to kurbo::stroke to generate new fill paths.
		let join = match stroke.join {
			StrokeJoin::Miter => kurbo::Join::Miter,
			StrokeJoin::Bevel => kurbo::Join::Bevel,
			StrokeJoin::Round => kurbo::Join::Round,
		};
		let cap = match stroke.cap {
			StrokeCap::Butt => kurbo::Cap::Butt,
			StrokeCap::Round => kurbo::Cap::Round,
			StrokeCap::Square => kurbo::Cap::Square,
		};
		let dash_offset = stroke.dash_offset;
		let dash_pattern = stroke.dash_lengths;
		let miter_limit = stroke.join_miter_limit;

		let stroke_style = kurbo::Stroke::new(stroke.weight)
			.with_caps(cap)
			.with_join(join)
			.with_dashes(dash_offset, dash_pattern)
			.with_miter_limit(miter_limit);

		let stroke_options = kurbo::StrokeOpts::default();

		// 0.25 is balanced between performace and accuracy of the curve.
		const STROKE_TOLERANCE: f64 = 0.25;

		for path in bezpaths {
			let solidified = kurbo::stroke(path, &stroke_style, &stroke_options, STROKE_TOLERANCE);
			result.append_bezpath(solidified);
		}

		// We set our fill to our stroke's color, then clear our stroke.
		if let Some(stroke) = vector_data.style.stroke() {
			result.style.set_fill(Fill::solid_or_none(stroke.color));
			result.style.set_stroke(Stroke::default());
		}

		vector_data_instance.instance = result;
		vector_data_instance.source_node_id = None;
		result_table.push(vector_data_instance);
	}

	result_table
}

/// Convert vector geometry into a polyline composed of evenly spaced points.
#[node_macro::node(category(""), path(graphene_core::vector))]
async fn sample_polyline(
	_: impl Ctx,
	vector_data: VectorDataTable,
	spacing: PointSpacingType,
	separation: f64,
	quantity: f64,
	start_offset: f64,
	stop_offset: f64,
	adaptive_spacing: bool,
	subpath_segment_lengths: Vec<f64>,
) -> VectorDataTable {
	let mut result_table = VectorDataTable::default();

	for mut vector_data_instance in vector_data.instance_iter() {
		let mut result = VectorData {
			point_domain: Default::default(),
			segment_domain: Default::default(),
			region_domain: Default::default(),
			colinear_manipulators: Default::default(),
			style: std::mem::take(&mut vector_data_instance.instance.style),
			upstream_graphic_group: std::mem::take(&mut vector_data_instance.instance.upstream_graphic_group),
		};
		// Transfer the stroke transform from the input vector data to the result.
		result.style.set_stroke_transform(vector_data_instance.transform);

		// Using `stroke_bezpath_iter` so that the `subpath_segment_lengths` is aligned to the segments of each bezpath.
		// So we can index into `subpath_segment_lengths` to get the length of the segments.
		// NOTE: `subpath_segment_lengths` has precalulated lengths with transformation applied.
		let bezpaths = vector_data_instance.instance.stroke_bezpath_iter();

		// Keeps track of the index of the first segment of the next bezpath in order to get lengths of all segments.
		let mut next_segment_index = 0;

		for mut bezpath in bezpaths {
			// Apply the tranformation to the current bezpath to calculate points after transformation.
			bezpath.apply_affine(Affine::new(vector_data_instance.transform.to_cols_array()));

			let segment_count = bezpath.segments().count();

			// For the current bezpath we get its segment's length by calculating the start index and end index.
			let current_bezpath_segments_length = &subpath_segment_lengths[next_segment_index..next_segment_index + segment_count];

			// Increment the segment index by the number of segments in the current bezpath to calculate the next bezpath segment's length.
			next_segment_index += segment_count;

			let amount = match spacing {
				PointSpacingType::Separation => separation,
				PointSpacingType::Quantity => quantity,
			};
			let Some(mut sample_bezpath) = sample_polyline_on_bezpath(bezpath, spacing, amount, start_offset, stop_offset, adaptive_spacing, current_bezpath_segments_length) else {
				continue;
			};

			// Reverse the transformation applied to the bezpath as the `result` already has the transformation set.
			sample_bezpath.apply_affine(Affine::new(vector_data_instance.transform.to_cols_array()).inverse());

			// Append the bezpath (subpath) that connects generated points by lines.
			result.append_bezpath(sample_bezpath);
		}

		vector_data_instance.instance = result;
		result_table.push(vector_data_instance);
	}

	result_table
}

/// Splits a path at a given progress from 0 to 1 along the path, creating two new subpaths from the original one (if the path is initially open) or one open subpath (if the path is initially closed).
///
/// If multiple subpaths make up the path, the whole number part of the progress value selects the subpath and the decimal part determines the position along it.
#[node_macro::node(category("Vector: Modifier"), path(graphene_core::vector))]
async fn split_path(_: impl Ctx, mut vector_data: VectorDataTable, progress: Fraction, parameterized_distance: bool, reverse: bool) -> VectorDataTable {
	let euclidian = !parameterized_distance;

	let bezpaths = vector_data
		.instance_ref_iter()
		.enumerate()
		.flat_map(|(instance_row_index, vector_data)| vector_data.instance.stroke_bezpath_iter().map(|bezpath| (instance_row_index, bezpath)).collect::<Vec<_>>())
		.collect::<Vec<_>>();

	let bezpath_count = bezpaths.len() as f64;
	let t_value = progress.clamp(0., bezpath_count);
	let t_value = if reverse { bezpath_count - t_value } else { t_value };
	let index = if t_value >= bezpath_count { (bezpath_count - 1.) as usize } else { t_value as usize };

	if let Some((instance_row_index, bezpath)) = bezpaths.get(index).cloned() {
		let mut result_vector_data = VectorData {
			style: vector_data.get(instance_row_index).unwrap().instance.style.clone(),
			..Default::default()
		};

		for (_, (_, bezpath)) in bezpaths.iter().enumerate().filter(|(i, (ri, _))| *i != index && *ri == instance_row_index) {
			result_vector_data.append_bezpath(bezpath.clone());
		}
		let t = if t_value == bezpath_count { 1. } else { t_value.fract() };

		if let Some((first, second)) = split_bezpath(&bezpath, t, euclidian) {
			result_vector_data.append_bezpath(first);
			result_vector_data.append_bezpath(second);
		} else {
			result_vector_data.append_bezpath(bezpath);
		}

		*vector_data.get_mut(instance_row_index).unwrap().instance = result_vector_data;
	}

	vector_data
}

/// Splits path segments into separate disconnected pieces where each is a distinct subpath.
#[node_macro::node(category("Vector: Modifier"), path(graphene_core::vector))]
async fn split_segments(_: impl Ctx, mut vector_data: VectorDataTable) -> VectorDataTable {
	// Iterate through every segment and make a copy of each of its endpoints, then reassign each segment's endpoints to its own unique point copy
	for vector_data_instance in vector_data.instance_mut_iter() {
		let points_count = vector_data_instance.instance.point_domain.ids().len();
		let segments_count = vector_data_instance.instance.segment_domain.ids().len();

		let mut point_usages = vec![0_usize; points_count];

		// Count how many times each point is used as an endpoint of the segments
		let start_points = vector_data_instance.instance.segment_domain.start_point().iter();
		let end_points = vector_data_instance.instance.segment_domain.end_point().iter();
		for (&start, &end) in start_points.zip(end_points) {
			point_usages[start] += 1;
			point_usages[end] += 1;
		}

		let mut new_points = PointDomain::new();
		let mut offset_sum: usize = 0;
		let mut points_with_new_offsets = Vec::with_capacity(points_count);

		// Build a new point domain with the original points, but with duplications based on their extra usages by the segments
		for (index, (point_id, point)) in vector_data_instance.instance.point_domain.iter().enumerate() {
			// Ensure at least one usage to preserve free-floating points not connected to any segments
			let usage_count = point_usages[index].max(1);

			new_points.push_unchecked(point_id, point);

			for i in 1..usage_count {
				new_points.push_unchecked(point_id.generate_from_hash(i as u64), point);
			}

			points_with_new_offsets.push(offset_sum);
			offset_sum += usage_count;
		}

		// Reconcile the segment domain with the new points
		vector_data_instance.instance.point_domain = new_points;
		for original_segment_index in 0..segments_count {
			let original_point_start_index = vector_data_instance.instance.segment_domain.start_point()[original_segment_index];
			let original_point_end_index = vector_data_instance.instance.segment_domain.end_point()[original_segment_index];

			point_usages[original_point_start_index] -= 1;
			point_usages[original_point_end_index] -= 1;

			let start_usage = points_with_new_offsets[original_point_start_index] + point_usages[original_point_start_index];
			let end_usage = points_with_new_offsets[original_point_end_index] + point_usages[original_point_end_index];

			vector_data_instance.instance.segment_domain.set_start_point(original_segment_index, start_usage);
			vector_data_instance.instance.segment_domain.set_end_point(original_segment_index, end_usage);
		}
	}

	vector_data
}

/// Determines the position of a point on the path, given by its progress from 0 to 1 along the path.
///
/// If multiple subpaths make up the path, the whole number part of the progress value selects the subpath and the decimal part determines the position along it.
#[node_macro::node(name("Position on Path"), category("Vector: Measure"), path(graphene_core::vector))]
async fn position_on_path(
	_: impl Ctx,
	/// The path to traverse.
	vector_data: VectorDataTable,
	/// The factor from the start to the end of the path, 0–1 for one subpath, 1–2 for a second subpath, and so on.
	progress: Fraction,
	/// Swap the direction of the path.
	reverse: bool,
	/// Traverse the path using each segment's Bézier curve parameterization instead of the Euclidean distance. Faster to compute but doesn't respect actual distances.
	parameterized_distance: bool,
) -> DVec2 {
	let euclidian = !parameterized_distance;

	let mut bezpaths = vector_data
		.instance_iter()
		.flat_map(|vector_data| {
			let transform = vector_data.transform;
			vector_data.instance.stroke_bezpath_iter().map(|bezpath| (bezpath, transform)).collect::<Vec<_>>()
		})
		.collect::<Vec<_>>();
	let bezpath_count = bezpaths.len() as f64;
	let progress = progress.clamp(0., bezpath_count);
	let progress = if reverse { bezpath_count - progress } else { progress };
	let index = if progress >= bezpath_count { (bezpath_count - 1.) as usize } else { progress as usize };

	bezpaths.get_mut(index).map_or(DVec2::ZERO, |(bezpath, transform)| {
		let t = if progress == bezpath_count { 1. } else { progress.fract() };
		bezpath.apply_affine(Affine::new(transform.to_cols_array()));

		point_to_dvec2(position_on_bezpath(bezpath, t, euclidian, None))
	})
}

/// Determines the angle of the tangent at a point on the path, given by its progress from 0 to 1 along the path.
///
/// If multiple subpaths make up the path, the whole number part of the progress value selects the subpath and the decimal part determines the position along it.
#[node_macro::node(name("Tangent on Path"), category("Vector: Measure"), path(graphene_core::vector))]
async fn tangent_on_path(
	_: impl Ctx,
	/// The path to traverse.
	vector_data: VectorDataTable,
	/// The factor from the start to the end of the path, 0–1 for one subpath, 1–2 for a second subpath, and so on.
	progress: Fraction,
	/// Swap the direction of the path.
	reverse: bool,
	/// Traverse the path using each segment's Bézier curve parameterization instead of the Euclidean distance. Faster to compute but doesn't respect actual distances.
	parameterized_distance: bool,
) -> f64 {
	let euclidian = !parameterized_distance;

	let mut bezpaths = vector_data
		.instance_iter()
		.flat_map(|vector_data| {
			let transform = vector_data.transform;
			vector_data.instance.stroke_bezpath_iter().map(|bezpath| (bezpath, transform)).collect::<Vec<_>>()
		})
		.collect::<Vec<_>>();
	let bezpath_count = bezpaths.len() as f64;
	let progress = progress.clamp(0., bezpath_count);
	let progress = if reverse { bezpath_count - progress } else { progress };
	let index = if progress >= bezpath_count { (bezpath_count - 1.) as usize } else { progress as usize };

	bezpaths.get_mut(index).map_or(0., |(bezpath, transform)| {
		let t = if progress == bezpath_count { 1. } else { progress.fract() };
		bezpath.apply_affine(Affine::new(transform.to_cols_array()));

		let mut tangent = point_to_dvec2(tangent_on_bezpath(bezpath, t, euclidian, None));
		if tangent == DVec2::ZERO {
			let t = t + if t > 0.5 { -0.001 } else { 0.001 };
			tangent = point_to_dvec2(tangent_on_bezpath(bezpath, t, euclidian, None));
		}
		if tangent == DVec2::ZERO {
			return 0.;
		}

		-tangent.angle_to(if reverse { -DVec2::X } else { DVec2::X })
	})
}

#[node_macro::node(category(""), path(graphene_core::vector))]
async fn poisson_disk_points(
	_: impl Ctx,
	vector_data: VectorDataTable,
	#[default(10.)]
	#[hard_min(0.01)]
	separation_disk_diameter: f64,
	seed: SeedValue,
) -> VectorDataTable {
	let mut rng = rand::rngs::StdRng::seed_from_u64(seed.into());

	let mut result_table = VectorDataTable::default();

	for mut vector_data_instance in vector_data.instance_iter() {
		let mut result = VectorData::default();

		let path_with_bounding_boxes: Vec<_> = vector_data_instance
			.instance
			.stroke_bezpath_iter()
			.map(|mut bezpath| {
				// TODO: apply transform to points instead of modifying the paths
				bezpath.close_path();
				let bbox = bezpath.bounding_box();
				(bezpath, bbox)
			})
			.collect();

		for (i, (subpath, _)) in path_with_bounding_boxes.iter().enumerate() {
			if subpath.segments().count() < 2 {
				continue;
			}

			for point in bezpath_algorithms::poisson_disk_points(i, &path_with_bounding_boxes, separation_disk_diameter, || rng.random::<f64>()) {
				result.point_domain.push(PointId::generate(), point);
			}
		}

		// Transfer the style from the input vector data to the result.
		result.style = vector_data_instance.instance.style.clone();
		result.style.set_stroke_transform(DAffine2::IDENTITY);

		vector_data_instance.instance = result;

		result_table.push(vector_data_instance);
	}

	result_table
}

#[node_macro::node(category(""), path(graphene_core::vector))]
async fn subpath_segment_lengths(_: impl Ctx, vector_data: VectorDataTable) -> Vec<f64> {
	vector_data
		.instance_iter()
		.flat_map(|vector_data| {
			let transform = vector_data.transform;
			vector_data
				.instance
				.stroke_bezpath_iter()
				.flat_map(|mut bezpath| {
					bezpath.apply_affine(Affine::new(transform.to_cols_array()));
					bezpath.segments().map(|segment| segment.perimeter(DEFAULT_ACCURACY)).collect::<Vec<f64>>()
				})
				.collect::<Vec<f64>>()
		})
		.collect()
}

#[node_macro::node(name("Spline"), category("Vector: Modifier"), path(graphene_core::vector))]
async fn spline(_: impl Ctx, vector_data: VectorDataTable) -> VectorDataTable {
	let mut result_table = VectorDataTable::default();

	for mut vector_data_instance in vector_data.instance_iter() {
		// Exit early if there are no points to generate splines from.
		if vector_data_instance.instance.point_domain.positions().is_empty() {
			continue;
		}

		let mut segment_domain = SegmentDomain::default();
		for (manipulator_groups, closed) in vector_data_instance.instance.stroke_manipulator_groups() {
			let positions = manipulator_groups.iter().map(|group| group.anchor).collect::<Vec<_>>();
			let closed = closed && positions.len() > 2;

			// Compute control point handles for Bezier spline.
			let first_handles = if closed {
				solve_spline_first_handle_closed(&positions)
			} else {
				solve_spline_first_handle_open(&positions)
			};

			let stroke_id = StrokeId::ZERO;

			// Create segments with computed Bezier handles and add them to vector data.
			for i in 0..(positions.len() - if closed { 0 } else { 1 }) {
				let next_index = (i + 1) % positions.len();

				let start_index = vector_data_instance.instance.point_domain.resolve_id(manipulator_groups[i].id).unwrap();
				let end_index = vector_data_instance.instance.point_domain.resolve_id(manipulator_groups[next_index].id).unwrap();

				let handle_start = first_handles[i];
				let handle_end = positions[next_index] * 2. - first_handles[next_index];
				let handles = bezier_rs::BezierHandles::Cubic { handle_start, handle_end };

				segment_domain.push(SegmentId::generate(), start_index, end_index, handles, stroke_id);
			}
		}

		vector_data_instance.instance.segment_domain = segment_domain;
		result_table.push(vector_data_instance);
	}

	result_table
}

#[node_macro::node(category("Vector: Modifier"), path(graphene_core::vector))]
async fn jitter_points(_: impl Ctx, vector_data: VectorDataTable, #[default(5.)] amount: f64, seed: SeedValue) -> VectorDataTable {
	let mut result_table = VectorDataTable::default();

	for mut vector_data_instance in vector_data.instance_iter() {
		let mut rng = rand::rngs::StdRng::seed_from_u64(seed.into());

		let vector_data_transform = vector_data_instance.transform;
		let inverse_transform = (vector_data_transform.matrix2.determinant() != 0.).then(|| vector_data_transform.inverse()).unwrap_or_default();

		let deltas = (0..vector_data_instance.instance.point_domain.positions().len())
			.map(|_| {
				let angle = rng.random::<f64>() * TAU;

				inverse_transform.transform_vector2(DVec2::from_angle(angle) * rng.random::<f64>() * amount)
			})
			.collect::<Vec<_>>();
		let mut already_applied = vec![false; vector_data_instance.instance.point_domain.positions().len()];

		for (handles, start, end) in vector_data_instance.instance.segment_domain.handles_and_points_mut() {
			let start_delta = deltas[*start];
			let end_delta = deltas[*end];

			if !already_applied[*start] {
				let start_position = vector_data_instance.instance.point_domain.positions()[*start];
				vector_data_instance.instance.point_domain.set_position(*start, start_position + start_delta);
				already_applied[*start] = true;
			}
			if !already_applied[*end] {
				let end_position = vector_data_instance.instance.point_domain.positions()[*end];
				vector_data_instance.instance.point_domain.set_position(*end, end_position + end_delta);
				already_applied[*end] = true;
			}

			match handles {
				bezier_rs::BezierHandles::Cubic { handle_start, handle_end } => {
					*handle_start += start_delta;
					*handle_end += end_delta;
				}
				bezier_rs::BezierHandles::Quadratic { handle } => {
					*handle = vector_data_instance.transform.transform_point2(*handle) + (start_delta + end_delta) / 2.;
				}
				bezier_rs::BezierHandles::Linear => {}
			}
		}

		vector_data_instance.instance.style.set_stroke_transform(DAffine2::IDENTITY);
		result_table.push(vector_data_instance);
	}

	result_table
}

#[node_macro::node(category("Vector: Modifier"), path(graphene_core::vector))]
async fn morph(_: impl Ctx, source: VectorDataTable, #[expose] target: VectorDataTable, #[default(0.5)] time: Fraction) -> VectorDataTable {
	/// Subdivides the last segment of the bezpath to until it appends 'count' number of segments.
	fn make_new_segments(bezpath: &mut BezPath, count: usize) {
		let bezpath_segment_count = bezpath.segments().count();

		if count == 0 || bezpath_segment_count == 0 {
			return;
		}

		// Initially push the last segment of the bezpath
		let mut new_segments = vec![bezpath.get_seg(bezpath_segment_count).unwrap()];

		// Generate new segments by subdividing last segment
		for _ in 0..count {
			let last = new_segments.pop().unwrap();
			let (first, second) = last.subdivide();
			new_segments.push(first);
			new_segments.push(second);
		}

		// Append the new segments.
		if count != 0 {
			// Remove the last segment as it is already appended to the new_segments.
			let mut is_closed = false;
			if let Some(last_element) = bezpath.pop() {
				if last_element == PathEl::ClosePath {
					is_closed = true;
					_ = bezpath.pop();
				}
			}

			for segment in new_segments {
				if bezpath.elements().is_empty() {
					bezpath.move_to(segment.start())
				}
				bezpath.push(segment.as_path_el());
			}

			if is_closed {
				bezpath.close_path();
			}
		}
	}

	let time = time.clamp(0., 1.);

	let mut result_table = VectorDataTable::default();

	for (source_instance, target_instance) in source.instance_iter().zip(target.instance_iter()) {
		let mut vector_data_instance = VectorData::default();

		// Lerp styles
		let vector_data_alpha_blending = source_instance.alpha_blending.lerp(&target_instance.alpha_blending, time as f32);
		vector_data_instance.style = source_instance.instance.style.lerp(&target_instance.instance.style, time);

		// Before and after transforms
		let source_transform = source_instance.transform;
		let target_transform = target_instance.transform;

		// Before and after paths
		let source_bezpaths = source_instance.instance.stroke_bezpath_iter();
		let target_bezpaths = target_instance.instance.stroke_bezpath_iter();

		for (mut source_bezpath, mut target_bezpath) in source_bezpaths.zip(target_bezpaths) {
			if source_bezpath.elements().is_empty() || target_bezpath.elements().is_empty() {
				continue;
			}

			source_bezpath.apply_affine(Affine::new(source_transform.to_cols_array()));
			target_bezpath.apply_affine(Affine::new(target_transform.to_cols_array()));

			let target_segment_len = target_bezpath.segments().count();
			let source_segment_len = source_bezpath.segments().count();

			// Insert new segments to align the number of segments in sorce_bezpath and target_bezpath.
			make_new_segments(&mut source_bezpath, target_segment_len.max(source_segment_len) - source_segment_len);
			make_new_segments(&mut target_bezpath, source_segment_len.max(target_segment_len) - target_segment_len);

			let source_segments = source_bezpath.segments().collect::<Vec<PathSeg>>();
			let target_segments = target_bezpath.segments().collect::<Vec<PathSeg>>();

			// Interpolate anchors and handles
			for (i, (source_element, target_element)) in source_bezpath.elements_mut().iter_mut().zip(target_bezpath.elements_mut().iter_mut()).enumerate() {
				match source_element {
					PathEl::MoveTo(point) => *point = point.lerp(target_element.end_point().unwrap(), time),
					PathEl::ClosePath => {}
					elm => {
						let mut source_segment = source_segments.get(i - 1).unwrap().to_cubic();
						let target_segment = target_segments.get(i - 1).unwrap().to_cubic();
						source_segment.p0 = source_segment.p0.lerp(target_segment.p0, time);
						source_segment.p1 = source_segment.p1.lerp(target_segment.p1, time);
						source_segment.p2 = source_segment.p2.lerp(target_segment.p2, time);
						source_segment.p3 = source_segment.p3.lerp(target_segment.p3, time);
						*elm = PathSeg::Cubic(source_segment).as_path_el();
					}
				}
			}

			vector_data_instance.append_bezpath(source_bezpath.clone());
		}

		// Deal with unmatched extra paths by collapsing them
		let source_paths_count = source_instance.instance.stroke_bezpath_iter().count();
		let target_paths_count = target_instance.instance.stroke_bezpath_iter().count();
		let source_paths = source_instance.instance.stroke_bezpath_iter().skip(target_paths_count);
		let target_paths = target_instance.instance.stroke_bezpath_iter().skip(source_paths_count);

		for mut source_path in source_paths {
			source_path.apply_affine(Affine::new(source_transform.to_cols_array()));

			// Skip if the path has no segments else get the point at the end of the path.
			let Some(end) = source_path.segments().last().and_then(|element| Some(element.end())) else {
				continue;
			};

			for element in source_path.elements_mut() {
				match element {
					PathEl::MoveTo(point) => *point = point.lerp(end, time),
					PathEl::LineTo(point) => *point = point.lerp(end, time),
					PathEl::QuadTo(point, point1) => {
						*point = point.lerp(end, time);
						*point1 = point1.lerp(end, time);
					}
					PathEl::CurveTo(point, point1, point2) => {
						*point = point.lerp(end, time);
						*point1 = point1.lerp(end, time);
						*point2 = point2.lerp(end, time);
					}
					PathEl::ClosePath => {}
				}
			}
			vector_data_instance.append_bezpath(source_path);
		}

		for mut target_path in target_paths {
			target_path.apply_affine(Affine::new(source_transform.to_cols_array()));

			// Skip if the path has no segments else get the point at the start of the path.
			let Some(start) = target_path.segments().next().and_then(|element| Some(element.start())) else {
				continue;
			};

			for element in target_path.elements_mut() {
				match element {
					PathEl::MoveTo(point) => *point = start.lerp(*point, time),
					PathEl::LineTo(point) => *point = start.lerp(*point, time),
					PathEl::QuadTo(point, point1) => {
						*point = start.lerp(*point, time);
						*point1 = start.lerp(*point1, time);
					}
					PathEl::CurveTo(point, point1, point2) => {
						*point = start.lerp(*point, time);
						*point1 = start.lerp(*point1, time);
						*point2 = start.lerp(*point2, time);
					}
					PathEl::ClosePath => {}
				}
			}
			vector_data_instance.append_bezpath(target_path);
		}

		result_table.push(Instance {
			instance: vector_data_instance,
			alpha_blending: vector_data_alpha_blending,
			..Default::default()
		});
	}

	result_table
}

fn bevel_algorithm(mut vector_data: VectorData, vector_data_transform: DAffine2, distance: f64) -> VectorData {
	// Splits a bézier curve based on a distance measurement
	fn split_distance(bezier: bezier_rs::Bezier, distance: f64, length: f64) -> bezier_rs::Bezier {
		const EUCLIDEAN_ERROR: f64 = 0.001;
		let parametric = bezier.euclidean_to_parametric_with_total_length((distance / length).clamp(0., 1.), EUCLIDEAN_ERROR, length);
		bezier.split(bezier_rs::TValue::Parametric(parametric))[1]
	}

	/// Produces a list that corresponds with the point ID. The value is how many segments are connected.
	fn segments_connected_count(vector_data: &VectorData) -> Vec<usize> {
		// Count the number of segments connecting to each point.
		let mut segments_connected_count = vec![0; vector_data.point_domain.ids().len()];
		for &point_index in vector_data.segment_domain.start_point().iter().chain(vector_data.segment_domain.end_point()) {
			segments_connected_count[point_index] += 1;
		}

		// Zero out points without exactly two connectors. These are ignored
		for count in &mut segments_connected_count {
			if *count != 2 {
				*count = 0;
			}
		}
		segments_connected_count
	}

	/// Updates the index so that it points at a point with the position. If nobody else will look at the index, the original point is updated. Otherwise a new point is created.
	fn create_or_modify_point(point_domain: &mut PointDomain, segments_connected_count: &mut [usize], pos: DVec2, index: &mut usize, next_id: &mut PointId, new_segments: &mut Vec<[usize; 2]>) {
		segments_connected_count[*index] -= 1;
		if segments_connected_count[*index] == 0 {
			// If nobody else is going to look at this point, we're alright to modify it
			point_domain.set_position(*index, pos);
		} else {
			let new_index = point_domain.ids().len();
			let original_index = *index;

			// Create a new point (since someone will wish to look at the point in the original position in future)
			*index = new_index;
			point_domain.push(next_id.next_id(), pos);

			// Add a new segment to be created later
			new_segments.push([new_index, original_index]);
		}
	}

	fn update_existing_segments(vector_data: &mut VectorData, vector_data_transform: DAffine2, distance: f64, segments_connected: &mut [usize]) -> Vec<[usize; 2]> {
		let mut next_id = vector_data.point_domain.next_id();
		let mut new_segments = Vec::new();

		for (handles, start_point_index, end_point_index) in vector_data.segment_domain.handles_and_points_mut() {
			// Convert the original segment to a bezier
			let mut bezier = bezier_rs::Bezier {
				start: vector_data.point_domain.positions()[*start_point_index],
				end: vector_data.point_domain.positions()[*end_point_index],
				handles: *handles,
			};

			if bezier.is_linear() {
				bezier.handles = bezier_rs::BezierHandles::Linear;
			}
			bezier = bezier.apply_transformation(|p| vector_data_transform.transform_point2(p));
			let inverse_transform = (vector_data_transform.matrix2.determinant() != 0.).then(|| vector_data_transform.inverse()).unwrap_or_default();

			let original_length = bezier.length(None);
			let mut length = original_length;

			// Only split if the length is big enough to make it worthwhile
			let valid_length = length > 1e-10;
			if segments_connected[*start_point_index] > 0 && valid_length {
				// Apply the bevel to the start
				let distance = distance.min(original_length / 2.);
				bezier = split_distance(bezier, distance, length);
				length = (length - distance).max(0.);
				// Update the start position
				let pos = inverse_transform.transform_point2(bezier.start);
				create_or_modify_point(&mut vector_data.point_domain, segments_connected, pos, start_point_index, &mut next_id, &mut new_segments);
			}

			// Only split if the length is big enough to make it worthwhile
			let valid_length = length > 1e-10;
			if segments_connected[*end_point_index] > 0 && valid_length {
				// Apply the bevel to the end
				let distance = distance.min(original_length / 2.);
				bezier = split_distance(bezier.reversed(), distance, length).reversed();
				// Update the end position
				let pos = inverse_transform.transform_point2(bezier.end);
				create_or_modify_point(&mut vector_data.point_domain, segments_connected, pos, end_point_index, &mut next_id, &mut new_segments);
			}
			// Update the handles
			*handles = bezier.handles.apply_transformation(|p| inverse_transform.transform_point2(p));
		}
		new_segments
	}

	fn insert_new_segments(vector_data: &mut VectorData, new_segments: &[[usize; 2]]) {
		let mut next_id = vector_data.segment_domain.next_id();
		for &[start, end] in new_segments {
			vector_data.segment_domain.push(next_id.next_id(), start, end, bezier_rs::BezierHandles::Linear, StrokeId::ZERO);
		}
	}

	let mut segments_connected = segments_connected_count(&vector_data);
	let new_segments = update_existing_segments(&mut vector_data, vector_data_transform, distance, &mut segments_connected);
	insert_new_segments(&mut vector_data, &new_segments);

	vector_data
}

#[node_macro::node(category("Vector: Modifier"), path(graphene_core::vector))]
fn bevel(_: impl Ctx, source: VectorDataTable, #[default(10.)] distance: Length) -> VectorDataTable {
	let mut result_table = VectorDataTable::default();

	for source_instance in source.instance_iter() {
		result_table.push(Instance {
			instance: bevel_algorithm(source_instance.instance, source_instance.transform, distance),
			..Default::default()
		});
	}

	result_table
}

#[node_macro::node(category("Vector: Modifier"), path(graphene_core::vector))]
fn close_path(_: impl Ctx, source: VectorDataTable) -> VectorDataTable {
	let mut result_table = VectorDataTable::default();

	for mut source_instance in source.instance_iter() {
		source_instance.instance.close_subpaths();
		result_table.push(source_instance);
	}

	result_table
}

#[node_macro::node(category("Vector: Measure"), path(graphene_core::vector))]
fn point_inside(_: impl Ctx, source: VectorDataTable, point: DVec2) -> bool {
	source.instance_iter().any(|instance| instance.instance.check_point_inside_shape(instance.transform, point))
}

#[node_macro::node(category("Vector: Measure"), path(graphene_core::vector))]
async fn path_length(_: impl Ctx, source: VectorDataTable) -> f64 {
	source
		.instance_iter()
		.map(|vector_data_instance| {
			let transform = vector_data_instance.transform;
			vector_data_instance
				.instance
				.stroke_bezpath_iter()
				.map(|mut bezpath| {
					bezpath.apply_affine(Affine::new(transform.to_cols_array()));
					bezpath.perimeter(DEFAULT_ACCURACY)
				})
				.sum::<f64>()
		})
		.sum()
}

#[node_macro::node(category("Vector: Measure"), path(graphene_core::vector))]
async fn area(ctx: impl Ctx + CloneVarArgs + ExtractAll, vector_data: impl Node<Context<'static>, Output = VectorDataTable>) -> f64 {
	let new_ctx = OwnedContextImpl::from(ctx).with_footprint(Footprint::default()).into_context();
	let vector_data = vector_data.eval(new_ctx).await;

	vector_data
		.instance_ref_iter()
		.map(|vector_data_instance| {
			let scale = vector_data_instance.transform.decompose_scale();
			vector_data_instance.instance.stroke_bezier_paths().map(|subpath| subpath.area(Some(1e-3), Some(1e-3))).sum::<f64>() * scale.x * scale.y
		})
		.sum()
}

#[node_macro::node(category("Vector: Measure"), path(graphene_core::vector))]
async fn centroid(ctx: impl Ctx + CloneVarArgs + ExtractAll, vector_data: impl Node<Context<'static>, Output = VectorDataTable>, centroid_type: CentroidType) -> DVec2 {
	let new_ctx = OwnedContextImpl::from(ctx).with_footprint(Footprint::default()).into_context();
	let vector_data = vector_data.eval(new_ctx).await;

	if vector_data.is_empty() {
		return DVec2::ZERO;
	}

	// All subpath centroid positions added together as if they were vectors from the origin.
	let mut centroid = DVec2::ZERO;
	// Cumulative area or length of all subpaths
	let mut sum = 0.;

	for vector_data_instance in vector_data.instance_ref_iter() {
		for subpath in vector_data_instance.instance.stroke_bezier_paths() {
			let partial = match centroid_type {
				CentroidType::Area => subpath.area_centroid_and_area(Some(1e-3), Some(1e-3)).filter(|(_, area)| *area > 0.),
				CentroidType::Length => subpath.length_centroid_and_length(None, true),
			};
			if let Some((subpath_centroid, area_or_length)) = partial {
				let subpath_centroid = vector_data_instance.transform.transform_point2(subpath_centroid);

				sum += area_or_length;
				centroid += area_or_length * subpath_centroid;
			}
		}
	}

	if sum > 0. {
		centroid / sum
	}
	// Without a summed denominator, return the average of all positions instead
	else {
		let mut count: usize = 0;

		let summed_positions = vector_data
			.instance_ref_iter()
			.flat_map(|vector_data_instance| {
				vector_data_instance
					.instance
					.point_domain
					.positions()
					.iter()
					.map(|&p| vector_data_instance.transform.transform_point2(p))
			})
			.inspect(|_| count += 1)
			.sum::<DVec2>();

		if count != 0 { summed_positions / (count as f64) } else { DVec2::ZERO }
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use bezier_rs::Bezier;
	use graphene_core::Node;
	use graphene_core::transform::Footprint;
	use kurbo::Rect;
	use std::pin::Pin;

	#[derive(Clone)]
	pub struct FutureWrapperNode<T: Clone>(T);

	impl<'i, T: 'i + Clone + Send> Node<'i, Footprint> for FutureWrapperNode<T> {
		type Output = Pin<Box<dyn Future<Output = T> + 'i + Send>>;
		fn eval(&'i self, _input: Footprint) -> Self::Output {
			let value = self.0.clone();
			Box::pin(async move { value })
		}
	}

	fn vector_node(data: Subpath<PointId>) -> VectorDataTable {
		VectorDataTable::new(VectorData::from_subpath(data))
	}

	fn create_vector_data_instance(bezpath: BezPath, transform: DAffine2) -> Instance<VectorData> {
		let mut instance = VectorData::default();
		instance.append_bezpath(bezpath);
		Instance {
			instance,
			transform,
			..Default::default()
		}
	}

	fn vector_node_from_instances(data: Vec<Instance<VectorData>>) -> VectorDataTable {
		let mut vector_data_table = VectorDataTable::default();
		for instance in data {
			vector_data_table.push(instance);
		}
		vector_data_table
	}
	#[tokio::test]
	async fn bounding_box() {
		let bounding_box = super::bounding_box((), vector_node(Subpath::new_rect(DVec2::NEG_ONE, DVec2::ONE))).await;
		let bounding_box = bounding_box.instance_ref_iter().next().unwrap().instance;
		assert_eq!(bounding_box.region_bezier_paths().count(), 1);
		let subpath = bounding_box.region_bezier_paths().next().unwrap().1;
		assert_eq!(&subpath.anchors()[..4], &[DVec2::NEG_ONE, DVec2::new(1., -1.), DVec2::ONE, DVec2::new(-1., 1.),]);

		// Test a VectorData with non-zero rotation
		let square = VectorData::from_subpath(Subpath::new_rect(DVec2::NEG_ONE, DVec2::ONE));
		let mut square = VectorDataTable::new(square);
		*square.get_mut(0).unwrap().transform *= DAffine2::from_angle(std::f64::consts::FRAC_PI_4);
		let bounding_box = BoundingBoxNode {
			vector_data: FutureWrapperNode(square),
		}
		.eval(Footprint::default())
		.await;
		let bounding_box = bounding_box.instance_ref_iter().next().unwrap().instance;
		assert_eq!(bounding_box.region_bezier_paths().count(), 1);
		let subpath = bounding_box.region_bezier_paths().next().unwrap().1;
		let expected_bounding_box = [DVec2::NEG_ONE, DVec2::new(1., -1.), DVec2::ONE, DVec2::new(-1., 1.)];
		for i in 0..4 {
			assert_eq!(subpath.anchors()[i], expected_bounding_box[i]);
		}
	}
	#[tokio::test]
	async fn sample_polyline() {
		let path = Subpath::from_bezier(&Bezier::from_cubic_dvec2(DVec2::ZERO, DVec2::ZERO, DVec2::X * 100., DVec2::X * 100.));
		let sample_polyline = super::sample_polyline(Footprint::default(), vector_node(path), PointSpacingType::Separation, 30., 0., 0., 0., false, vec![100.]).await;
		let sample_polyline = sample_polyline.instance_ref_iter().next().unwrap().instance;
		assert_eq!(sample_polyline.point_domain.positions().len(), 4);
		for (pos, expected) in sample_polyline.point_domain.positions().iter().zip([DVec2::X * 0., DVec2::X * 30., DVec2::X * 60., DVec2::X * 90.]) {
			assert!(pos.distance(expected) < 1e-3, "Expected {expected} found {pos}");
		}
	}
	#[tokio::test]
	async fn sample_polyline_adaptive_spacing() {
		let path = Subpath::from_bezier(&Bezier::from_cubic_dvec2(DVec2::ZERO, DVec2::ZERO, DVec2::X * 100., DVec2::X * 100.));
		let sample_polyline = super::sample_polyline(Footprint::default(), vector_node(path), PointSpacingType::Separation, 18., 0., 45., 10., true, vec![100.]).await;
		let sample_polyline = sample_polyline.instance_ref_iter().next().unwrap().instance;
		assert_eq!(sample_polyline.point_domain.positions().len(), 4);
		for (pos, expected) in sample_polyline.point_domain.positions().iter().zip([DVec2::X * 45., DVec2::X * 60., DVec2::X * 75., DVec2::X * 90.]) {
			assert!(pos.distance(expected) < 1e-3, "Expected {expected} found {pos}");
		}
	}
	#[tokio::test]
	async fn poisson() {
		let poisson_points = super::poisson_disk_points(
			Footprint::default(),
			vector_node(Subpath::new_ellipse(DVec2::NEG_ONE * 50., DVec2::ONE * 50.)),
			10. * std::f64::consts::SQRT_2,
			0,
		)
		.await;
		let poisson_points = poisson_points.instance_ref_iter().next().unwrap().instance;
		assert!(
			(20..=40).contains(&poisson_points.point_domain.positions().len()),
			"actual len {}",
			poisson_points.point_domain.positions().len()
		);
		for point in poisson_points.point_domain.positions() {
			assert!(point.length() < 50. + 1., "Expected point in circle {point}")
		}
	}
	#[tokio::test]
	async fn segment_lengths() {
		let subpath = Subpath::from_bezier(&Bezier::from_cubic_dvec2(DVec2::ZERO, DVec2::ZERO, DVec2::X * 100., DVec2::X * 100.));
		let lengths = subpath_segment_lengths(Footprint::default(), vector_node(subpath)).await;
		assert_eq!(lengths, vec![100.]);
	}
	#[tokio::test]
	async fn path_length() {
		let bezpath = Rect::new(100., 100., 201., 201.).to_path(DEFAULT_ACCURACY);
		let transform = DAffine2::from_scale(DVec2::new(2., 2.));
		let instance = create_vector_data_instance(bezpath, transform);
		let instances = (0..5).map(|_| instance.clone()).collect::<Vec<Instance<VectorData>>>();

		let length = super::path_length(Footprint::default(), vector_node_from_instances(instances)).await;

		// 101 (each rectangle edge length) * 4 (rectangle perimeter) * 2 (scale) * 5 (number of rows)
		assert_eq!(length, 101. * 4. * 2. * 5.);
	}
	#[tokio::test]
	async fn spline() {
		let spline = super::spline(Footprint::default(), vector_node(Subpath::new_rect(DVec2::ZERO, DVec2::ONE * 100.))).await;
		let spline = spline.instance_ref_iter().next().unwrap().instance;
		assert_eq!(spline.stroke_bezier_paths().count(), 1);
		assert_eq!(spline.point_domain.positions(), &[DVec2::ZERO, DVec2::new(100., 0.), DVec2::new(100., 100.), DVec2::new(0., 100.)]);
	}
	#[tokio::test]
	async fn morph() {
		let source = Subpath::new_rect(DVec2::ZERO, DVec2::ONE * 100.);
		let target = Subpath::new_ellipse(DVec2::NEG_ONE * 100., DVec2::ZERO);
		let morphed = super::morph(Footprint::default(), vector_node(source), vector_node(target), 0.5).await;
		let morphed = morphed.instance_ref_iter().next().unwrap().instance;
		assert_eq!(
			&morphed.point_domain.positions()[..4],
			vec![DVec2::new(-25., -50.), DVec2::new(50., -25.), DVec2::new(25., 50.), DVec2::new(-50., 25.)]
		);
	}

	#[track_caller]
	fn contains_segment(vector: VectorData, target: Bezier) {
		let segments = vector.segment_bezier_iter().map(|x| x.1);
		let count = segments.filter(|bezier| bezier.abs_diff_eq(&target, 0.01) || bezier.reversed().abs_diff_eq(&target, 0.01)).count();
		assert_eq!(
			count,
			1,
			"Expected exactly one matching segment for {target:?}, but found {count}. The given segments are: {:#?}",
			vector.segment_bezier_iter().collect::<Vec<_>>()
		);
	}

	#[tokio::test]
	async fn bevel_rect() {
		let source = Subpath::new_rect(DVec2::ZERO, DVec2::ONE * 100.);
		let beveled = super::bevel(Footprint::default(), vector_node(source), 5.);
		let beveled = beveled.instance_ref_iter().next().unwrap().instance;

		assert_eq!(beveled.point_domain.positions().len(), 8);
		assert_eq!(beveled.segment_domain.ids().len(), 8);

		// Segments
		contains_segment(beveled.clone(), Bezier::from_linear_dvec2(DVec2::new(5., 0.), DVec2::new(95., 0.)));
		contains_segment(beveled.clone(), Bezier::from_linear_dvec2(DVec2::new(5., 100.), DVec2::new(95., 100.)));
		contains_segment(beveled.clone(), Bezier::from_linear_dvec2(DVec2::new(0., 5.), DVec2::new(0., 95.)));
		contains_segment(beveled.clone(), Bezier::from_linear_dvec2(DVec2::new(100., 5.), DVec2::new(100., 95.)));

		// Joins
		contains_segment(beveled.clone(), Bezier::from_linear_dvec2(DVec2::new(5., 0.), DVec2::new(0., 5.)));
		contains_segment(beveled.clone(), Bezier::from_linear_dvec2(DVec2::new(95., 0.), DVec2::new(100., 5.)));
		contains_segment(beveled.clone(), Bezier::from_linear_dvec2(DVec2::new(100., 95.), DVec2::new(95., 100.)));
		contains_segment(beveled.clone(), Bezier::from_linear_dvec2(DVec2::new(5., 100.), DVec2::new(0., 95.)));
	}

	#[tokio::test]
	async fn bevel_open_curve() {
		let curve = Bezier::from_cubic_dvec2(DVec2::ZERO, DVec2::new(10., 0.), DVec2::new(10., 100.), DVec2::X * 100.);
		let source = Subpath::from_beziers(&[Bezier::from_linear_dvec2(DVec2::X * -100., DVec2::ZERO), curve], false);
		let beveled = super::bevel((), vector_node(source), 5.);
		let beveled = beveled.instance_ref_iter().next().unwrap().instance;

		assert_eq!(beveled.point_domain.positions().len(), 4);
		assert_eq!(beveled.segment_domain.ids().len(), 3);

		// Segments
		contains_segment(beveled.clone(), Bezier::from_linear_dvec2(DVec2::new(-5., 0.), DVec2::new(-100., 0.)));
		let trimmed = curve.trim(bezier_rs::TValue::Euclidean(5. / curve.length(Some(0.00001))), bezier_rs::TValue::Parametric(1.));
		contains_segment(beveled.clone(), trimmed);

		// Join
		contains_segment(beveled.clone(), Bezier::from_linear_dvec2(DVec2::new(-5., 0.), trimmed.start));
	}

	#[tokio::test]
	async fn bevel_with_transform() {
		let curve = Bezier::from_cubic_dvec2(DVec2::new(0., 0.), DVec2::new(1., 0.), DVec2::new(1., 10.), DVec2::new(10., 0.));
		let source = Subpath::<PointId>::from_beziers(&[Bezier::from_linear_dvec2(DVec2::new(-10., 0.), DVec2::ZERO), curve], false);
		let vector_data = VectorData::from_subpath(source);
		let mut vector_data_table = VectorDataTable::new(vector_data.clone());

		*vector_data_table.get_mut(0).unwrap().transform = DAffine2::from_scale_angle_translation(DVec2::splat(10.), 1., DVec2::new(99., 77.));

		let beveled = super::bevel((), VectorDataTable::new(vector_data), 5.);
		let beveled = beveled.instance_ref_iter().next().unwrap().instance;

		assert_eq!(beveled.point_domain.positions().len(), 4);
		assert_eq!(beveled.segment_domain.ids().len(), 3);

		// Segments
		contains_segment(beveled.clone(), Bezier::from_linear_dvec2(DVec2::new(-5., 0.), DVec2::new(-10., 0.)));
		let trimmed = curve.trim(bezier_rs::TValue::Euclidean(5. / curve.length(Some(0.00001))), bezier_rs::TValue::Parametric(1.));
		contains_segment(beveled.clone(), trimmed);

		// Join
		contains_segment(beveled.clone(), Bezier::from_linear_dvec2(DVec2::new(-5., 0.), trimmed.start));
	}

	#[tokio::test]
	async fn bevel_too_high() {
		let source = Subpath::from_anchors([DVec2::ZERO, DVec2::new(100., 0.), DVec2::new(100., 100.), DVec2::new(0., 100.)], false);
		let beveled = super::bevel(Footprint::default(), vector_node(source), 999.);
		let beveled = beveled.instance_ref_iter().next().unwrap().instance;

		assert_eq!(beveled.point_domain.positions().len(), 6);
		assert_eq!(beveled.segment_domain.ids().len(), 5);

		// Segments
		contains_segment(beveled.clone(), Bezier::from_linear_dvec2(DVec2::new(0., 0.), DVec2::new(50., 0.)));
		contains_segment(beveled.clone(), Bezier::from_linear_dvec2(DVec2::new(100., 50.), DVec2::new(100., 50.)));
		contains_segment(beveled.clone(), Bezier::from_linear_dvec2(DVec2::new(100., 50.), DVec2::new(50., 100.)));

		// Joins
		contains_segment(beveled.clone(), Bezier::from_linear_dvec2(DVec2::new(50., 0.), DVec2::new(100., 50.)));
		contains_segment(beveled.clone(), Bezier::from_linear_dvec2(DVec2::new(100., 50.), DVec2::new(50., 100.)));
	}

	#[tokio::test]
	async fn bevel_repeated_point() {
		let curve = Bezier::from_cubic_dvec2(DVec2::ZERO, DVec2::new(10., 0.), DVec2::new(10., 100.), DVec2::X * 100.);
		let point = Bezier::from_cubic_dvec2(DVec2::ZERO, DVec2::ZERO, DVec2::ZERO, DVec2::ZERO);
		let source = Subpath::from_beziers(&[Bezier::from_linear_dvec2(DVec2::X * -100., DVec2::ZERO), point, curve], false);
		let beveled = super::bevel(Footprint::default(), vector_node(source), 5.);
		let beveled = beveled.instance_ref_iter().next().unwrap().instance;

		assert_eq!(beveled.point_domain.positions().len(), 6);
		assert_eq!(beveled.segment_domain.ids().len(), 5);

		// Segments
		contains_segment(beveled.clone(), Bezier::from_linear_dvec2(DVec2::new(-100., 0.), DVec2::new(-5., 0.)));
		contains_segment(beveled.clone(), Bezier::from_linear_dvec2(DVec2::new(-5., 0.), DVec2::new(0., 0.)));
		contains_segment(beveled.clone(), point);
		let [start, end] = curve.split(bezier_rs::TValue::Euclidean(5. / curve.length(Some(0.00001))));
		contains_segment(beveled.clone(), Bezier::from_linear_dvec2(start.start, start.end));
		contains_segment(beveled.clone(), end);
	}
}

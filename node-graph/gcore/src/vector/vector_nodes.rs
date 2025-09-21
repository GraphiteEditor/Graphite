use super::algorithms::bezpath_algorithms::{self, TValue, evaluate_bezpath, sample_polyline_on_bezpath, split_bezpath, tangent_on_bezpath};
use super::algorithms::offset_subpath::offset_bezpath;
use super::algorithms::spline::{solve_spline_first_handle_closed, solve_spline_first_handle_open};
use super::misc::{CentroidType, bezpath_from_manipulator_groups, bezpath_to_manipulator_groups, point_to_dvec2};
use super::style::{Fill, Gradient, GradientStops, Stroke};
use super::{PointId, SegmentDomain, SegmentId, StrokeId, Vector, VectorExt};
use crate::bounds::{BoundingBox, RenderBoundingBox};
use crate::raster_types::{CPU, GPU, Raster};
use crate::registry::types::{Angle, Fraction, IntegerCount, Length, Multiplier, Percentage, PixelLength, PixelSize, SeedValue};
use crate::subpath::{BezierHandles, ManipulatorGroup};
use crate::table::{Table, TableRow, TableRowMut};
use crate::transform::{Footprint, ReferencePoint, Transform};
use crate::vector::PointDomain;
use crate::vector::algorithms::bezpath_algorithms::eval_pathseg_euclidean;
use crate::vector::algorithms::merge_by_distance::MergeByDistanceExt;
use crate::vector::misc::{MergeByDistanceAlgorithm, PointSpacingType, is_linear};
use crate::vector::misc::{handles_to_segment, segment_to_handles};
use crate::vector::style::{PaintOrder, StrokeAlign, StrokeCap, StrokeJoin};
use crate::vector::{FillId, RegionId};
use crate::{CloneVarArgs, Color, Context, Ctx, ExtractAll, Graphic, OwnedContextImpl};
use core::f64::consts::PI;
use core::hash::{Hash, Hasher};
use glam::{DAffine2, DVec2};
use kurbo::{Affine, BezPath, DEFAULT_ACCURACY, Line, ParamCurve, PathEl, PathSeg, Shape};
use rand::{Rng, SeedableRng};
use std::collections::hash_map::DefaultHasher;
use std::f64::consts::TAU;

/// Implemented for types that can be converted to an iterator of vector rows.
/// Used for the fill and stroke node so they can be used on `Table<Graphic>` or `Table<Vector>`.
trait VectorTableIterMut {
	fn vector_iter_mut(&mut self) -> impl Iterator<Item = TableRowMut<'_, Vector>>;
}

impl VectorTableIterMut for Table<Graphic> {
	fn vector_iter_mut(&mut self) -> impl Iterator<Item = TableRowMut<'_, Vector>> {
		// Grab only the direct children
		self.iter_mut().filter_map(|element| element.element.as_vector_mut()).flat_map(move |vector| vector.iter_mut())
	}
}

impl VectorTableIterMut for Table<Vector> {
	fn vector_iter_mut(&mut self) -> impl Iterator<Item = TableRowMut<'_, Vector>> {
		self.iter_mut()
	}
}

#[node_macro::node(category("Vector: Style"), path(graphene_core::vector))]
async fn assign_colors<T>(
	_: impl Ctx,
	/// The content with vector paths to apply the fill and/or stroke style to.
	#[implementations(Table<Graphic>, Table<Vector>)]
	#[widget(ParsedWidgetOverride::Hidden)]
	mut content: T,
	/// Whether to style the fill.
	#[default(true)]
	fill: bool,
	/// Whether to style the stroke.
	stroke: bool,
	/// The range of colors to select from.
	#[widget(ParsedWidgetOverride::Custom = "assign_colors_gradient")]
	gradient: GradientStops,
	/// Whether to reverse the gradient.
	reverse: bool,
	/// Whether to randomize the color selection for each element from throughout the gradient.
	randomize: bool,
	/// The seed used for randomization.
	/// Seed to determine unique variations on the randomized color selection.
	#[widget(ParsedWidgetOverride::Custom = "assign_colors_seed")]
	seed: SeedValue,
	/// The number of elements to span across the gradient before repeating. A 0 value will span the entire gradient once.
	#[widget(ParsedWidgetOverride::Custom = "assign_colors_repeat_every")]
	repeat_every: u32,
) -> T
where
	T: VectorTableIterMut + 'n + Send,
{
	let length = content.vector_iter_mut().count();
	let gradient = if reverse { gradient.reversed() } else { gradient };

	let mut rng = rand::rngs::StdRng::seed_from_u64(seed.into());

	for (i, vector) in content.vector_iter_mut().enumerate() {
		let factor = match randomize {
			true => rng.random::<f64>(),
			false => match repeat_every {
				0 => i as f64 / (length - 1).max(1) as f64,
				1 => 0.,
				_ => i as f64 % repeat_every as f64 / (repeat_every - 1) as f64,
			},
		};

		let color = gradient.evaluate(factor);

		if fill {
			vector.element.style.set_fill(Fill::Solid(color));
		}
		if stroke {
			if let Some(stroke) = vector.element.style.stroke().and_then(|stroke| stroke.with_color(&Some(color))) {
				vector.element.style.set_stroke(stroke);
			}
		}
	}

	content
}

#[node_macro::node(category("Vector: Style"), path(graphene_core::vector), properties("fill_properties"))]
async fn fill<F: Into<Fill> + 'n + Send, V: VectorTableIterMut + 'n + Send>(
	_: impl Ctx,
	/// The content with vector paths to apply the fill style to.
	#[implementations(
		Table<Vector>,
		Table<Vector>,
		Table<Vector>,
		Table<Vector>,
		Table<Graphic>,
		Table<Graphic>,
		Table<Graphic>,
		Table<Graphic>,
	)]
	mut content: V,
	/// The fill to paint the path with.
	#[implementations(
		Fill,
		Table<Color>,
		Table<GradientStops>,
		Gradient,
		Fill,
		Table<Color>,
		Table<GradientStops>,
		Gradient,
	)]
	#[default(Color::BLACK)]
	fill: F,
	_backup_color: Table<Color>,
	_backup_gradient: Gradient,
) -> V {
	let fill: Fill = fill.into();
	for vector in content.vector_iter_mut() {
		vector.element.style.set_fill(fill.clone());
	}

	content
}

/// Applies a stroke style to the vector contained in the input.
#[node_macro::node(category("Vector: Style"), path(graphene_core::vector), properties("stroke_properties"))]
async fn stroke<V>(
	_: impl Ctx,
	/// The content with vector paths to apply the stroke style to.
	#[implementations(Table<Vector>, Table<Graphic>)]
	mut content: Table<V>,
	/// The stroke color.
	#[default(Color::BLACK)]
	color: Table<Color>,
	/// The stroke weight.
	#[unit(" px")]
	#[default(2.)]
	weight: f64,
	/// The alignment of stroke to the path's centerline or (for closed shapes) the inside or outside of the shape.
	align: StrokeAlign,
	/// The shape of the stroke at open endpoints.
	cap: StrokeCap,
	/// The curvature of the bent stroke at sharp corners.
	join: StrokeJoin,
	/// The threshold for when a miter-joined stroke is converted to a bevel-joined stroke when a sharp angle becomes pointier than this ratio.
	#[default(4.)]
	miter_limit: f64,
	/// The order to paint the stroke on top of the fill, or the fill on top of the stroke.
	/// <https://svgwg.org/svg2-draft/painting.html#PaintOrderProperty>
	paint_order: PaintOrder,
	/// The stroke dash lengths. Each length forms a distance in a pattern where the first length is a dash, the second is a gap, and so on. If the list is an odd length, the pattern repeats with solid-gap roles reversed.
	dash_lengths: Vec<f64>,
	/// The phase offset distance from the starting point of the dash pattern.
	#[unit(" px")]
	dash_offset: f64,
) -> Table<V>
where
	Table<V>: VectorTableIterMut + 'n + Send,
{
	let stroke = Stroke {
		color: color.into(),
		weight,
		dash_lengths,
		dash_offset,
		cap,
		join,
		join_miter_limit: miter_limit,
		align,
		transform: DAffine2::IDENTITY,
		non_scaling: false,
		paint_order,
	};

	for vector in content.vector_iter_mut() {
		let mut stroke = stroke.clone();
		stroke.transform *= *vector.transform;
		vector.element.style.set_stroke(stroke);
	}

	content
}

#[node_macro::node(category("Instancing"), path(graphene_core::vector))]
async fn repeat<I: 'n + Send + Clone>(
	_: impl Ctx,
	// TODO: Implement other graphical types.
	#[implementations(Table<Graphic>, Table<Vector>, Table<Raster<CPU>>, Table<Color>, Table<GradientStops>)] instance: Table<I>,
	#[default(100., 100.)]
	// TODO: When using a custom Properties panel layout in document_node_definitions.rs and this default is set, the widget weirdly doesn't show up in the Properties panel. Investigation is needed.
	direction: PixelSize,
	angle: Angle,
	#[default(4)] count: IntegerCount,
) -> Table<I> {
	let angle = angle.to_radians();
	let count = count.max(1);
	let total = (count - 1) as f64;

	let mut result_table = Table::new();

	for index in 0..count {
		let angle = index as f64 * angle / total;
		let translation = index as f64 * direction / total;
		let transform = DAffine2::from_angle(angle) * DAffine2::from_translation(translation);

		for row in instance.iter() {
			let mut row = row.into_cloned();

			let local_translation = DAffine2::from_translation(row.transform.translation);
			let local_matrix = DAffine2::from_mat2(row.transform.matrix2);
			row.transform = local_translation * transform * local_matrix;

			result_table.push(row);
		}
	}

	result_table
}

#[node_macro::node(category("Instancing"), path(graphene_core::vector))]
async fn circular_repeat<I: 'n + Send + Clone>(
	_: impl Ctx,
	#[implementations(Table<Graphic>, Table<Vector>, Table<Raster<CPU>>, Table<Color>, Table<GradientStops>)] instance: Table<I>,
	start_angle: Angle,
	#[unit(" px")]
	#[default(5)]
	radius: f64,
	#[default(5)] count: IntegerCount,
) -> Table<I> {
	let count = count.max(1);

	let mut result_table = Table::new();

	for index in 0..count {
		let angle = DAffine2::from_angle((TAU / count as f64) * index as f64 + start_angle.to_radians());
		let translation = DAffine2::from_translation(radius * DVec2::Y);
		let transform = angle * translation;

		for row in instance.iter() {
			let mut row = row.into_cloned();

			let local_translation = DAffine2::from_translation(row.transform.translation);
			let local_matrix = DAffine2::from_mat2(row.transform.matrix2);
			row.transform = local_translation * transform * local_matrix;

			result_table.push(row);
		}
	}

	result_table
}

#[node_macro::node(name("Copy to Points"), category("Instancing"), path(graphene_core::vector))]
async fn copy_to_points<I: 'n + Send + Clone>(
	_: impl Ctx,
	points: Table<Vector>,
	/// Artwork to be copied and placed at each point.
	#[expose]
	#[implementations(Table<Graphic>, Table<Vector>, Table<Raster<CPU>>, Table<Color>, Table<GradientStops>)]
	instance: Table<I>,
	/// Minimum range of randomized sizes given to each instance.
	#[default(1)]
	#[range((0., 2.))]
	#[unit("x")]
	random_scale_min: Multiplier,
	/// Maximum range of randomized sizes given to each instance.
	#[default(1)]
	#[range((0., 2.))]
	#[unit("x")]
	random_scale_max: Multiplier,
	/// Bias for the probability distribution of randomized sizes (0 is uniform, negatives favor more of small sizes, positives favor more of large sizes).
	#[range((-50., 50.))]
	random_scale_bias: f64,
	/// Seed to determine unique variations on all the randomized instance sizes.
	random_scale_seed: SeedValue,
	/// Range of randomized angles given to each instance, in degrees ranging from furthest clockwise to counterclockwise.
	#[range((0., 360.))]
	random_rotation: Angle,
	/// Seed to determine unique variations on all the randomized instance angles.
	random_rotation_seed: SeedValue,
) -> Table<I> {
	let mut result_table = Table::new();

	let random_scale_difference = random_scale_max - random_scale_min;

	for row in points.into_iter() {
		let mut scale_rng = rand::rngs::StdRng::seed_from_u64(random_scale_seed.into());
		let mut rotation_rng = rand::rngs::StdRng::seed_from_u64(random_rotation_seed.into());

		let do_scale = random_scale_difference.abs() > 1e-6;
		let do_rotation = random_rotation.abs() > 1e-6;

		let points_transform = row.transform;
		for &point in row.element.point_domain.positions() {
			let translation = points_transform.transform_point2(point);

			let rotation = if do_rotation {
				let degrees = (rotation_rng.random::<f64>() - 0.5) * random_rotation;
				degrees / 360. * TAU
			} else {
				0.
			};

			let scale = if do_scale {
				if random_scale_bias.abs() < 1e-6 {
					// Linear
					random_scale_min + scale_rng.random::<f64>() * random_scale_difference
				} else {
					// Weighted (see <https://www.desmos.com/calculator/gmavd3m9bd>)
					let horizontal_scale_factor = 1. - 2_f64.powf(random_scale_bias);
					let scale_factor = (1. - scale_rng.random::<f64>() * horizontal_scale_factor).log2() / random_scale_bias;
					random_scale_min + scale_factor * random_scale_difference
				}
			} else {
				random_scale_min
			};

			let transform = DAffine2::from_scale_angle_translation(DVec2::splat(scale), rotation, translation);

			for mut row in instance.iter().map(|row| row.into_cloned()) {
				row.transform = transform * row.transform;

				result_table.push(row);
			}
		}
	}

	result_table
}

#[node_macro::node(category("Instancing"), path(graphene_core::vector))]
async fn mirror<I: 'n + Send + Clone>(
	_: impl Ctx,
	#[implementations(Table<Graphic>, Table<Vector>, Table<Raster<CPU>>, Table<Color>, Table<GradientStops>)] content: Table<I>,
	#[default(ReferencePoint::Center)] relative_to_bounds: ReferencePoint,
	#[unit(" px")] offset: f64,
	#[range((-90., 90.))] angle: Angle,
	#[default(true)] keep_original: bool,
) -> Table<I>
where
	Table<I>: BoundingBox,
{
	// Normalize the direction vector
	let normal = DVec2::from_angle(angle.to_radians());

	// The mirror reference may be based on the bounding box if an explicit reference point is chosen
	let RenderBoundingBox::Rectangle(bounding_box) = content.bounding_box(DAffine2::IDENTITY, false) else {
		return content;
	};

	let reference_point_location = relative_to_bounds.point_in_bounding_box((bounding_box[0], bounding_box[1]).into());
	let mirror_reference_point = reference_point_location.map(|point| point + normal * offset);

	// Create the reflection matrix
	let reflection = DAffine2::from_mat2_translation(
		glam::DMat2::from_cols(
			DVec2::new(1. - 2. * normal.x * normal.x, -2. * normal.y * normal.x),
			DVec2::new(-2. * normal.x * normal.y, 1. - 2. * normal.y * normal.y),
		),
		DVec2::ZERO,
	);

	// Apply reflection around the reference point
	let reflected_transform = if let Some(mirror_reference_point) = mirror_reference_point {
		DAffine2::from_translation(mirror_reference_point) * reflection * DAffine2::from_translation(-mirror_reference_point)
	} else {
		reflection * DAffine2::from_translation(DVec2::from_angle(angle.to_radians()) * DVec2::splat(-offset))
	};

	let mut result_table = Table::new();

	// Add original instance depending on the keep_original flag
	if keep_original {
		for instance in content.clone().into_iter() {
			result_table.push(instance);
		}
	}

	// Create and add mirrored instance
	for mut row in content.into_iter() {
		row.transform = reflected_transform * row.transform;
		result_table.push(row);
	}

	result_table
}

#[node_macro::node(category("Vector: Modifier"), path(graphene_core::vector))]
async fn round_corners(
	_: impl Ctx,
	source: Table<Vector>,
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
) -> Table<Vector> {
	source
		.iter()
		.map(|source| {
			let source_transform = *source.transform;
			let source_transform_inverse = source_transform.inverse();
			let source_node_id = source.source_node_id;
			let source = source.element;

			let upstream_nested_layers = source.upstream_nested_layers.clone();

			// Flip the roundness to help with user intuition
			let roundness = 1. - roundness;
			// Convert 0-100 to 0-0.5
			let edge_length_limit = edge_length_limit * 0.005;

			let mut result = Vector {
				style: source.style.clone(),
				..Default::default()
			};

			// Grab the initial point ID as a stable starting point
			let mut initial_point_id = source.point_domain.ids().first().copied().unwrap_or(PointId::generate());

			for mut bezpath in source.stroke_bezpath_iter() {
				bezpath.apply_affine(Affine::new(source_transform.to_cols_array()));
				let (manipulator_groups, is_closed) = bezpath_to_manipulator_groups(&bezpath);

				// End if not enough points for corner rounding
				if manipulator_groups.len() < 3 {
					result.append_bezpath(bezpath);
					continue;
				}

				let mut new_manipulator_groups = Vec::new();

				for i in 0..manipulator_groups.len() {
					// Skip first and last points for open paths
					if !is_closed && (i == 0 || i == manipulator_groups.len() - 1) {
						new_manipulator_groups.push(manipulator_groups[i]);
						continue;
					}

					// Not the prettiest, but it makes the rest of the logic more readable
					let prev_idx = if i == 0 { if is_closed { manipulator_groups.len() - 1 } else { 0 } } else { i - 1 };
					let curr_idx = i;
					let next_idx = if i == manipulator_groups.len() - 1 { if is_closed { 0 } else { i } } else { i + 1 };

					let prev = manipulator_groups[prev_idx].anchor;
					let curr = manipulator_groups[curr_idx].anchor;
					let next = manipulator_groups[next_idx].anchor;

					let dir1 = (curr - prev).normalize_or(DVec2::X);
					let dir2 = (next - curr).normalize_or(DVec2::X);

					let theta = PI - dir1.angle_to(dir2).abs();

					// Skip near-straight corners
					if theta > PI - min_angle_threshold.to_radians() {
						new_manipulator_groups.push(manipulator_groups[curr_idx]);
						continue;
					}

					// Calculate L, with limits to avoid extreme values
					let distance_along_edge = radius / (theta / 2.).sin();
					let distance_along_edge = distance_along_edge.min(edge_length_limit * (curr - prev).length().min((next - curr).length())).max(0.01);

					// Find points on each edge at distance L from corner
					let p1 = curr - dir1 * distance_along_edge;
					let p2 = curr + dir2 * distance_along_edge;

					// Add first point (coming into the rounded corner)
					new_manipulator_groups.push(ManipulatorGroup {
						anchor: p1,
						in_handle: None,
						out_handle: Some(curr - dir1 * distance_along_edge * roundness),
						id: initial_point_id.next_id(),
					});

					// Add second point (coming out of the rounded corner)
					new_manipulator_groups.push(ManipulatorGroup {
						anchor: p2,
						in_handle: Some(curr + dir2 * distance_along_edge * roundness),
						out_handle: None,
						id: initial_point_id.next_id(),
					});
				}

				// One subpath for each shape
				let mut rounded_subpath = bezpath_from_manipulator_groups(&new_manipulator_groups, is_closed);
				rounded_subpath.apply_affine(Affine::new(source_transform_inverse.to_cols_array()));
				result.append_bezpath(rounded_subpath);
			}

			result.upstream_nested_layers = upstream_nested_layers;

			TableRow {
				element: result,
				transform: source_transform,
				alpha_blending: Default::default(),
				source_node_id: *source_node_id,
			}
		})
		.collect()
}

#[node_macro::node(name("Merge by Distance"), category("Vector: Modifier"), path(graphene_core::vector))]
pub fn merge_by_distance(
	_: impl Ctx,
	content: Table<Vector>,
	#[default(0.1)]
	#[hard_min(0.0001)]
	distance: PixelLength,
	algorithm: MergeByDistanceAlgorithm,
) -> Table<Vector> {
	match algorithm {
		MergeByDistanceAlgorithm::Spatial => content
			.into_iter()
			.map(|mut row| {
				row.element.merge_by_distance_spatial(row.transform, distance);
				row
			})
			.collect(),
		MergeByDistanceAlgorithm::Topological => content
			.into_iter()
			.map(|mut row| {
				row.element.merge_by_distance_topological(distance);
				row
			})
			.collect(),
	}
}

#[node_macro::node(category("Vector: Modifier"), path(graphene_core::vector))]
async fn box_warp(_: impl Ctx, content: Table<Vector>, #[expose] rectangle: Table<Vector>) -> Table<Vector> {
	let Some((target, target_transform)) = rectangle.get(0).map(|rect| (rect.element, rect.transform)) else {
		return content;
	};

	content
		.into_iter()
		.map(|mut row| {
			let transform = row.transform;
			let vector = row.element;

			// Get the bounding box of the source vector geometry
			let source_bbox = vector.bounding_box_with_transform(transform).unwrap_or([DVec2::ZERO, DVec2::ONE]);

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
			let mut result = vector.clone();

			// Precompute source bounding box size for normalization
			let source_size = source_bbox[1] - source_bbox[0];

			// Transform points
			for (_, position) in result.point_domain.positions_mut() {
				// Get the point in world space
				let world_pos = transform.transform_point2(*position);

				// Normalize coordinates within the source bounding box
				let t = ((world_pos - source_bbox[0]) / source_size).clamp(DVec2::ZERO, DVec2::ONE);

				// Apply bilinear interpolation
				*position = bilinear_interpolate(t, &dst_corners);
			}

			// Transform handles in bezier curves
			for (_, handles, _, _) in result.handles_mut() {
				*handles = handles.apply_transformation(|pos| {
					// Get the handle in world space
					let world_pos = transform.transform_point2(pos);

					// Normalize coordinates within the source bounding box
					let t = ((world_pos - source_bbox[0]) / source_size).clamp(DVec2::ZERO, DVec2::ONE);

					// Apply bilinear interpolation
					bilinear_interpolate(t, &dst_corners)
				});
			}

			result.style.set_stroke_transform(DAffine2::IDENTITY);

			// Add this to the table and reset the transform since we've applied it directly to the points
			row.element = result;
			row.transform = DAffine2::IDENTITY;
			row
		})
		.collect()
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
	source: Table<Vector>,
	/// The amount of spread for the auto-tangents, from 0 (sharp corner) to 1 (full spread).
	#[default(0.5)]
	// TODO: Make this a soft range to allow any value to be typed in outside the slider range of 0 to 1
	#[range((0., 1.))]
	spread: f64,
	/// If active, existing non-zero handles won't be affected.
	#[default(true)]
	preserve_existing: bool,
) -> Table<Vector> {
	source
		.iter()
		.map(|source| {
			let transform = *source.transform;
			let alpha_blending = *source.alpha_blending;
			let source_node_id = *source.source_node_id;
			let source = source.element;

			let mut result = Vector {
				style: source.style.clone(),
				..Default::default()
			};

			for mut subpath in source.stroke_bezier_paths() {
				subpath.apply_transform(transform);

				let manipulators_list = subpath.manipulator_groups();
				if manipulators_list.len() < 2 {
					// Not enough points for softening or handle removal
					result.append_subpath(subpath, true);
					continue;
				}

				let mut new_manipulators_list = Vec::with_capacity(manipulators_list.len());
				let is_closed = subpath.closed();

				for i in 0..manipulators_list.len() {
					let curr = &manipulators_list[i];

					if preserve_existing {
						// Check if this point has handles that are meaningfully different from the anchor
						let has_handles = (curr.in_handle.is_some() && !curr.in_handle.unwrap().abs_diff_eq(curr.anchor, 1e-5))
							|| (curr.out_handle.is_some() && !curr.out_handle.unwrap().abs_diff_eq(curr.anchor, 1e-5));

						// If the point already has handles, or if it's an endpoint of an open path, keep it as is.
						if has_handles || (!is_closed && (i == 0 || i == manipulators_list.len() - 1)) {
							new_manipulators_list.push(*curr);
							continue;
						}
					}

					// If spread is 0, remove handles for this point, making it a sharp corner.
					if spread == 0. {
						new_manipulators_list.push(ManipulatorGroup {
							anchor: curr.anchor,
							in_handle: None,
							out_handle: None,
							id: curr.id,
						});
						continue;
					}

					// Get previous and next points for auto-tangent calculation
					let prev_idx = if i == 0 { if is_closed { manipulators_list.len() - 1 } else { i } } else { i - 1 };
					let next_idx = if i == manipulators_list.len() - 1 { if is_closed { 0 } else { i } } else { i + 1 };

					let prev = manipulators_list[prev_idx].anchor;
					let curr_pos = curr.anchor;
					let next = manipulators_list[next_idx].anchor;

					// Calculate directions from current point to adjacent points
					let dir_prev = (prev - curr_pos).normalize_or_zero();
					let dir_next = (next - curr_pos).normalize_or_zero();

					// Check if we have valid directions (e.g., points are not coincident)
					if dir_prev.length_squared() < 1e-5 || dir_next.length_squared() < 1e-5 {
						// Fallback: keep the original manipulator group (which has no active handles here)
						new_manipulators_list.push(*curr);
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
					new_manipulators_list.push(ManipulatorGroup {
						anchor: curr_pos,
						in_handle: Some(curr_pos + handle_dir * in_length),
						out_handle: Some(curr_pos - handle_dir * out_length),
						id: curr.id,
					});
				}

				let mut softened_bezpath = bezpath_from_manipulator_groups(&new_manipulators_list, is_closed);
				softened_bezpath.apply_affine(Affine::new(transform.inverse().to_cols_array()));
				result.append_bezpath(softened_bezpath);
			}

			TableRow {
				element: result,
				transform,
				alpha_blending,
				source_node_id,
			}
		})
		.collect()
}

#[node_macro::node(category("Vector: Modifier"), path(graphene_core::vector))]
async fn bounding_box(_: impl Ctx, content: Table<Vector>) -> Table<Vector> {
	content
		.into_iter()
		.map(|mut row| {
			let vector = row.element;

			let mut result = vector
				.bounding_box_rect()
				.map(|bbox| {
					let mut vector = Vector::default();
					vector.append_bezpath(bbox.to_path(DEFAULT_ACCURACY));
					vector
				})
				.unwrap_or_default();

			result.style = vector.style.clone();
			result.style.set_stroke_transform(DAffine2::IDENTITY);

			row.element = result;
			row
		})
		.collect()
}

#[node_macro::node(category("Vector: Measure"), path(graphene_core::vector))]
async fn dimensions(_: impl Ctx, content: Table<Vector>) -> DVec2 {
	content
		.iter()
		.filter_map(|vector| vector.element.bounding_box_with_transform(*vector.transform))
		.reduce(|[acc_top_left, acc_bottom_right], [top_left, bottom_right]| [acc_top_left.min(top_left), acc_bottom_right.max(bottom_right)])
		.map(|[top_left, bottom_right]| bottom_right - top_left)
		.unwrap_or_default()
}

/// Converts a vec2 value into a vector path composed of a single anchor point.
///
/// This is useful in conjunction with nodes that repeat it, followed by the "Points to Polyline" node to string together a path of the points.
#[node_macro::node(category("Vector"), name("Vec2 to Point"), path(graphene_core::vector))]
async fn vec2_to_point(_: impl Ctx, vec2: DVec2) -> Table<Vector> {
	let mut point_domain = PointDomain::new();
	point_domain.push(PointId::generate(), vec2);

	Table::new_from_row(TableRow {
		element: Vector { point_domain, ..Default::default() },
		..Default::default()
	})
}

/// Creates a polyline from a series of vector points, replacing any existing segments and regions that may already exist.
#[node_macro::node(category("Vector"), name("Points to Polyline"), path(graphene_core::vector))]
async fn points_to_polyline(_: impl Ctx, mut points: Table<Vector>, #[default(true)] closed: bool) -> Table<Vector> {
	for row in points.iter_mut() {
		let mut segment_domain = SegmentDomain::new();

		let points_count = row.element.point_domain.ids().len();

		if points_count > 2 {
			(0..points_count - 1).for_each(|i| {
				segment_domain.push(SegmentId::generate(), i, i + 1, BezierHandles::Linear, StrokeId::generate());
			});

			if closed {
				segment_domain.push(SegmentId::generate(), points_count - 1, 0, BezierHandles::Linear, StrokeId::generate());

				row.element
					.region_domain
					.push(RegionId::generate(), segment_domain.ids()[0]..=*segment_domain.ids().last().unwrap(), FillId::generate());
			}
		}

		row.element.segment_domain = segment_domain;
	}

	points
}

#[node_macro::node(category("Vector: Modifier"), path(graphene_core::vector), properties("offset_path_properties"))]
async fn offset_path(_: impl Ctx, content: Table<Vector>, distance: f64, join: StrokeJoin, #[default(4.)] miter_limit: f64) -> Table<Vector> {
	content
		.into_iter()
		.map(|mut row| {
			let transform = Affine::new(row.transform.to_cols_array());
			let vector = row.element;

			let bezpaths = vector.stroke_bezpath_iter();
			let mut result = Vector {
				style: vector.style.clone(),
				..Default::default()
			};
			result.style.set_stroke_transform(DAffine2::IDENTITY);

			// Perform operation on all subpaths in this shape.
			for mut bezpath in bezpaths {
				bezpath.apply_affine(transform);

				// Taking the existing stroke data and passing it to Kurbo to generate new paths.
				let mut bezpath_out = offset_bezpath(
					&bezpath,
					-distance,
					match join {
						StrokeJoin::Miter => kurbo::Join::Miter,
						StrokeJoin::Bevel => kurbo::Join::Bevel,
						StrokeJoin::Round => kurbo::Join::Round,
					},
					Some(miter_limit),
				);

				bezpath_out.apply_affine(transform.inverse());

				// One closed subpath, open path.
				result.append_bezpath(bezpath_out);
			}

			row.element = result;
			row
		})
		.collect()
}

#[node_macro::node(category("Vector: Modifier"), path(graphene_core::vector))]
async fn solidify_stroke(_: impl Ctx, content: Table<Vector>) -> Table<Vector> {
	content
		.into_iter()
		.map(|mut row| {
			let vector = row.element;

			let stroke = vector.style.stroke().clone().unwrap_or_default();
			let bezpaths = vector.stroke_bezpath_iter();
			let mut result = Vector::default();

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
			if let Some(stroke) = vector.style.stroke() {
				result.style.set_fill(Fill::solid_or_none(stroke.color));
				result.style.set_stroke(Stroke::default());
			}

			row.element = result;
			row
		})
		.collect()
}

#[node_macro::node(category("Vector: Modifier"), path(graphene_core::vector))]
async fn separate_subpaths(_: impl Ctx, content: Table<Vector>) -> Table<Vector> {
	content
		.into_iter()
		.flat_map(|row| {
			let style = row.element.style.clone();
			let transform = row.transform;
			let alpha_blending = row.alpha_blending;
			let source_node_id = row.source_node_id;

			row.element
				.stroke_bezpath_iter()
				.map(move |bezpath| {
					let mut vector = Vector::default();
					vector.append_bezpath(bezpath);
					vector.style = style.clone();

					TableRow {
						element: vector,
						transform,
						alpha_blending,
						source_node_id,
					}
				})
				.collect::<Vec<TableRow<Vector>>>()
		})
		.collect()
}

#[node_macro::node(category("Vector"), path(graphene_core::vector))]
async fn flatten_path<I: 'n + Send>(_: impl Ctx, #[implementations(Table<Graphic>, Table<Vector>)] content: Table<I>) -> Table<Vector>
where
	Graphic: From<Table<I>>,
{
	// NOTE(AdamGerhant):
	// A node-based solution to support passing through vector data could be a network node with a cache node
	// connected to a Flatten Path connected to an if else node, another connection from the cache directly to
	// the if else node, and another connection from the cache to a matches type node connected to the if else node.

	fn flatten_table(output: &mut TableRowMut<Vector>, graphic_table: &Table<Graphic>) {
		for (current_index, current_element) in graphic_table.iter().enumerate() {
			match current_element.element {
				Graphic::Vector(vector) => {
					// Loop through every row of the `Table<Vector>` and concatenate each element's subpath into the output `Vector` element.
					for (vector_index, row) in vector.iter().enumerate() {
						let other = row.element;
						let transform = *current_element.transform * *row.transform;
						let node_id = current_element.source_node_id.map(|node_id| node_id.0).unwrap_or_default();

						let mut hasher = DefaultHasher::new();
						(current_index, vector_index, node_id).hash(&mut hasher);
						let collision_hash_seed = hasher.finish();

						output.element.concat(other, transform, collision_hash_seed);

						// Use the last encountered style as the output style
						output.element.style = row.element.style.clone();
					}
				}
				Graphic::Graphic(graphic) => {
					let mut graphic = graphic.clone();
					for row in graphic.iter_mut() {
						*row.transform = *current_element.transform * *row.transform;
					}

					flatten_table(output, &graphic);
				}
				_ => {}
			}
		}
	}

	// Create a table with one empty `Vector` element, then get a mutable reference to it which we append flattened subpaths to
	let mut output_table = Table::new_from_element(Vector::default());
	let Some(mut output) = output_table.iter_mut().next() else { return output_table };

	// Flatten the graphic input into the output `Vector` element
	let base_graphic_table = Table::new_from_element(Graphic::from(content));
	flatten_table(&mut output, &base_graphic_table);

	// Return the single-row Table<Vector> containing the flattened Vector subpaths
	output_table
}

/// Convert vector geometry into a polyline composed of evenly spaced points.
#[node_macro::node(category(""), path(graphene_core::vector))]
async fn sample_polyline(
	_: impl Ctx,
	content: Table<Vector>,
	spacing: PointSpacingType,
	#[unit(" px")] separation: f64,
	quantity: u32,
	#[unit(" px")] start_offset: f64,
	#[unit(" px")] stop_offset: f64,
	adaptive_spacing: bool,
	subpath_segment_lengths: Vec<f64>,
) -> Table<Vector> {
	content
		.into_iter()
		.map(|mut row| {
			let mut result = Vector {
				point_domain: Default::default(),
				segment_domain: Default::default(),
				region_domain: Default::default(),
				colinear_manipulators: Default::default(),
				style: std::mem::take(&mut row.element.style),
				upstream_nested_layers: std::mem::take(&mut row.element.upstream_nested_layers),
			};
			// Transfer the stroke transform from the input vector content to the result.
			result.style.set_stroke_transform(row.transform);

			// Using `stroke_bezpath_iter` so that the `subpath_segment_lengths` is aligned to the segments of each bezpath.
			// So we can index into `subpath_segment_lengths` to get the length of the segments.
			// NOTE: `subpath_segment_lengths` has precalulated lengths with transformation applied.
			let bezpaths = row.element.stroke_bezpath_iter();

			// Keeps track of the index of the first segment of the next bezpath in order to get lengths of all segments.
			let mut next_segment_index = 0;

			for mut bezpath in bezpaths {
				// Apply the tranformation to the current bezpath to calculate points after transformation.
				bezpath.apply_affine(Affine::new(row.transform.to_cols_array()));

				let segment_count = bezpath.segments().count();

				// For the current bezpath we get its segment's length by calculating the start index and end index.
				let current_bezpath_segments_length = &subpath_segment_lengths[next_segment_index..next_segment_index + segment_count];

				// Increment the segment index by the number of segments in the current bezpath to calculate the next bezpath segment's length.
				next_segment_index += segment_count;

				let amount = match spacing {
					PointSpacingType::Separation => separation,
					PointSpacingType::Quantity => quantity as f64,
				};
				let Some(mut sample_bezpath) = sample_polyline_on_bezpath(bezpath, spacing, amount, start_offset, stop_offset, adaptive_spacing, current_bezpath_segments_length) else {
					continue;
				};

				// Reverse the transformation applied to the bezpath as the `result` already has the transformation set.
				sample_bezpath.apply_affine(Affine::new(row.transform.to_cols_array()).inverse());

				// Append the bezpath (subpath) that connects generated points by lines.
				result.append_bezpath(sample_bezpath);
			}

			row.element = result;
			row
		})
		.collect()
}

/// Cuts a path at a given progress from 0 to 1 along the path, creating two new subpaths from the original one (if the path is initially open) or one open subpath (if the path is initially closed).
///
/// If multiple subpaths make up the path, the whole number part of the progress value selects the subpath and the decimal part determines the position along it.
#[node_macro::node(category("Vector: Modifier"), path(graphene_core::vector))]
async fn cut_path(_: impl Ctx, mut content: Table<Vector>, progress: Fraction, parameterized_distance: bool, reverse: bool) -> Table<Vector> {
	let euclidian = !parameterized_distance;

	let bezpaths = content
		.iter()
		.enumerate()
		.flat_map(|(row_index, vector)| vector.element.stroke_bezpath_iter().map(|bezpath| (row_index, bezpath)).collect::<Vec<_>>())
		.collect::<Vec<_>>();

	let bezpath_count = bezpaths.len() as f64;
	let t_value = progress.clamp(0., bezpath_count);
	let t_value = if reverse { bezpath_count - t_value } else { t_value };
	let index = if t_value >= bezpath_count { (bezpath_count - 1.) as usize } else { t_value as usize };

	if let Some((row_index, bezpath)) = bezpaths.get(index).cloned() {
		let mut result_vector = Vector {
			style: content.get(row_index).unwrap().element.style.clone(),
			..Default::default()
		};

		for (_, (_, bezpath)) in bezpaths.iter().enumerate().filter(|(i, (ri, _))| *i != index && *ri == row_index) {
			result_vector.append_bezpath(bezpath.clone());
		}
		let t = if t_value == bezpath_count { 1. } else { t_value.fract() };
		let t = if euclidian { TValue::Euclidean(t) } else { TValue::Parametric(t) };

		if let Some((first, second)) = split_bezpath(&bezpath, t) {
			result_vector.append_bezpath(first);
			result_vector.append_bezpath(second);
		} else {
			result_vector.append_bezpath(bezpath);
		}

		*content.get_mut(row_index).unwrap().element = result_vector;
	}

	content
}

/// Cuts path segments into separate disconnected pieces where each is a distinct subpath.
#[node_macro::node(category("Vector: Modifier"), path(graphene_core::vector))]
async fn cut_segments(_: impl Ctx, mut content: Table<Vector>) -> Table<Vector> {
	// Iterate through every segment and make a copy of each of its endpoints, then reassign each segment's endpoints to its own unique point copy
	for row in content.iter_mut() {
		let points_count = row.element.point_domain.ids().len();
		let segments_count = row.element.segment_domain.ids().len();

		let mut point_usages = vec![0_usize; points_count];

		// Count how many times each point is used as an endpoint of the segments
		let start_points = row.element.segment_domain.start_point().iter();
		let end_points = row.element.segment_domain.end_point().iter();
		for (&start, &end) in start_points.zip(end_points) {
			point_usages[start] += 1;
			point_usages[end] += 1;
		}

		let mut new_points = PointDomain::new();
		let mut offset_sum: usize = 0;
		let mut points_with_new_offsets = Vec::with_capacity(points_count);

		// Build a new point domain with the original points, but with duplications based on their extra usages by the segments
		for (index, (point_id, point)) in row.element.point_domain.iter().enumerate() {
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
		row.element.point_domain = new_points;
		for original_segment_index in 0..segments_count {
			let original_point_start_index = row.element.segment_domain.start_point()[original_segment_index];
			let original_point_end_index = row.element.segment_domain.end_point()[original_segment_index];

			point_usages[original_point_start_index] -= 1;
			point_usages[original_point_end_index] -= 1;

			let start_usage = points_with_new_offsets[original_point_start_index] + point_usages[original_point_start_index];
			let end_usage = points_with_new_offsets[original_point_end_index] + point_usages[original_point_end_index];

			row.element.segment_domain.set_start_point(original_segment_index, start_usage);
			row.element.segment_domain.set_end_point(original_segment_index, end_usage);
		}
	}

	content
}

/// Determines the position of a point on the path, given by its progress from 0 to 1 along the path.
///
/// If multiple subpaths make up the path, the whole number part of the progress value selects the subpath and the decimal part determines the position along it.
#[node_macro::node(name("Position on Path"), category("Vector: Measure"), path(graphene_core::vector))]
async fn position_on_path(
	_: impl Ctx,
	/// The path to traverse.
	content: Table<Vector>,
	/// The factor from the start to the end of the path, 0–1 for one subpath, 1–2 for a second subpath, and so on.
	progress: Fraction,
	/// Swap the direction of the path.
	reverse: bool,
	/// Traverse the path using each segment's Bézier curve parameterization instead of the Euclidean distance. Faster to compute but doesn't respect actual distances.
	parameterized_distance: bool,
) -> DVec2 {
	let euclidian = !parameterized_distance;

	let mut bezpaths = content
		.iter()
		.flat_map(|vector| {
			let transform = *vector.transform;
			vector.element.stroke_bezpath_iter().map(move |bezpath| (bezpath, transform))
		})
		.collect::<Vec<_>>();
	let bezpath_count = bezpaths.len() as f64;
	let progress = progress.clamp(0., bezpath_count);
	let progress = if reverse { bezpath_count - progress } else { progress };
	let index = if progress >= bezpath_count { (bezpath_count - 1.) as usize } else { progress as usize };

	bezpaths.get_mut(index).map_or(DVec2::ZERO, |(bezpath, transform)| {
		let t = if progress == bezpath_count { 1. } else { progress.fract() };
		let t = if euclidian { TValue::Euclidean(t) } else { TValue::Parametric(t) };

		bezpath.apply_affine(Affine::new(transform.to_cols_array()));

		point_to_dvec2(evaluate_bezpath(bezpath, t, None))
	})
}

/// Determines the angle of the tangent at a point on the path, given by its progress from 0 to 1 along the path.
///
/// If multiple subpaths make up the path, the whole number part of the progress value selects the subpath and the decimal part determines the position along it.
#[node_macro::node(name("Tangent on Path"), category("Vector: Measure"), path(graphene_core::vector))]
async fn tangent_on_path(
	_: impl Ctx,
	/// The path to traverse.
	content: Table<Vector>,
	/// The factor from the start to the end of the path, 0–1 for one subpath, 1–2 for a second subpath, and so on.
	progress: Fraction,
	/// Swap the direction of the path.
	reverse: bool,
	/// Traverse the path using each segment's Bézier curve parameterization instead of the Euclidean distance. Faster to compute but doesn't respect actual distances.
	parameterized_distance: bool,
	/// Whether the resulting angle should be given in as radians instead of degrees.
	radians: bool,
) -> f64 {
	let euclidian = !parameterized_distance;

	let mut bezpaths = content
		.iter()
		.flat_map(|vector| {
			let transform = *vector.transform;
			vector.element.stroke_bezpath_iter().map(move |bezpath| (bezpath, transform))
		})
		.collect::<Vec<_>>();
	let bezpath_count = bezpaths.len() as f64;
	let progress = progress.clamp(0., bezpath_count);
	let progress = if reverse { bezpath_count - progress } else { progress };
	let index = if progress >= bezpath_count { (bezpath_count - 1.) as usize } else { progress as usize };

	let angle = bezpaths.get_mut(index).map_or(0., |(bezpath, transform)| {
		let t = if progress == bezpath_count { 1. } else { progress.fract() };
		let t_value = |t: f64| if euclidian { TValue::Euclidean(t) } else { TValue::Parametric(t) };

		bezpath.apply_affine(Affine::new(transform.to_cols_array()));

		let mut tangent = point_to_dvec2(tangent_on_bezpath(bezpath, t_value(t), None));
		if tangent == DVec2::ZERO {
			let t = t + if t > 0.5 { -0.001 } else { 0.001 };
			tangent = point_to_dvec2(tangent_on_bezpath(bezpath, t_value(t), None));
		}
		if tangent == DVec2::ZERO {
			return 0.;
		}

		-tangent.angle_to(if reverse { -DVec2::X } else { DVec2::X })
	});

	if radians { angle } else { angle.to_degrees() }
}

#[node_macro::node(category(""), path(graphene_core::vector))]
async fn poisson_disk_points(
	_: impl Ctx,
	content: Table<Vector>,
	#[unit(" px")]
	#[default(10.)]
	#[hard_min(0.01)]
	separation_disk_diameter: f64,
	seed: SeedValue,
) -> Table<Vector> {
	let mut rng = rand::rngs::StdRng::seed_from_u64(seed.into());

	content
		.into_iter()
		.map(|mut row| {
			let mut result = Vector::default();

			let path_with_bounding_boxes: Vec<_> = row
				.element
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

			// Transfer the style from the input vector content to the result.
			result.style = row.element.style.clone();
			result.style.set_stroke_transform(DAffine2::IDENTITY);

			row.element = result;
			row
		})
		.collect()
}

#[node_macro::node(category(""), path(graphene_core::vector))]
async fn subpath_segment_lengths(_: impl Ctx, content: Table<Vector>) -> Vec<f64> {
	let pathseg_perimeter = |segment: PathSeg| {
		if is_linear(segment) {
			Line::new(segment.start(), segment.end()).perimeter(DEFAULT_ACCURACY)
		} else {
			segment.perimeter(DEFAULT_ACCURACY)
		}
	};

	content
		.into_iter()
		.flat_map(|vector| {
			let transform = vector.transform;
			vector
				.element
				.stroke_bezpath_iter()
				.flat_map(|mut bezpath| {
					bezpath.apply_affine(Affine::new(transform.to_cols_array()));
					bezpath.segments().map(pathseg_perimeter).collect::<Vec<f64>>()
				})
				.collect::<Vec<f64>>()
		})
		.collect()
}

#[node_macro::node(name("Spline"), category("Vector: Modifier"), path(graphene_core::vector))]
async fn spline(_: impl Ctx, content: Table<Vector>) -> Table<Vector> {
	content
		.into_iter()
		.filter_map(|mut row| {
			// Exit early if there are no points to generate splines from.
			if row.element.point_domain.positions().is_empty() {
				return None;
			}

			let mut segment_domain = SegmentDomain::default();
			for (manipulator_groups, closed) in row.element.stroke_manipulator_groups() {
				let positions = manipulator_groups.iter().map(|manipulators| manipulators.anchor).collect::<Vec<_>>();
				let closed = closed && positions.len() > 2;

				// Compute control point handles for Bezier spline.
				let first_handles = if closed {
					solve_spline_first_handle_closed(&positions)
				} else {
					solve_spline_first_handle_open(&positions)
				};

				let stroke_id = StrokeId::ZERO;

				// Create segments with computed Bezier handles and add them to the output vector element's segment domain.
				for i in 0..(positions.len() - if closed { 0 } else { 1 }) {
					let next_index = (i + 1) % positions.len();

					let start_index = row.element.point_domain.resolve_id(manipulator_groups[i].id).unwrap();
					let end_index = row.element.point_domain.resolve_id(manipulator_groups[next_index].id).unwrap();

					let handle_start = first_handles[i];
					let handle_end = positions[next_index] * 2. - first_handles[next_index];
					let handles = BezierHandles::Cubic { handle_start, handle_end };

					segment_domain.push(SegmentId::generate(), start_index, end_index, handles, stroke_id);
				}
			}

			row.element.segment_domain = segment_domain;
			Some(row)
		})
		.collect()
}

#[node_macro::node(category("Vector: Modifier"), path(graphene_core::vector))]
async fn jitter_points(
	_: impl Ctx,
	content: Table<Vector>,
	#[unit(" px")]
	#[default(5.)]
	amount: f64,
	seed: SeedValue,
) -> Table<Vector> {
	content
		.into_iter()
		.map(|mut row| {
			let mut rng = rand::rngs::StdRng::seed_from_u64(seed.into());

			let transform = row.transform;
			let inverse_transform = if transform.matrix2.determinant() != 0. { transform.inverse() } else { Default::default() };

			let deltas = (0..row.element.point_domain.positions().len())
				.map(|_| {
					let angle = rng.random::<f64>() * TAU;

					inverse_transform.transform_vector2(DVec2::from_angle(angle) * rng.random::<f64>() * amount)
				})
				.collect::<Vec<_>>();
			let mut already_applied = vec![false; row.element.point_domain.positions().len()];

			for (handles, start, end) in row.element.segment_domain.handles_and_points_mut() {
				let start_delta = deltas[*start];
				let end_delta = deltas[*end];

				if !already_applied[*start] {
					let start_position = row.element.point_domain.positions()[*start];
					row.element.point_domain.set_position(*start, start_position + start_delta);
					already_applied[*start] = true;
				}
				if !already_applied[*end] {
					let end_position = row.element.point_domain.positions()[*end];
					row.element.point_domain.set_position(*end, end_position + end_delta);
					already_applied[*end] = true;
				}

				match handles {
					BezierHandles::Cubic { handle_start, handle_end } => {
						*handle_start += start_delta;
						*handle_end += end_delta;
					}
					BezierHandles::Quadratic { handle } => {
						*handle = row.transform.transform_point2(*handle) + (start_delta + end_delta) / 2.;
					}
					BezierHandles::Linear => {}
				}
			}

			row.element.style.set_stroke_transform(DAffine2::IDENTITY);
			row
		})
		.collect()
}

#[node_macro::node(category("Vector: Modifier"), path(graphene_core::vector))]
async fn morph(_: impl Ctx, source: Table<Vector>, #[expose] target: Table<Vector>, #[default(0.5)] time: Fraction) -> Table<Vector> {
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

	source
		.into_iter()
		.zip(target.into_iter())
		.map(|(source_row, target_row)| {
			let mut vector = Vector::default();

			// Lerp styles
			let vector_alpha_blending = source_row.alpha_blending.lerp(&target_row.alpha_blending, time as f32);
			vector.style = source_row.element.style.lerp(&target_row.element.style, time);

			// Before and after transforms
			let source_transform = source_row.transform;
			let target_transform = target_row.transform;

			// Before and after paths
			let source_bezpaths = source_row.element.stroke_bezpath_iter();
			let target_bezpaths = target_row.element.stroke_bezpath_iter();

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

				vector.append_bezpath(source_bezpath.clone());
			}

			// Deal with unmatched extra paths by collapsing them
			let source_paths_count = source_row.element.stroke_bezpath_iter().count();
			let target_paths_count = target_row.element.stroke_bezpath_iter().count();
			let source_paths = source_row.element.stroke_bezpath_iter().skip(target_paths_count);
			let target_paths = target_row.element.stroke_bezpath_iter().skip(source_paths_count);

			for mut source_path in source_paths {
				source_path.apply_affine(Affine::new(source_transform.to_cols_array()));

				// Skip if the path has no segments else get the point at the end of the path.
				let Some(end) = source_path.segments().last().map(|element| element.end()) else { continue };

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
				vector.append_bezpath(source_path);
			}

			for mut target_path in target_paths {
				target_path.apply_affine(Affine::new(source_transform.to_cols_array()));

				// Skip if the path has no segments else get the point at the start of the path.
				let Some(start) = target_path.segments().next().map(|element| element.start()) else { continue };

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
				vector.append_bezpath(target_path);
			}

			TableRow {
				element: vector,
				alpha_blending: vector_alpha_blending,
				..Default::default()
			}
		})
		.collect()
}

fn bevel_algorithm(mut vector: Vector, transform: DAffine2, distance: f64) -> Vector {
	// Splits a bézier curve based on a distance measurement
	fn split_distance(bezier: PathSeg, distance: f64, length: f64) -> PathSeg {
		let parametric = eval_pathseg_euclidean(bezier, (distance / length).clamp(0., 1.), DEFAULT_ACCURACY);
		bezier.subsegment(parametric..1.)
	}

	/// Produces a list that corresponds with the point ID. The value is how many segments are connected.
	fn segments_connected_count(vector: &Vector) -> Vec<usize> {
		// Count the number of segments connecting to each point.
		let mut segments_connected_count = vec![0; vector.point_domain.ids().len()];
		for &point_index in vector.segment_domain.start_point().iter().chain(vector.segment_domain.end_point()) {
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

	fn calculate_distance_to_spilt(bezier1: PathSeg, bezier2: PathSeg, bevel_length: f64) -> f64 {
		if is_linear(bezier1) && is_linear(bezier2) {
			let v1 = (bezier1.end() - bezier1.start()).normalize();
			let v2 = (bezier1.end() - bezier2.end()).normalize();

			let dot_product = v1.dot(v2);
			let angle_rad = dot_product.acos();

			return bevel_length / (2. * (angle_rad / 2.).sin());
		}

		let length1 = bezier1.perimeter(DEFAULT_ACCURACY);
		let length2 = bezier2.perimeter(DEFAULT_ACCURACY);

		let max_split = length1.min(length2);

		let mut split_distance = 0.;
		let mut best_diff = f64::MAX;
		let mut current_best_distance = 0.;

		let clamp_and_round = |value: f64| ((value * 1000.).round() / 1000.).clamp(0., 1.);

		const INITIAL_SAMPLES: usize = 50;
		for i in 0..=INITIAL_SAMPLES {
			let distance_sample = max_split * (i as f64 / INITIAL_SAMPLES as f64);

			let x_point_t = eval_pathseg_euclidean(bezier1, 1. - clamp_and_round(distance_sample / length1), DEFAULT_ACCURACY);
			let y_point_t = eval_pathseg_euclidean(bezier2, clamp_and_round(distance_sample / length2), DEFAULT_ACCURACY);

			let x_point = bezier1.eval(x_point_t);
			let y_point = bezier2.eval(y_point_t);

			let distance = x_point.distance(y_point);
			let diff = (bevel_length - distance).abs();

			if diff < best_diff {
				best_diff = diff;
				current_best_distance = distance_sample;
			}

			if bevel_length - distance < 0. {
				split_distance = distance_sample;

				if i > 0 {
					let prev_sample = max_split * ((i - 1) as f64 / INITIAL_SAMPLES as f64);

					const REFINE_STEPS: usize = 10;
					for j in 1..=REFINE_STEPS {
						let refined_sample = prev_sample + (distance_sample - prev_sample) * (j as f64 / REFINE_STEPS as f64);

						let x_point_t = eval_pathseg_euclidean(bezier1, 1. - (refined_sample / length1).clamp(0., 1.), DEFAULT_ACCURACY);
						let y_point_t = eval_pathseg_euclidean(bezier2, (refined_sample / length2).clamp(0., 1.), DEFAULT_ACCURACY);

						let x_point = bezier1.eval(x_point_t);
						let y_point = bezier2.eval(y_point_t);

						let distance = x_point.distance(y_point);

						if bevel_length - distance < 0. {
							split_distance = refined_sample;
							break;
						}
					}
				}
				break;
			}
		}

		if split_distance == 0. && current_best_distance > 0. {
			split_distance = current_best_distance;
		}

		split_distance
	}

	fn sort_segments(segment_domain: &SegmentDomain) -> Vec<usize> {
		let start_points = segment_domain.start_point();
		let end_points = segment_domain.end_point();

		let mut sorted_segments = vec![0];
		let segment_domain_length = segment_domain.ids().len();

		for _ in 0..segment_domain_length {
			match sorted_segments.last() {
				Some(&last) => {
					if let Some(index) = start_points.iter().position(|&p| p == end_points[last]) {
						if index == 0 {
							break;
						}
						sorted_segments.push(index);
					}
				}
				None => break,
			}
		}

		if segment_domain_length != sorted_segments.len() {
			for i in 0..segment_domain_length {
				if !sorted_segments.contains(&i) {
					sorted_segments.push(i);
				}
			}
		}

		sorted_segments
	}

	fn update_existing_segments(vector: &mut Vector, transform: DAffine2, distance: f64, segments_connected: &mut [usize]) -> Vec<[usize; 2]> {
		let mut next_id = vector.point_domain.next_id();
		let mut new_segments = Vec::new();

		let sorted_segments = sort_segments(&vector.segment_domain);
		let segment_domain = &mut vector.segment_domain;
		let segment_domain_length = segment_domain.ids().len();

		let mut first_original_length = 0.;
		let mut first_length = 0.;
		let mut prev_original_length = 0.;
		let mut prev_length = 0.;

		for i in 0..segment_domain_length {
			let (index, next_index) = if i == segment_domain_length - 1 { (i, 0) } else { (i, i + 1) };
			let pair_handles_and_points = segment_domain.pair_handles_and_points_mut_by_index(sorted_segments[index], sorted_segments[next_index]);
			let (handles, start_point, end_point, next_handles, next_start_point, next_end_point) = pair_handles_and_points;

			let start = vector.point_domain.positions()[*start_point];
			let end = vector.point_domain.positions()[*end_point];

			let mut bezier = handles_to_segment(start, *handles, end);
			bezier = Affine::new(transform.to_cols_array()) * bezier;

			let next_start = vector.point_domain.positions()[*next_start_point];
			let next_end = vector.point_domain.positions()[*next_end_point];

			let mut next_bezier = handles_to_segment(next_start, *next_handles, next_end);
			next_bezier = Affine::new(transform.to_cols_array()) * next_bezier;

			let spilt_distance = calculate_distance_to_spilt(bezier, next_bezier, distance);

			if is_linear(bezier) {
				bezier = PathSeg::Line(Line::new(bezier.start(), bezier.end()));
			}

			if is_linear(next_bezier) {
				next_bezier = PathSeg::Line(Line::new(next_bezier.start(), next_bezier.end()));
			}

			let inverse_transform = if transform.matrix2.determinant() != 0. { transform.inverse() } else { Default::default() };

			if index == 0 && next_index == 1 {
				first_original_length = bezier.perimeter(DEFAULT_ACCURACY);
				first_length = first_original_length;
			}

			let (original_length, length) = if index == 0 {
				(bezier.perimeter(DEFAULT_ACCURACY), bezier.perimeter(DEFAULT_ACCURACY))
			} else {
				(prev_original_length, prev_length)
			};

			let (next_original_length, mut next_length) = if index == segment_domain_length - 1 && next_index == 0 {
				(first_original_length, first_length)
			} else {
				(next_bezier.perimeter(DEFAULT_ACCURACY), next_bezier.perimeter(DEFAULT_ACCURACY))
			};

			// Only split if the length is big enough to make it worthwhile
			let valid_length = length > 1e-10;
			if segments_connected[*end_point] > 0 && valid_length {
				// Apply the bevel to the end
				let distance = spilt_distance.min(original_length.min(next_original_length) / 2.);
				bezier = split_distance(bezier.reverse(), distance, length).reverse();

				if index == 0 && next_index == 1 {
					first_length = (length - distance).max(0.);
				}

				// Update the end position
				let pos = inverse_transform.transform_point2(point_to_dvec2(bezier.end()));
				create_or_modify_point(&mut vector.point_domain, segments_connected, pos, end_point, &mut next_id, &mut new_segments);
			}

			// Update the handles
			*handles = segment_to_handles(&bezier).apply_transformation(|p| inverse_transform.transform_point2(p));

			// Only split if the length is big enough to make it worthwhile
			let valid_length = next_length > 1e-10;
			if segments_connected[*next_start_point] > 0 && valid_length {
				// Apply the bevel to the start
				let distance = spilt_distance.min(next_original_length.min(original_length) / 2.);
				next_bezier = split_distance(next_bezier, distance, next_length);
				next_length = (next_length - distance).max(0.);

				// Update the start position
				let pos = inverse_transform.transform_point2(point_to_dvec2(next_bezier.start()));

				create_or_modify_point(&mut vector.point_domain, segments_connected, pos, next_start_point, &mut next_id, &mut new_segments);

				// Update the handles
				*next_handles = segment_to_handles(&next_bezier).apply_transformation(|p| inverse_transform.transform_point2(p));
			}

			prev_original_length = next_original_length;
			prev_length = next_length;
		}

		new_segments
	}

	fn insert_new_segments(vector: &mut Vector, new_segments: &[[usize; 2]]) {
		let mut next_id = vector.segment_domain.next_id();

		for &[start, end] in new_segments {
			let handles = BezierHandles::Linear;
			vector.segment_domain.push(next_id.next_id(), start, end, handles, StrokeId::ZERO);
		}
	}

	if distance > 1. && vector.segment_domain.ids().len() > 1 {
		let mut segments_connected = segments_connected_count(&vector);
		let new_segments = update_existing_segments(&mut vector, transform, distance, &mut segments_connected);
		insert_new_segments(&mut vector, &new_segments);
	}

	vector
}

#[node_macro::node(category("Vector: Modifier"), path(graphene_core::vector))]
fn bevel(_: impl Ctx, source: Table<Vector>, #[default(10.)] distance: Length) -> Table<Vector> {
	source
		.into_iter()
		.map(|row| TableRow {
			element: bevel_algorithm(row.element, row.transform, distance),
			..row
		})
		.collect()
}

#[node_macro::node(category("Vector: Modifier"), path(graphene_core::vector))]
fn close_path(_: impl Ctx, source: Table<Vector>) -> Table<Vector> {
	source
		.into_iter()
		.map(|mut row| {
			row.element.close_subpaths();
			row
		})
		.collect()
}

#[node_macro::node(category("Vector: Measure"), path(graphene_core::vector))]
fn point_inside(_: impl Ctx, source: Table<Vector>, point: DVec2) -> bool {
	source.into_iter().any(|row| row.element.check_point_inside_shape(row.transform, point))
}

// TODO: Return u32, u64, or usize instead of f64 after #1621 is resolved and has allowed us to implement automatic type conversion in the node graph for nodes with generic type inputs.
// TODO: (Currently automatic type conversion only works for concrete types, via the Graphene preprocessor and not the full Graphene type system.)
#[node_macro::node(category("General"), path(graphene_core::vector))]
async fn count_elements<I>(_: impl Ctx, #[implementations(Table<Graphic>, Table<Vector>, Table<Raster<CPU>>, Table<Raster<GPU>>, Table<Color>, Table<GradientStops>)] source: Table<I>) -> f64 {
	source.len() as f64
}

#[node_macro::node(category("Vector: Measure"), path(graphene_core::vector))]
async fn path_length(_: impl Ctx, source: Table<Vector>) -> f64 {
	source
		.into_iter()
		.map(|row| {
			let transform = row.transform;
			row.element
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
async fn area(ctx: impl Ctx + CloneVarArgs + ExtractAll, content: impl Node<Context<'static>, Output = Table<Vector>>) -> f64 {
	let new_ctx = OwnedContextImpl::from(ctx).with_footprint(Footprint::default()).into_context();
	let vector = content.eval(new_ctx).await;

	vector
		.iter()
		.map(|row| {
			let scale = row.transform.decompose_scale();
			row.element.stroke_bezpath_iter().map(|subpath| subpath.area() * scale.x * scale.y).sum::<f64>()
		})
		.sum()
}

#[node_macro::node(category("Vector: Measure"), path(graphene_core::vector))]
async fn centroid(ctx: impl Ctx + CloneVarArgs + ExtractAll, content: impl Node<Context<'static>, Output = Table<Vector>>, centroid_type: CentroidType) -> DVec2 {
	let new_ctx = OwnedContextImpl::from(ctx).with_footprint(Footprint::default()).into_context();
	let vector = content.eval(new_ctx).await;

	if vector.is_empty() {
		return DVec2::ZERO;
	}

	// All subpath centroid positions added together as if they were vectors from the origin.
	let mut centroid = DVec2::ZERO;
	// Cumulative area or length of all subpaths
	let mut sum = 0.;

	for row in vector.iter() {
		for subpath in row.element.stroke_bezier_paths() {
			let partial = match centroid_type {
				CentroidType::Area => subpath.area_centroid_and_area(Some(1e-3), Some(1e-3)).filter(|(_, area)| *area > 0.),
				CentroidType::Length => subpath.length_centroid_and_length(None, true),
			};
			if let Some((subpath_centroid, area_or_length)) = partial {
				let subpath_centroid = row.transform.transform_point2(subpath_centroid);

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

		let summed_positions = vector
			.iter()
			.flat_map(|row| row.element.point_domain.positions().iter().map(|&p| row.transform.transform_point2(p)))
			.inspect(|_| count += 1)
			.sum::<DVec2>();

		if count != 0 { summed_positions / (count as f64) } else { DVec2::ZERO }
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::Node;
	use crate::vector::algorithms::bezpath_algorithms::{TValue, trim_pathseg};
	use crate::vector::misc::pathseg_abs_diff_eq;
	use kurbo::{CubicBez, Ellipse, Point, Rect};
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

	fn vector_node_from_bezpath(bezpath: BezPath) -> Table<Vector> {
		Table::new_from_element(Vector::from_bezpath(bezpath))
	}

	fn create_vector_row(bezpath: BezPath, transform: DAffine2) -> TableRow<Vector> {
		let mut row = Vector::default();
		row.append_bezpath(bezpath);
		TableRow {
			element: row,
			transform,
			..Default::default()
		}
	}

	#[tokio::test]
	async fn repeat() {
		let direction = DVec2::X * 1.5;
		let count = 3;
		let repeated = super::repeat(
			Footprint::default(),
			vector_node_from_bezpath(Rect::new(0., 0., 1., 1.).to_path(DEFAULT_ACCURACY)),
			direction,
			0.,
			count,
		)
		.await;
		let vector_table = super::flatten_path(Footprint::default(), repeated).await;
		let vector = vector_table.iter().next().unwrap().element;
		assert_eq!(vector.region_manipulator_groups().count(), 3);
		for (index, (_, manipulator_groups)) in vector.region_manipulator_groups().enumerate() {
			assert!((manipulator_groups[0].anchor - direction * index as f64 / (count - 1) as f64).length() < 1e-5);
		}
	}
	#[tokio::test]
	async fn repeat_transform_position() {
		let direction = DVec2::new(12., 10.);
		let count = 8;
		let repeated = super::repeat(
			Footprint::default(),
			vector_node_from_bezpath(Rect::new(0., 0., 1., 1.).to_path(DEFAULT_ACCURACY)),
			direction,
			0.,
			count,
		)
		.await;
		let vector_table = super::flatten_path(Footprint::default(), repeated).await;
		let vector = vector_table.iter().next().unwrap().element;
		assert_eq!(vector.region_manipulator_groups().count(), 8);
		for (index, (_, manipulator_groups)) in vector.region_manipulator_groups().enumerate() {
			assert!((manipulator_groups[0].anchor - direction * index as f64 / (count - 1) as f64).length() < 1e-5);
		}
	}
	#[tokio::test]
	async fn circular_repeat() {
		let repeated = super::circular_repeat(Footprint::default(), vector_node_from_bezpath(Rect::new(-1., -1., 1., 1.).to_path(DEFAULT_ACCURACY)), 45., 4., 8).await;
		let vector_table = super::flatten_path(Footprint::default(), repeated).await;
		let vector = vector_table.iter().next().unwrap().element;
		assert_eq!(vector.region_manipulator_groups().count(), 8);

		for (index, (_, manipulator_groups)) in vector.region_manipulator_groups().enumerate() {
			let expected_angle = (index as f64 + 1.) * 45.;

			let center = (manipulator_groups[0].anchor + manipulator_groups[2].anchor) / 2.;
			let actual_angle = DVec2::Y.angle_to(center).to_degrees();

			assert!((actual_angle - expected_angle).abs() % 360. < 1e-5, "Expected {expected_angle} found {actual_angle}");
		}
	}
	#[tokio::test]
	async fn bounding_box() {
		let bounding_box = super::bounding_box((), vector_node_from_bezpath(Rect::new(-1., -1., 1., 1.).to_path(DEFAULT_ACCURACY))).await;
		let bounding_box = bounding_box.iter().next().unwrap().element;
		assert_eq!(bounding_box.region_manipulator_groups().count(), 1);
		let manipulator_groups_anchors = bounding_box
			.region_manipulator_groups()
			.next()
			.unwrap()
			.1
			.iter()
			.map(|manipulators| manipulators.anchor)
			.collect::<Vec<DVec2>>();

		assert_eq!(&manipulator_groups_anchors[..4], &[DVec2::NEG_ONE, DVec2::new(1., -1.), DVec2::ONE, DVec2::new(-1., 1.),]);

		// Test a rectangular path with non-zero rotation
		let square = Vector::from_bezpath(Rect::new(-1., -1., 1., 1.).to_path(DEFAULT_ACCURACY));
		let mut square = Table::new_from_element(square);
		*square.get_mut(0).unwrap().transform *= DAffine2::from_angle(std::f64::consts::FRAC_PI_4);
		let bounding_box = BoundingBoxNode { content: FutureWrapperNode(square) }.eval(Footprint::default()).await;
		let bounding_box = bounding_box.iter().next().unwrap().element;
		assert_eq!(bounding_box.region_manipulator_groups().count(), 1);
		let manipulator_groups_anchors = bounding_box
			.region_manipulator_groups()
			.next()
			.unwrap()
			.1
			.iter()
			.map(|manipulators| manipulators.anchor)
			.collect::<Vec<DVec2>>();

		let expected_bounding_box = [DVec2::NEG_ONE, DVec2::new(1., -1.), DVec2::ONE, DVec2::new(-1., 1.)];
		for i in 0..4 {
			assert_eq!(manipulator_groups_anchors[i], expected_bounding_box[i]);
		}
	}
	#[tokio::test]
	async fn copy_to_points() {
		let points = Rect::new(-10., -10., 10., 10.).to_path(DEFAULT_ACCURACY);
		let element = Rect::new(-1., -1., 1., 1.).to_path(DEFAULT_ACCURACY);

		let expected_points = Vector::from_bezpath(points.clone()).point_domain.positions().to_vec();

		let copy_to_points = super::copy_to_points(Footprint::default(), vector_node_from_bezpath(points), vector_node_from_bezpath(element), 1., 1., 0., 0, 0., 0).await;
		let flatten_path = super::flatten_path(Footprint::default(), copy_to_points).await;
		let flattened_copy_to_points = flatten_path.iter().next().unwrap().element;

		assert_eq!(flattened_copy_to_points.region_manipulator_groups().count(), expected_points.len());

		for (index, (_, manipulator_groups)) in flattened_copy_to_points.region_manipulator_groups().enumerate() {
			let offset = expected_points[index];
			let manipulator_groups_anchors = manipulator_groups.iter().map(|manipulators| manipulators.anchor).collect::<Vec<DVec2>>();
			assert_eq!(
				&manipulator_groups_anchors,
				&[offset + DVec2::NEG_ONE, offset + DVec2::new(1., -1.), offset + DVec2::ONE, offset + DVec2::new(-1., 1.),]
			);
		}
	}

	#[tokio::test]
	async fn sample_polyline() {
		let path = BezPath::from_vec(vec![PathEl::MoveTo(Point::ZERO), PathEl::CurveTo(Point::ZERO, Point::new(100., 0.), Point::new(100., 0.))]);
		let sample_polyline = super::sample_polyline(Footprint::default(), vector_node_from_bezpath(path), PointSpacingType::Separation, 30., 0, 0., 0., false, vec![100.]).await;
		let sample_polyline = sample_polyline.iter().next().unwrap().element;
		assert_eq!(sample_polyline.point_domain.positions().len(), 4);
		for (pos, expected) in sample_polyline.point_domain.positions().iter().zip([DVec2::X * 0., DVec2::X * 30., DVec2::X * 60., DVec2::X * 90.]) {
			assert!(pos.distance(expected) < 1e-3, "Expected {expected} found {pos}");
		}
	}
	#[tokio::test]
	async fn sample_polyline_adaptive_spacing() {
		let path = BezPath::from_vec(vec![PathEl::MoveTo(Point::ZERO), PathEl::CurveTo(Point::ZERO, Point::new(100., 0.), Point::new(100., 0.))]);
		let sample_polyline = super::sample_polyline(Footprint::default(), vector_node_from_bezpath(path), PointSpacingType::Separation, 18., 0, 45., 10., true, vec![100.]).await;
		let sample_polyline = sample_polyline.iter().next().unwrap().element;
		assert_eq!(sample_polyline.point_domain.positions().len(), 4);
		for (pos, expected) in sample_polyline.point_domain.positions().iter().zip([DVec2::X * 45., DVec2::X * 60., DVec2::X * 75., DVec2::X * 90.]) {
			assert!(pos.distance(expected) < 1e-3, "Expected {expected} found {pos}");
		}
	}
	#[tokio::test]
	async fn poisson() {
		let poisson_points = super::poisson_disk_points(
			Footprint::default(),
			vector_node_from_bezpath(Ellipse::from_rect(Rect::new(-50., -50., 50., 50.)).to_path(DEFAULT_ACCURACY)),
			10. * std::f64::consts::SQRT_2,
			0,
		)
		.await;
		let poisson_points = poisson_points.iter().next().unwrap().element;
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
		let bezpath = BezPath::from_vec(vec![PathEl::MoveTo(Point::ZERO), PathEl::CurveTo(Point::ZERO, Point::new(100., 0.), Point::new(100., 0.))]);
		let lengths = subpath_segment_lengths(Footprint::default(), vector_node_from_bezpath(bezpath)).await;
		assert_eq!(lengths, vec![100.]);
	}
	#[tokio::test]
	async fn path_length() {
		let bezpath = Rect::new(100., 100., 201., 201.).to_path(DEFAULT_ACCURACY);
		let transform = DAffine2::from_scale(DVec2::new(2., 2.));
		let row = create_vector_row(bezpath, transform);
		let table = (0..5).map(|_| row.clone()).collect::<Table<Vector>>();

		let length = super::path_length(Footprint::default(), table).await;

		// 101 (each rectangle edge length) * 4 (rectangle perimeter) * 2 (scale) * 5 (number of rows)
		assert_eq!(length, 101. * 4. * 2. * 5.);
	}
	#[tokio::test]
	async fn spline() {
		let spline = super::spline(Footprint::default(), vector_node_from_bezpath(Rect::new(0., 0., 100., 100.).to_path(DEFAULT_ACCURACY))).await;
		let spline = spline.iter().next().unwrap().element;
		assert_eq!(spline.stroke_bezpath_iter().count(), 1);
		assert_eq!(spline.point_domain.positions(), &[DVec2::ZERO, DVec2::new(100., 0.), DVec2::new(100., 100.), DVec2::new(0., 100.)]);
	}
	#[tokio::test]
	async fn morph() {
		let source = Rect::new(0., 0., 100., 100.).to_path(DEFAULT_ACCURACY);
		let target = Rect::new(-100., -100., 0., 0.).to_path(DEFAULT_ACCURACY);
		let morphed = super::morph(Footprint::default(), vector_node_from_bezpath(source), vector_node_from_bezpath(target), 0.5).await;
		let morphed = morphed.iter().next().unwrap().element;
		assert_eq!(
			&morphed.point_domain.positions()[..4],
			vec![DVec2::new(-50., -50.), DVec2::new(50., -50.), DVec2::new(50., 50.), DVec2::new(-50., 50.)]
		);
	}

	#[track_caller]
	fn contains_segment(vector: Vector, target: PathSeg) {
		let segments = vector.segment_iter().map(|x| x.1);
		let count = segments
			.filter(|segment| pathseg_abs_diff_eq(*segment, target, 0.01) || pathseg_abs_diff_eq(segment.reverse(), target, 0.01))
			.count();

		assert_eq!(
			count,
			1,
			"Expected exactly one matching segment for {target:?}, but found {count}. The given segments are: {:#?}",
			vector.segment_iter().collect::<Vec<_>>()
		);
	}

	#[tokio::test]
	async fn bevel_rect() {
		let source = Rect::new(0., 0., 100., 100.).to_path(DEFAULT_ACCURACY);
		let beveled = super::bevel(Footprint::default(), vector_node_from_bezpath(source), 2_f64.sqrt() * 10.);
		let beveled = beveled.iter().next().unwrap().element;

		assert_eq!(beveled.point_domain.positions().len(), 8);
		assert_eq!(beveled.segment_domain.ids().len(), 8);

		// Segments
		contains_segment(beveled.clone(), PathSeg::Line(Line::new(Point::new(10., 0.), Point::new(90., 0.))));
		contains_segment(beveled.clone(), PathSeg::Line(Line::new(Point::new(10., 100.), Point::new(90., 100.))));
		contains_segment(beveled.clone(), PathSeg::Line(Line::new(Point::new(0., 10.), Point::new(0., 90.))));
		contains_segment(beveled.clone(), PathSeg::Line(Line::new(Point::new(100., 10.), Point::new(100., 90.))));

		// Joins
		contains_segment(beveled.clone(), PathSeg::Line(Line::new(Point::new(10., 0.), Point::new(0., 10.))));
		contains_segment(beveled.clone(), PathSeg::Line(Line::new(Point::new(90., 0.), Point::new(100., 10.))));
		contains_segment(beveled.clone(), PathSeg::Line(Line::new(Point::new(100., 90.), Point::new(90., 100.))));
		contains_segment(beveled.clone(), PathSeg::Line(Line::new(Point::new(10., 100.), Point::new(0., 90.))));
	}

	#[tokio::test]
	async fn bevel_open_curve() {
		let curve = PathSeg::Cubic(CubicBez::new(Point::ZERO, Point::new(10., 0.), Point::new(10., 100.), Point::new(100., 0.)));

		let mut source = BezPath::new();
		source.move_to(Point::new(-100., 0.));
		source.line_to(Point::ZERO);
		source.push(curve.as_path_el());

		let beveled = super::bevel((), vector_node_from_bezpath(source), 2_f64.sqrt() * 10.);
		let beveled = beveled.iter().next().unwrap().element;

		assert_eq!(beveled.point_domain.positions().len(), 4);
		assert_eq!(beveled.segment_domain.ids().len(), 3);

		// Segments
		contains_segment(beveled.clone(), PathSeg::Line(Line::new(Point::new(-8.2, 0.), Point::new(-100., 0.))));
		let trimmed = trim_pathseg(curve, TValue::Euclidean(8.2 / curve.perimeter(DEFAULT_ACCURACY)), TValue::Parametric(1.)).unwrap();
		contains_segment(beveled.clone(), trimmed);

		// Join
		contains_segment(beveled.clone(), PathSeg::Line(Line::new(Point::new(-8.2, 0.), trimmed.start())));
	}

	#[tokio::test]
	async fn bevel_with_transform() {
		let curve = PathSeg::Cubic(CubicBez::new(Point::ZERO, Point::new(10., 0.), Point::new(10., 100.), Point::new(100., 0.)));

		let mut source = BezPath::new();
		source.move_to(Point::new(-100., 0.));
		source.line_to(Point::ZERO);
		source.push(curve.as_path_el());

		let vector = Vector::from_bezpath(source);
		let mut vector_table = Table::new_from_element(vector.clone());

		*vector_table.get_mut(0).unwrap().transform = DAffine2::from_scale_angle_translation(DVec2::splat(10.), 1., DVec2::new(99., 77.));

		let beveled = super::bevel((), Table::new_from_element(vector), 2_f64.sqrt() * 10.);
		let beveled = beveled.iter().next().unwrap().element;

		assert_eq!(beveled.point_domain.positions().len(), 4);
		assert_eq!(beveled.segment_domain.ids().len(), 3);

		// Segments
		contains_segment(beveled.clone(), PathSeg::Line(Line::new(Point::new(-8.2, 0.), Point::new(-100., 0.))));
		let trimmed = trim_pathseg(curve, TValue::Euclidean(8.2 / curve.perimeter(DEFAULT_ACCURACY)), TValue::Parametric(1.)).unwrap();
		contains_segment(beveled.clone(), trimmed);

		// Join
		contains_segment(beveled.clone(), PathSeg::Line(Line::new(Point::new(-8.2, 0.), trimmed.start())));
	}

	#[tokio::test]
	async fn bevel_too_high() {
		let mut source = BezPath::new();
		source.move_to(Point::ZERO);
		source.line_to(Point::new(100., 0.));
		source.line_to(Point::new(100., 100.));
		source.line_to(Point::new(0., 100.));

		let beveled = super::bevel(Footprint::default(), vector_node_from_bezpath(source), 999.);
		let beveled = beveled.iter().next().unwrap().element;

		assert_eq!(beveled.point_domain.positions().len(), 6);
		assert_eq!(beveled.segment_domain.ids().len(), 5);

		// Segments
		contains_segment(beveled.clone(), PathSeg::Line(Line::new(Point::new(0., 0.), Point::new(50., 0.))));
		contains_segment(beveled.clone(), PathSeg::Line(Line::new(Point::new(100., 50.), Point::new(100., 50.))));
		contains_segment(beveled.clone(), PathSeg::Line(Line::new(Point::new(100., 50.), Point::new(50., 100.))));

		// Joins
		contains_segment(beveled.clone(), PathSeg::Line(Line::new(Point::new(50., 0.), Point::new(100., 50.))));
		contains_segment(beveled.clone(), PathSeg::Line(Line::new(Point::new(100., 50.), Point::new(50., 100.))));
	}

	#[tokio::test]
	async fn bevel_repeated_point() {
		let line = PathSeg::Line(Line::new(Point::ZERO, Point::new(100., 0.)));
		let point = PathSeg::Cubic(CubicBez::new(Point::new(100., 0.), Point::ZERO, Point::ZERO, Point::new(100., 0.)));
		let curve = PathSeg::Cubic(CubicBez::new(Point::new(100., 0.), Point::new(110., 0.), Point::new(110., 200.), Point::new(200., 0.)));

		let subpath = BezPath::from_path_segments([line, point, curve].into_iter());

		let beveled_table = super::bevel(Footprint::default(), vector_node_from_bezpath(subpath), 5.);
		let beveled = beveled_table.iter().next().unwrap().element;

		assert_eq!(beveled.point_domain.positions().len(), 6);
		assert_eq!(beveled.segment_domain.ids().len(), 5);
	}
}

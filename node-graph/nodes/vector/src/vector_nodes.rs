use core::cmp::Ordering;
use core::f64::consts::{PI, TAU};
use core::hash::{Hash, Hasher};
use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::registry::types::{Angle, Length, Multiplier, Percentage, PixelLength, Progression, SeedValue};
use core_types::table::{Table, TableRow, TableRowMut};
use core_types::transform::{Footprint, Transform};
use core_types::{CloneVarArgs, Color, Context, Ctx, ExtractAll, OwnedContextImpl};
use glam::{DAffine2, DMat2, DVec2};
use graphic_types::Vector;
use graphic_types::raster_types::{CPU, GPU, Raster};
use graphic_types::{Graphic, IntoGraphicTable};
use kurbo::simplify::{SimplifyOptions, simplify_bezpath};
use kurbo::{Affine, BezPath, DEFAULT_ACCURACY, Line, ParamCurve, ParamCurveArclen, PathEl, PathSeg, Shape};
use rand::{Rng, SeedableRng};
use std::collections::hash_map::DefaultHasher;
use vector_types::subpath::{BezierHandles, ManipulatorGroup};
use vector_types::vector::PointDomain;
use vector_types::vector::algorithms::bezpath_algorithms::{self, TValue, eval_pathseg_euclidean, evaluate_bezpath, split_bezpath, tangent_on_bezpath};
use vector_types::vector::algorithms::merge_by_distance::MergeByDistanceExt;
use vector_types::vector::algorithms::offset_subpath::offset_bezpath;
use vector_types::vector::algorithms::spline::{solve_spline_first_handle_closed, solve_spline_first_handle_open};
use vector_types::vector::misc::{
	CentroidType, ExtrudeJoiningAlgorithm, HandleId, InterpolationDistribution, MergeByDistanceAlgorithm, PointSpacingType, RowsOrColumns, bezpath_from_manipulator_groups,
	bezpath_to_manipulator_groups, handles_to_segment, is_linear, point_to_dvec2, segment_to_handles,
};
use vector_types::vector::style::{Fill, Gradient, GradientStops, PaintOrder, Stroke, StrokeAlign, StrokeCap, StrokeJoin};
use vector_types::vector::{FillId, PointId, RegionId, SegmentDomain, SegmentId, StrokeId, VectorExt};

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

/// Uniquely sets the fill and/or stroke style of every vector element to individual colors sampled along a chosen gradient.
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
	gradient: Table<GradientStops>,
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
	let Some(row) = gradient.into_iter().next() else { return content };

	let length = content.vector_iter_mut().count();
	let gradient = if reverse { row.element.reversed() } else { row.element };

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
		if stroke && let Some(stroke) = vector.element.style.stroke().and_then(|stroke| stroke.with_color(&Some(color))) {
			vector.element.style.set_stroke(stroke);
		}
	}

	content
}

/// Applies a fill style to the vector content, giving an appearance to the area within the interior of the geometry.
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
	#[default(Color::BLACK)]
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

trait IntoF64Vec {
	fn into_vec(self) -> Vec<f64>;
}
impl IntoF64Vec for f64 {
	fn into_vec(self) -> Vec<f64> {
		vec![self]
	}
}
impl IntoF64Vec for Vec<f64> {
	fn into_vec(self) -> Vec<f64> {
		self
	}
}
impl IntoF64Vec for String {
	fn into_vec(self) -> Vec<f64> {
		self.split(&[',', ' ']).filter(|s| !s.is_empty()).filter_map(|s| s.parse::<f64>().ok()).collect()
	}
}

/// Applies a stroke style to the vector content, giving an appearance to the area within the outline of the geometry.
#[node_macro::node(category("Vector: Style"), path(graphene_core::vector), properties("stroke_properties"))]
async fn stroke<V, L: IntoF64Vec>(
	_: impl Ctx,
	/// The content with vector paths to apply the stroke style to.
	#[implementations(Table<Vector>, Table<Vector>, Table<Vector>, Table<Graphic>, Table<Graphic>, Table<Graphic>)]
	mut content: Table<V>,
	/// The stroke color.
	#[default(Color::BLACK)]
	color: Table<Color>,
	/// The stroke thickness.
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
	// <https://svgwg.org/svg2-draft/painting.html#PaintOrderProperty>
	/// The order to paint the stroke on top of the fill, or the fill on top of the stroke.
	paint_order: PaintOrder,
	/// The stroke dash lengths. Each length forms a distance in a pattern where the first length is a dash, the second is a gap, and so on. If the list is an odd length, the pattern repeats with solid-gap roles reversed.
	#[implementations(Vec<f64>, f64, String, Vec<f64>, f64, String)]
	dash_lengths: L,
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
		dash_lengths: dash_lengths.into_vec(),
		dash_offset,
		cap,
		join,
		join_miter_limit: miter_limit,
		align,
		transform: DAffine2::IDENTITY,
		paint_order,
	};

	for vector in content.vector_iter_mut() {
		let mut stroke = stroke.clone();
		stroke.transform *= *vector.transform;
		vector.element.style.set_stroke(stroke);
	}

	content
}

#[node_macro::node(name("Copy to Points"), category("Repeat"), path(core_types::vector))]
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

#[node_macro::node(category("Vector: Modifier"), path(core_types::vector))]
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

			let upstream_nested_layers = source.upstream_data.clone();

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
					let prev_index = if i == 0 { if is_closed { manipulator_groups.len() - 1 } else { 0 } } else { i - 1 };
					let curr_index = i;
					let next_index = if i == manipulator_groups.len() - 1 { if is_closed { 0 } else { i } } else { i + 1 };

					let prev = manipulator_groups[prev_index].anchor;
					let curr = manipulator_groups[curr_index].anchor;
					let next = manipulator_groups[next_index].anchor;

					let dir1 = (curr - prev).normalize_or(DVec2::X);
					let dir2 = (next - curr).normalize_or(DVec2::X);

					let theta = PI - dir1.angle_to(dir2).abs();

					// Skip near-straight corners
					if theta > PI - min_angle_threshold.to_radians() {
						new_manipulator_groups.push(manipulator_groups[curr_index]);
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

			result.upstream_data = upstream_nested_layers;

			TableRow {
				element: result,
				transform: source_transform,
				alpha_blending: Default::default(),
				source_node_id: *source_node_id,
				additional: Default::default(),
			}
		})
		.collect()
}

#[node_macro::node(name("Merge by Distance"), category("Vector: Modifier"), path(core_types::vector))]
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

pub mod extrude_algorithms {
	use glam::DVec2;
	use kurbo::{ParamCurve, ParamCurveDeriv};
	use vector_types::subpath::BezierHandles;
	use vector_types::vector::StrokeId;
	use vector_types::vector::misc::ExtrudeJoiningAlgorithm;

	/// Convert [`vector_types::subpath::Bezier`] to [`kurbo::PathSeg`].
	fn bezier_to_path_seg(bezier: vector_types::subpath::Bezier) -> kurbo::PathSeg {
		let [start, end] = [(bezier.start().x, bezier.start().y), (bezier.end().x, bezier.end().y)];
		match bezier.handles {
			BezierHandles::Linear => kurbo::Line::new(start, end).into(),
			BezierHandles::Quadratic { handle } => kurbo::QuadBez::new(start, (handle.x, handle.y), end).into(),
			BezierHandles::Cubic { handle_start, handle_end } => kurbo::CubicBez::new(start, (handle_start.x, handle_start.y), (handle_end.x, handle_end.y), end).into(),
		}
	}

	/// Convert [`kurbo::CubicBez`] to [`vector_types::subpath::BezierHandles`].
	fn cubic_to_handles(cubic_bez: kurbo::CubicBez) -> BezierHandles {
		BezierHandles::Cubic {
			handle_start: DVec2::new(cubic_bez.p1.x, cubic_bez.p1.y),
			handle_end: DVec2::new(cubic_bez.p2.x, cubic_bez.p2.y),
		}
	}

	/// Find the `t` values to split (where the tangent changes to be on the other side of the direction).
	fn find_splits(cubic_segment: kurbo::CubicBez, direction: DVec2) -> impl Iterator<Item = f64> {
		let derivative = cubic_segment.deriv();
		let convert = |x: kurbo::Point| DVec2::new(x.x, x.y);
		let derivative_points = [derivative.p0, derivative.p1, derivative.p2].map(convert);

		let t_squared = derivative_points[0] - 2. * derivative_points[1] + derivative_points[2];
		let t_scalar = -2. * derivative_points[0] + 2. * derivative_points[1];
		let constant = derivative_points[0];

		kurbo::common::solve_quadratic(constant.perp_dot(direction), t_scalar.perp_dot(direction), t_squared.perp_dot(direction))
			.into_iter()
			.filter(|&t| t > 1e-6 && t < 1. - 1e-6)
	}

	/// Split so segments no longer have tangents on both sides of the direction vector.
	fn split(vector: &mut graphic_types::Vector, direction: DVec2) {
		let segment_count = vector.segment_domain.ids().len();
		let mut next_point = vector.point_domain.next_id();
		let mut next_segment = vector.segment_domain.next_id();

		for segment_index in 0..segment_count {
			let (_, _, bezier) = vector.segment_points_from_index(segment_index);
			let mut start_index = vector.segment_domain.start_point()[segment_index];
			let pathseg = bezier_to_path_seg(bezier).to_cubic();
			let mut start_t = 0.;

			for split_t in find_splits(pathseg, direction) {
				let [first, second] = [pathseg.subsegment(start_t..split_t), pathseg.subsegment(split_t..1.)];
				let [first_handles, second_handles] = [first, second].map(cubic_to_handles);
				let middle_point = next_point.next_id();
				let start_segment = next_segment.next_id();

				let middle_point_index = vector.point_domain.len();
				vector.point_domain.push(middle_point, DVec2::new(first.end().x, first.end().y));
				vector.segment_domain.push(start_segment, start_index, middle_point_index, first_handles, StrokeId::ZERO);
				vector.segment_domain.set_start_point(segment_index, middle_point_index);
				vector.segment_domain.set_handles(segment_index, second_handles);

				start_t = split_t;
				start_index = middle_point_index;
			}
		}
	}

	/// Copy all segments with the offset of `direction`.
	fn offset_copy_all_segments(vector: &mut graphic_types::Vector, direction: DVec2) {
		let points_count = vector.point_domain.ids().len();
		let mut next_point = vector.point_domain.next_id();
		for index in 0..points_count {
			vector.point_domain.push(next_point.next_id(), vector.point_domain.positions()[index] + direction);
		}

		let segment_count = vector.segment_domain.ids().len();
		let mut next_segment = vector.segment_domain.next_id();
		for index in 0..segment_count {
			vector.segment_domain.push(
				next_segment.next_id(),
				vector.segment_domain.start_point()[index] + points_count,
				vector.segment_domain.end_point()[index] + points_count,
				vector.segment_domain.handles()[index].apply_transformation(|x| x + direction),
				vector.segment_domain.stroke()[index],
			);
		}
	}

	/// Join points from the original to the copied that are on opposite sides of the direction.
	fn join_extrema_edges(vector: &mut graphic_types::Vector, direction: DVec2) {
		#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
		enum Found {
			#[default]
			None,
			Positive,
			Negative,
			Both,
			Invalid,
		}

		impl Found {
			fn update(&mut self, value: f64) {
				*self = match (*self, value > 0.) {
					(Found::None, true) => Found::Positive,
					(Found::None, false) => Found::Negative,
					(Found::Positive, true) | (Found::Negative, false) => Found::Both,
					_ => Found::Invalid,
				};
			}
		}

		let first_half_points = vector.point_domain.len() / 2;
		let mut points = vec![Found::None; first_half_points];
		let first_half_segments = vector.segment_domain.ids().len() / 2;

		for segment_id in 0..first_half_segments {
			let index = [vector.segment_domain.start_point()[segment_id], vector.segment_domain.end_point()[segment_id]];
			let position = index.map(|index| vector.point_domain.positions()[index]);

			if position[0].abs_diff_eq(position[1], 1e-6) {
				continue; // Skip zero length segments
			}

			points[index[0]].update(direction.perp_dot(position[1] - position[0]));
			points[index[1]].update(direction.perp_dot(position[0] - position[1]));
		}

		let mut next_segment = vector.segment_domain.next_id();
		for (index, &point) in points.iter().enumerate().take(first_half_points) {
			// Extrema are single connected points or points with both positive and negative values
			if !matches!(point, Found::Both | Found::Positive | Found::Negative) {
				continue;
			}

			vector
				.segment_domain
				.push(next_segment.next_id(), index, index + first_half_points, BezierHandles::Linear, StrokeId::ZERO);
		}
	}

	/// Join all points from the original to the copied.
	fn join_all(vector: &mut graphic_types::Vector) {
		let mut next_segment = vector.segment_domain.next_id();
		let first_half = vector.point_domain.len() / 2;
		for index in 0..first_half {
			vector.segment_domain.push(next_segment.next_id(), index, index + first_half, BezierHandles::Linear, StrokeId::ZERO);
		}
	}

	pub fn extrude(vector: &mut graphic_types::Vector, direction: DVec2, joining_algorithm: ExtrudeJoiningAlgorithm) {
		split(vector, direction);
		offset_copy_all_segments(vector, direction);

		match joining_algorithm {
			ExtrudeJoiningAlgorithm::Extrema => join_extrema_edges(vector, direction),
			ExtrudeJoiningAlgorithm::All => join_all(vector),
			ExtrudeJoiningAlgorithm::None => {}
		}
	}

	#[cfg(test)]
	mod extrude_tests {
		use glam::DVec2;
		use kurbo::{ParamCurve, ParamCurveDeriv};

		#[test]
		fn split_cubic() {
			let l1 = kurbo::CubicBez::new((0., 0.), (100., 0.), (100., 100.), (0., 100.));
			assert_eq!(super::find_splits(l1, DVec2::Y).collect::<Vec<f64>>(), vec![0.5]);
			assert!(super::find_splits(l1, DVec2::X).collect::<Vec<f64>>().is_empty());

			let l2 = kurbo::CubicBez::new((0., 0.), (0., 0.), (100., 0.), (100., 0.));
			assert!(super::find_splits(l2, DVec2::X).collect::<Vec<f64>>().is_empty());

			let l3 = kurbo::PathSeg::Line(kurbo::Line::new((0., 0.), (100., 0.)));
			assert!(super::find_splits(l3.to_cubic(), DVec2::X).collect::<Vec<f64>>().is_empty());

			let l4 = kurbo::CubicBez::new((0., 0.), (100., -10.), (100., 110.), (0., 100.));
			let splits = super::find_splits(l4, DVec2::X).map(|t| l4.deriv().eval(t)).collect::<Vec<_>>();
			assert_eq!(splits.len(), 2);
			assert!(splits.iter().all(|&deriv| deriv.y.abs() < 1e-8), "{splits:?}");
		}

		#[test]
		fn split_vector() {
			let curve = kurbo::PathSeg::Cubic(kurbo::CubicBez::new((0., 0.), (100., -10.), (100., 110.), (0., 100.)));
			let mut vector = graphic_types::Vector::from_bezpath(kurbo::BezPath::from_path_segments([curve].into_iter()));
			super::split(&mut vector, DVec2::X);
			assert_eq!(vector.segment_ids().len(), 3);
			assert_eq!(vector.point_domain.ids().len(), 4);
		}
	}
}

#[node_macro::node(category("Vector: Modifier"), path(core_types::vector))]
async fn extrude(_: impl Ctx, mut source: Table<Vector>, direction: DVec2, joining_algorithm: ExtrudeJoiningAlgorithm) -> Table<Vector> {
	for TableRowMut { element: source, .. } in source.iter_mut() {
		extrude_algorithms::extrude(source, direction, joining_algorithm);
	}
	source
}

#[node_macro::node(category("Vector: Modifier"), path(core_types::vector))]
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

#[node_macro::node(category("Vector"), path(graphene_core::vector))]
async fn pack_strips<T: 'n + Send + Clone>(
	_: impl Ctx,
	#[implementations(
		Table<Graphic>,
		Table<Vector>,
		Table<Raster<CPU>>,
		Table<Raster<GPU>>,
	)]
	elements: Table<T>,
	#[default(0.)]
	#[unit(" px")]
	separation: f64,
	#[default(1000.)]
	#[unit(" px")]
	strip_max_length: f64,
	strip_direction: RowsOrColumns,
) -> Table<T>
where
	Graphic: From<Table<T>>,
	Table<T>: BoundingBox,
{
	// Packs shapes using bounds with Best-Fit Decreasing Height (BFDH) algorithm:
	// - Sort shapes by cross-axis size (tallest first for rows, widest first for columns)
	// - For each shape, find the existing strip with minimum remaining space that fits
	// - Create new strip only if no existing strip can accommodate the shape

	struct Strip {
		along_position: f64,
		cross_position: f64,
		cross_extent: f64,
	}

	// Prepare the items to be sorted
	let mut items: Vec<(f64, f64, DVec2, TableRow<T>)> = elements
		.into_iter()
		.map(|row| {
			// Single-element table to query its bounding box
			let single = Table::new_from_row(row.clone());
			let (w, h, top_left) = match single.bounding_box(DAffine2::IDENTITY, false) {
				RenderBoundingBox::Rectangle([min, max]) => {
					let size = max - min;
					(size.x.max(0.), size.y.max(0.), min)
				}
				_ => (0., 0., DVec2::ZERO),
			};
			let (along, cross) = match strip_direction {
				RowsOrColumns::Rows => (w, h),
				RowsOrColumns::Columns => (h, w),
			};
			(along, cross, top_left, row)
		})
		.collect();

	// Sort by cross-axis size, largest first
	items.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

	let mut result = Table::new();
	let mut strips: Vec<Strip> = Vec::new();

	// This looks n^2 but it is just n*k where k is the number of strips, which is generally much smaller than n
	for (along, cross, top_left, mut row) in items {
		if along <= 0. {
			result.push(row);
			continue;
		}

		// Find a good strip, minimum remaining space that can fit this item ideally
		let mut best_strip_index = None;
		let mut min_remaining_space = f64::INFINITY;

		for (index, strip) in strips.iter().enumerate() {
			let remaining_space = strip_max_length - strip.along_position;
			if remaining_space >= along && remaining_space < min_remaining_space {
				min_remaining_space = remaining_space;
				best_strip_index = Some(index);
			}
		}

		if let Some(strip_index) = best_strip_index {
			// Place on existing strip
			let strip = &mut strips[strip_index];

			// Update strip cross extent if needed
			if cross > strip.cross_extent {
				strip.cross_extent = cross;
			}

			let target_position = match strip_direction {
				RowsOrColumns::Rows => DVec2::new(strip.along_position, strip.cross_position),
				RowsOrColumns::Columns => DVec2::new(strip.cross_position, strip.along_position),
			};
			row.transform = DAffine2::from_translation(target_position - top_left) * row.transform;

			strip.along_position += along + separation;
		} else {
			// Create new strip
			let new_cross = strips.last().map_or(0., |last| last.cross_position + last.cross_extent + separation);

			let target_position = match strip_direction {
				RowsOrColumns::Rows => DVec2::new(0., new_cross),
				RowsOrColumns::Columns => DVec2::new(new_cross, 0.),
			};
			row.transform = DAffine2::from_translation(target_position - top_left) * row.transform;

			strips.push(Strip {
				along_position: along + separation,
				cross_position: new_cross,
				cross_extent: cross,
			});
		}

		result.push(row);
	}

	result
}

/// Automatically constructs tangents (Bézier handles) for anchor points in a vector path.
#[node_macro::node(category("Vector: Modifier"), name("Auto-Tangents"), path(core_types::vector))]
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
				// Track which manipulator indices were given auto-tangent (colinear) handles
				let mut auto_tangented = vec![false; manipulators_list.len()];
				let is_closed = subpath.closed();

				for i in 0..manipulators_list.len() {
					let current = &manipulators_list[i];
					let is_endpoint = !is_closed && (i == 0 || i == manipulators_list.len() - 1);

					if preserve_existing {
						// Check if this point has handles that are meaningfully different from the anchor
						let has_handles = (current.in_handle.is_some() && !current.in_handle.unwrap().abs_diff_eq(current.anchor, 1e-5))
							|| (current.out_handle.is_some() && !current.out_handle.unwrap().abs_diff_eq(current.anchor, 1e-5));

						// If the point already has handles, keep it as is
						if has_handles {
							new_manipulators_list.push(*current);
							continue;
						}
					}

					// If spread is 0, remove handles for this point, making it a sharp corner
					if spread == 0. {
						new_manipulators_list.push(ManipulatorGroup {
							anchor: current.anchor,
							in_handle: None,
							out_handle: None,
							id: current.id,
						});
						continue;
					}

					// Endpoints of open paths get zero-length cubic handles so adjacent segments remain cubic (not quadratic)
					if is_endpoint {
						new_manipulators_list.push(ManipulatorGroup {
							anchor: current.anchor,
							in_handle: Some(current.anchor),
							out_handle: Some(current.anchor),
							id: current.id,
						});
						continue;
					}

					// Get previous and next points for auto-tangent calculation
					let prev_index = if i == 0 { manipulators_list.len() - 1 } else { i - 1 };
					let next_index = if i == manipulators_list.len() - 1 { 0 } else { i + 1 };

					let current_position = current.anchor;
					let delta_prev = manipulators_list[prev_index].anchor - current_position;
					let delta_next = manipulators_list[next_index].anchor - current_position;

					// Calculate normalized directions and distances to adjacent points
					let distance_prev = delta_prev.length();
					let distance_next = delta_next.length();

					// Check if we have valid directions (e.g., points are not coincident)
					if distance_prev < 1e-5 || distance_next < 1e-5 {
						// Fallback: keep the original manipulator group (which has no active handles here)
						new_manipulators_list.push(*current);
						continue;
					}

					let direction_prev = delta_prev / distance_prev;
					let direction_next = delta_next / distance_next;

					// Calculate handle direction as the bisector of the two normalized directions.
					// This ensures the in and out handles are colinear (180° apart) through the anchor.
					let mut handle_direction = (direction_prev - direction_next).try_normalize().unwrap_or_else(|| direction_prev.perp());

					// Ensure consistent orientation of the handle direction.
					// This makes the `+ handle_direction` for in_handle and `- handle_direction` for out_handle consistent.
					if direction_prev.dot(handle_direction) < 0. {
						handle_direction = -handle_direction;
					}

					// Calculate handle lengths: 1/3 of distance to adjacent points, scaled by spread
					let in_length = distance_prev / 3. * spread;
					let out_length = distance_next / 3. * spread;

					// Create new manipulator group with calculated auto-tangents
					new_manipulators_list.push(ManipulatorGroup {
						anchor: current_position,
						in_handle: Some(current_position + handle_direction * in_length),
						out_handle: Some(current_position - handle_direction * out_length),
						id: current.id,
					});
					auto_tangented[i] = true;
				}

				// Record segment count before appending so we can find the new segment IDs
				let segment_offset = result.segment_domain.ids().len();

				let mut softened_bezpath = bezpath_from_manipulator_groups(&new_manipulators_list, is_closed);
				softened_bezpath.apply_affine(Affine::new(transform.inverse().to_cols_array()));
				result.append_bezpath(softened_bezpath);

				// Mark auto-tangented points as having colinear handles
				let segment_ids = result.segment_domain.ids();
				let num_manipulators = new_manipulators_list.len();
				for (i, _) in auto_tangented.iter().enumerate().filter(|&(_, &tangented)| tangented) {
					// For interior point i, the incoming segment is segment_offset + (i - 1) and outgoing is segment_offset + i.
					// For closed paths, point 0's incoming segment is the last one (segment_offset + num_manipulators - 1).
					// For open paths, endpoints are never auto-tangented (the `is_endpoint` check above ensures that),
					// so `i == 0` and `i == num_manipulators - 1` only occur here when the path is closed
					let in_segment_index = if i == 0 { segment_offset + num_manipulators - 1 } else { segment_offset + i - 1 };
					let out_segment_index = if i == num_manipulators - 1 { segment_offset } else { segment_offset + i };

					if in_segment_index < segment_ids.len() && out_segment_index < segment_ids.len() {
						result
							.colinear_manipulators
							.push([HandleId::end(segment_ids[in_segment_index]), HandleId::primary(segment_ids[out_segment_index])]);
					}
				}
			}

			TableRow {
				element: result,
				transform,
				alpha_blending,
				source_node_id,
				additional: Default::default(),
			}
		})
		.collect()
}

#[node_macro::node(category("Vector: Modifier"), path(core_types::vector))]
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

#[node_macro::node(category("Vector: Measure"), path(core_types::vector))]
async fn dimensions(_: impl Ctx, content: Table<Vector>) -> DVec2 {
	content
		.iter()
		.filter_map(|vector| vector.element.bounding_box_with_transform(*vector.transform))
		.reduce(|[acc_top_left, acc_bottom_right], [top_left, bottom_right]| [acc_top_left.min(top_left), acc_bottom_right.max(bottom_right)])
		.map(|[top_left, bottom_right]| bottom_right - top_left)
		.unwrap_or_default()
}

// TODO: Replace this node with an automatic type conversion implementation of the `Convert` trait
/// Converts a vec2 value into a vector path composed of a single anchor point.
///
/// This is useful in conjunction with nodes that repeat it, followed by the "Points to Polyline" node to string together a path of the points.
#[node_macro::node(category("Vector"), name("Vec2 to Point"), path(core_types::vector))]
async fn vec2_to_point(_: impl Ctx, vec2: DVec2) -> Table<Vector> {
	let mut point_domain = PointDomain::new();
	point_domain.push(PointId::generate(), vec2);

	Table::new_from_row(TableRow {
		element: Vector { point_domain, ..Default::default() },
		..Default::default()
	})
}

/// Creates a polyline from a series of vector points, replacing any existing segments and regions that may already exist.
#[node_macro::node(category("Vector"), name("Points to Polyline"), path(core_types::vector))]
async fn points_to_polyline(_: impl Ctx, mut points: Table<Vector>, #[default(true)] closed: bool) -> Table<Vector> {
	for row in points.iter_mut() {
		let mut segment_domain = SegmentDomain::new();
		let mut next_id = SegmentId::ZERO;

		let points_count = row.element.point_domain.ids().len();

		if points_count >= 2 {
			(0..points_count - 1).for_each(|i| {
				segment_domain.push(next_id.next_id(), i, i + 1, BezierHandles::Linear, StrokeId::generate());
			});

			if closed && points_count != 2 {
				segment_domain.push(next_id.next_id(), points_count - 1, 0, BezierHandles::Linear, StrokeId::generate());

				row.element
					.region_domain
					.push(RegionId::generate(), segment_domain.ids()[0]..=*segment_domain.ids().last().unwrap(), FillId::generate());
			}
		}

		row.element.segment_domain = segment_domain;
	}

	points
}

#[node_macro::node(category("Vector: Modifier"), path(core_types::vector), properties("offset_path_properties"))]
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

#[node_macro::node(category("Vector: Modifier"), path(core_types::vector))]
async fn solidify_stroke(_: impl Ctx, content: Table<Vector>) -> Table<Vector> {
	// TODO: Make this node support stroke align, which it currently ignores

	content
		.into_iter()
		.flat_map(|row| {
			let mut vector = row.element;
			let transform = row.transform;
			let alpha_blending = row.alpha_blending;
			let source_node_id = row.source_node_id;

			let stroke = vector.style.stroke().clone().unwrap_or_default();
			let bezpaths = vector.stroke_bezpath_iter();
			let mut solidified_stroke = Vector::default();

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
			let paint_order = stroke.paint_order;

			let stroke_style = kurbo::Stroke::new(stroke.weight)
				.with_caps(cap)
				.with_join(join)
				.with_dashes(dash_offset, dash_pattern)
				.with_miter_limit(miter_limit);

			let stroke_options = kurbo::StrokeOpts::default();

			// 0.25 is balanced between performace and accuracy of the curve.
			const STROKE_TOLERANCE: f64 = 0.25;

			for mut path in bezpaths {
				path.apply_affine(Affine::new(stroke.transform.to_cols_array()));

				let mut solidified = kurbo::stroke(path, &stroke_style, &stroke_options, STROKE_TOLERANCE);
				if stroke.transform.matrix2.determinant() != 0. {
					solidified.apply_affine(Affine::new(stroke.transform.inverse().to_cols_array()));
				}

				solidified_stroke.append_bezpath(solidified);
			}

			// We set the solidified stroke's fill to the stroke's color and without a stroke.
			if let Some(stroke) = vector.style.stroke() {
				solidified_stroke.style.set_fill(Fill::solid_or_none(stroke.color));
			}

			let stroke_row = TableRow {
				element: solidified_stroke,
				transform,
				alpha_blending,
				source_node_id,
				additional: Default::default(),
			};

			// If the original vector has a fill, preserve it as a separate row with the stroke cleared.
			let has_fill = !vector.style.fill().is_none();
			let fill_row = has_fill.then(move || {
				vector.style.clear_stroke();
				TableRow {
					element: vector,
					transform,
					alpha_blending,
					source_node_id,
					additional: Default::default(),
				}
			});

			// Ordering based on the paint order. The first row in the table is rendered below the second.
			match paint_order {
				PaintOrder::StrokeAbove => fill_row.into_iter().chain(std::iter::once(stroke_row)).collect::<Vec<_>>(),
				PaintOrder::StrokeBelow => std::iter::once(stroke_row).chain(fill_row).collect::<Vec<_>>(),
			}
		})
		.collect()
}

#[node_macro::node(category("Vector: Modifier"), path(core_types::vector))]
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
						additional: Default::default(),
					}
				})
				.collect::<Vec<TableRow<Vector>>>()
		})
		.collect()
}

/// Determines if the subpath at the given index (across all vector element subpaths) is closed, meaning its ends are connected together forming a loop.
#[node_macro::node(name("Path is Closed"), category("Vector: Measure"), path(core_types::vector))]
async fn path_is_closed(
	_: impl Ctx,
	/// The vector content whose subpaths are inspected.
	content: Table<Vector>,
	/// The index of the subpath to check, counting across subpaths in all vector elements.
	index: f64,
) -> bool {
	content
		.iter()
		.flat_map(|row| row.element.build_stroke_path_iter().map(|(_, closed)| closed))
		.nth(index.max(0.) as usize)
		.unwrap_or(false)
}

#[node_macro::node(category("Vector"), path(graphene_core::vector))]
async fn map_points(ctx: impl Ctx + CloneVarArgs + ExtractAll, content: Table<Vector>, mapped: impl Node<Context<'static>, Output = DVec2>) -> Table<Vector> {
	let mut content = content;
	let mut index = 0;

	for row in content.iter_mut() {
		for (_, position) in row.element.point_domain.positions_mut() {
			let owned_ctx = OwnedContextImpl::from(ctx.clone()).with_index(index).with_position(*position);
			index += 1;

			*position = mapped.eval(owned_ctx.into_context()).await;
		}
	}

	content
}

// TODO: Rename to "Combine Paths" and make this happen per-element instead of flattening every element into a single path. The migration for this should then become a Flatten Vector -> Combine Paths pair of nodes.
#[node_macro::node(category("Vector"), path(graphene_core::vector))]
pub async fn flatten_path<T: IntoGraphicTable + 'n + Send>(_: impl Ctx, #[implementations(Table<Graphic>, Table<Vector>)] content: T) -> Table<Vector> {
	// Create a table with one empty `Vector` element, then get a mutable reference to it which we append flattened subpaths to
	let mut output_table = Table::new_from_element(Vector::default());
	let Some(output) = output_table.iter_mut().next() else { return output_table };

	// Concatenate every vector element's subpaths into the single output compound path
	for (index, row) in content.into_flattened_table().iter().enumerate() {
		let node_id = row.source_node_id.map(|node_id| node_id.0).unwrap_or_default();

		let mut hasher = DefaultHasher::new();
		(index, node_id).hash(&mut hasher);
		let collision_hash_seed = hasher.finish();

		output.element.concat(row.element, *row.transform, collision_hash_seed);

		// TODO: Make this instead use the first encountered style
		// Use the last encountered style as the output style
		output.element.style = row.element.style.clone();
	}

	output_table
}

/// Convert vector geometry into a polyline composed of evenly spaced points.
#[node_macro::node(category(""), path(core_types::vector))]
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
				upstream_data: std::mem::take(&mut row.element.upstream_data),
			};
			// Transfer the stroke transform from the input vector content to the result.
			result.style.set_stroke_transform(row.transform);

			// Using `stroke_bezpath_iter` so that the `subpath_segment_lengths` is aligned to the segments of each bezpath.
			// So we can index into `subpath_segment_lengths` to get the length of the segments.
			// NOTE: `subpath_segment_lengths` has precalulated lengths with transformation applied.
			let bezpaths = row.element.stroke_bezpath_iter();

			// Keeps track of the index of the first segment of the next bezpath in order to get lengths of all segments.
			let mut next_segment_index = 0;

			for local_bezpath in bezpaths {
				// Apply the transform to compute sample locations in world space (for correct distance-based spacing)
				let mut world_bezpath = local_bezpath.clone();
				world_bezpath.apply_affine(Affine::new(row.transform.to_cols_array()));

				let segment_count = world_bezpath.segments().count();

				// For the current bezpath we get its segment's length by calculating the start index and end index.
				let current_bezpath_segments_length = &subpath_segment_lengths[next_segment_index..next_segment_index + segment_count];

				// Increment the segment index by the number of segments in the current bezpath to calculate the next bezpath segment's length.
				next_segment_index += segment_count;

				let amount = match spacing {
					PointSpacingType::Separation => separation,
					PointSpacingType::Quantity => quantity as f64,
				};

				// Compute sample locations using world-space distances, then evaluate positions on the untransformed bezpath.
				// This avoids needing to invert the transform (which fails when the transform is singular, e.g. zero scale).
				let Some((locations, was_closed)) =
					bezpath_algorithms::compute_sample_locations(&world_bezpath, spacing, amount, start_offset, stop_offset, adaptive_spacing, current_bezpath_segments_length)
				else {
					continue;
				};

				// Evaluate the sample locations on the untransformed bezpath and append the result
				let mut sample_bezpath = BezPath::new();
				for &(segment_index, t) in &locations {
					let segment = local_bezpath.get_seg(segment_index + 1).unwrap();
					let point = segment.eval(t);

					if sample_bezpath.elements().is_empty() {
						sample_bezpath.move_to(point);
					} else {
						sample_bezpath.line_to(point);
					}
				}
				if was_closed {
					sample_bezpath.close_path();
				}
				result.append_bezpath(sample_bezpath);
			}

			row.element = result;
			row
		})
		.collect()
}

/// Simplifies vector paths by reducing the number of curve segments while preserving the overall shape within the given tolerance.
#[node_macro::node(category("Vector: Modifier"), path(core_types::vector))]
async fn simplify(
	_: impl Ctx,
	/// The vector paths to simplify.
	content: Table<Vector>,
	/// The maximum distance the simplified path may deviate from the original.
	#[default(5.)]
	#[unit(" px")]
	tolerance: Length,
) -> Table<Vector> {
	if tolerance <= 0. {
		return content;
	}

	let options = SimplifyOptions::default();

	content
		.into_iter()
		.map(|mut row| {
			let transform = Affine::new(row.transform.to_cols_array());
			let inverse_transform = transform.inverse();

			let mut result = Vector {
				style: std::mem::take(&mut row.element.style),
				upstream_data: std::mem::take(&mut row.element.upstream_data),
				..Default::default()
			};

			for mut bezpath in row.element.stroke_bezpath_iter() {
				bezpath.apply_affine(transform);

				let mut simplified = simplify_bezpath(bezpath, tolerance, &options);

				simplified.apply_affine(inverse_transform);
				result.append_bezpath(simplified);
			}

			row.element = result;
			row
		})
		.collect()
}

/// Decimates vector paths into polylines by sampling any curves into line segments, then removing points that don't significantly contribute to the shape using the Ramer-Douglas-Peucker algorithm.
#[node_macro::node(category("Vector: Modifier"), path(core_types::vector))]
async fn decimate(
	_: impl Ctx,
	/// The vector paths to decimate.
	content: Table<Vector>,
	/// The maximum distance a point can deviate from the simplified path before it is kept.
	#[default(5.)]
	#[unit(" px")]
	tolerance: Length,
) -> Table<Vector> {
	// Tolerance of 0 means no simplification is possible, so return immediately
	if tolerance <= 0. {
		return content;
	}

	// Below this squared length, a line segment is treated as a degenerate point and the distance
	// falls back to a simple point-to-point measurement to avoid division by near-zero.
	const NEAR_ZERO_LENGTH_SQUARED: f64 = 1e-20;

	fn perpendicular_distance(point: DVec2, line_start: DVec2, line_end: DVec2) -> f64 {
		let line_vector = line_end - line_start;
		let line_length_squared = line_vector.length_squared();
		if line_length_squared < NEAR_ZERO_LENGTH_SQUARED {
			return point.distance(line_start);
		}
		(point - line_start).perp_dot(line_vector).abs() / line_length_squared.sqrt()
	}

	fn rdp_simplify(points: &[DVec2], tolerance: f64) -> Vec<DVec2> {
		if points.len() < 3 {
			return points.to_vec();
		}

		let mut keep = vec![false; points.len()];
		keep[0] = true;
		keep[points.len() - 1] = true;

		let mut stack = vec![(0, points.len() - 1)];

		while let Some((start_index, end_index)) = stack.pop() {
			let start = points[start_index];
			let end = points[end_index];

			let mut max_distance = 0.;
			let mut max_index = 0;

			for (i, &point) in points.iter().enumerate().take(end_index).skip(start_index + 1) {
				let distance = perpendicular_distance(point, start, end);
				if distance > max_distance {
					max_distance = distance;
					max_index = i;
				}
			}

			if max_distance > tolerance {
				keep[max_index] = true;
				if max_index - start_index > 1 {
					stack.push((start_index, max_index));
				}
				if end_index - max_index > 1 {
					stack.push((max_index, end_index));
				}
			}
		}

		points.iter().enumerate().filter(|(i, _)| keep[*i]).map(|(_, p)| *p).collect()
	}

	content
		.into_iter()
		.map(|mut row| {
			let transform = Affine::new(row.transform.to_cols_array());
			let inverse_transform = transform.inverse();

			let mut result = Vector {
				style: std::mem::take(&mut row.element.style),
				upstream_data: std::mem::take(&mut row.element.upstream_data),
				..Default::default()
			};

			for mut bezpath in row.element.stroke_bezpath_iter() {
				bezpath.apply_affine(transform);

				let is_closed = matches!(bezpath.elements().last(), Some(PathEl::ClosePath));

				// Flatten the bezpath into line segments, then collect the points
				let mut points = Vec::new();
				kurbo::flatten(bezpath, tolerance * 0.5, |el| match el {
					PathEl::MoveTo(p) | PathEl::LineTo(p) => {
						points.push(DVec2::new(p.x, p.y));
					}
					_ => {}
				});

				// For closed paths, the last point duplicates the first, so remove it
				if is_closed && points.len() > 1 && points.last() == points.first() {
					points.pop();
				}

				// Apply RDP simplification
				let simplified = rdp_simplify(&points, tolerance);
				if simplified.is_empty() {
					continue;
				}

				// Reconstruct as a polyline
				let mut new_bezpath = BezPath::new();
				new_bezpath.move_to((simplified[0].x, simplified[0].y));
				for &point in &simplified[1..] {
					new_bezpath.line_to((point.x, point.y));
				}
				if is_closed {
					new_bezpath.close_path();
				}

				new_bezpath.apply_affine(inverse_transform);
				result.append_bezpath(new_bezpath);
			}

			row.element = result;
			row
		})
		.collect()
}

/// Cuts a path at a given progression from 0 to 1 along the path, creating two new subpaths from the original one (if the path is initially open) or one open subpath (if the path is initially closed).
///
/// If multiple subpaths make up the path, the whole number part of the progression value selects the subpath and the decimal part determines the position along it.
#[node_macro::node(category("Vector: Modifier"), path(graphene_core::vector))]
async fn cut_path(
	_: impl Ctx,
	/// The path to insert a cut into.
	mut content: Table<Vector>,
	/// The factor from the start to the end of the path, 0–1 for one subpath, 1–2 for a second subpath, and so on.
	progression: Progression,
	/// Swap the direction of the path.
	reverse: bool,
	/// Traverse the path using each segment's Bézier curve parameterization instead of the Euclidean distance. Faster to compute but doesn't respect actual distances.
	parameterized_distance: bool,
) -> Table<Vector> {
	let euclidian = !parameterized_distance;

	let bezpaths = content
		.iter()
		.enumerate()
		.flat_map(|(row_index, vector)| vector.element.stroke_bezpath_iter().map(|bezpath| (row_index, bezpath)).collect::<Vec<_>>())
		.collect::<Vec<_>>();

	let bezpath_count = bezpaths.len() as f64;
	let t_value = progression.clamp(0., bezpath_count);
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
#[node_macro::node(category("Vector: Modifier"), path(core_types::vector))]
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

/// Determines the position of a point on the path, given by its progression from 0 to 1 along the path.
///
/// If multiple subpaths make up the path, the whole number part of the progression value selects the subpath and the decimal part determines the position along it.
#[node_macro::node(name("Position on Path"), category("Vector: Measure"), path(graphene_core::vector))]
async fn position_on_path(
	_: impl Ctx,
	/// The path to traverse.
	content: Table<Vector>,
	/// The factor from the start to the end of the path, 0–1 for one subpath, 1–2 for a second subpath, and so on.
	progression: Progression,
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
	let progression = progression.clamp(0., bezpath_count);
	let progression = if reverse { bezpath_count - progression } else { progression };
	let index = if progression >= bezpath_count { (bezpath_count - 1.) as usize } else { progression as usize };

	bezpaths.get_mut(index).map_or(DVec2::ZERO, |(bezpath, transform)| {
		let t = if progression == bezpath_count { 1. } else { progression.fract() };
		let t = if euclidian { TValue::Euclidean(t) } else { TValue::Parametric(t) };

		bezpath.apply_affine(Affine::new(transform.to_cols_array()));

		point_to_dvec2(evaluate_bezpath(bezpath, t, None))
	})
}

/// Determines the angle of the tangent at a point on the path, given by its progression from 0 to 1 along the path.
///
/// If multiple subpaths make up the path, the whole number part of the progression value selects the subpath and the decimal part determines the position along it.
#[node_macro::node(name("Tangent on Path"), category("Vector: Measure"), path(graphene_core::vector))]
async fn tangent_on_path(
	_: impl Ctx,
	/// The path to traverse.
	content: Table<Vector>,
	/// The factor from the start to the end of the path, 0–1 for one subpath, 1–2 for a second subpath, and so on.
	progression: Progression,
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
	let progression = progression.clamp(0., bezpath_count);
	let progression = if reverse { bezpath_count - progression } else { progression };
	let index = if progression >= bezpath_count { (bezpath_count - 1.) as usize } else { progression as usize };

	let angle = bezpaths.get_mut(index).map_or(0., |(bezpath, transform)| {
		let t = if progression == bezpath_count { 1. } else { progression.fract() };
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

#[node_macro::node(category(""), path(core_types::vector))]
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

#[node_macro::node(category(""), path(core_types::vector))]
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

#[node_macro::node(name("Spline"), category("Vector: Modifier"), path(core_types::vector))]
async fn spline(_: impl Ctx, content: Table<Vector>) -> Table<Vector> {
	content
		.into_iter()
		.filter_map(|mut row| {
			// Exit early if there are no points to generate splines from.
			if row.element.point_domain.positions().is_empty() {
				return None;
			}

			let mut segment_domain = SegmentDomain::default();
			let mut next_id = SegmentId::ZERO;
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

					segment_domain.push(next_id.next_id(), start_index, end_index, handles, stroke_id);
				}
			}

			row.element.segment_domain = segment_domain;
			Some(row)
		})
		.collect()
}

/// Computes the inverse of a transform's linear (matrix2) part, handling singular transforms
/// (e.g. zero scale on one axis) by replacing the collapsed axis with a unit perpendicular
/// so offsets still apply there (visible if the transform is later replaced).
fn inverse_linear_or_repair(linear: DMat2) -> DMat2 {
	if linear.determinant() != 0. {
		return linear.inverse();
	}

	let col0 = linear.col(0);
	let col1 = linear.col(1);
	let col0_exists = col0.length_squared() > (f64::EPSILON * 1e3).powi(2);
	let col1_exists = col1.length_squared() > (f64::EPSILON * 1e3).powi(2);

	let repaired = match (col0_exists, col1_exists) {
		(true, _) => DMat2::from_cols(col0, col0.perp().normalize()),
		(false, true) => DMat2::from_cols(col1.perp().normalize(), col1),
		(false, false) => DMat2::IDENTITY,
	};
	repaired.inverse()
}

/// Applies per-point displacement deltas to the point and handle positions of a vector element.
fn apply_point_deltas(element: &mut Vector, deltas: &[DVec2], transform: DAffine2) {
	let mut already_applied = vec![false; element.point_domain.positions().len()];

	for (handles, start, end) in element.segment_domain.handles_and_points_mut() {
		let start_delta = deltas[*start];
		let end_delta = deltas[*end];

		if !already_applied[*start] {
			let start_position = element.point_domain.positions()[*start];
			element.point_domain.set_position(*start, start_position + start_delta);
			already_applied[*start] = true;
		}
		if !already_applied[*end] {
			let end_position = element.point_domain.positions()[*end];
			element.point_domain.set_position(*end, end_position + end_delta);
			already_applied[*end] = true;
		}

		match handles {
			BezierHandles::Cubic { handle_start, handle_end } => {
				*handle_start += start_delta;
				*handle_end += end_delta;
			}
			BezierHandles::Quadratic { handle } => {
				*handle = transform.transform_point2(*handle) + (start_delta + end_delta) / 2.;
			}
			BezierHandles::Linear => {}
		}
	}
}

/// Perturbs the positions of anchor points in vector geometry by random amounts and directions.
#[node_macro::node(category("Vector: Modifier"), path(core_types::vector))]
async fn jitter_points(
	_: impl Ctx,
	/// The vector geometry with points to be jittered.
	content: Table<Vector>,
	/// The maximum extent of the random distance each point can be offset.
	#[default(5.)]
	#[unit(" px")]
	max_distance: f64,
	/// Seed used to determine unique variations on all randomized offsets.
	seed: SeedValue,
	/// Whether to offset anchor points along their normal direction (perpendicular to the path) or in a random direction. Free-floating and branching points have no normal direction, so they receive a random-angled offset regardless of this setting.
	#[default(true)]
	along_normals: bool,
) -> Table<Vector> {
	content
		.into_iter()
		.map(|mut row| {
			let mut rng = rand::rngs::StdRng::seed_from_u64(seed.into());
			let inverse_linear = inverse_linear_or_repair(row.transform.matrix2);

			let deltas: Vec<_> = (0..row.element.point_domain.positions().len())
				.map(|point_index| {
					let normal = if along_normals {
						row.element.segment_domain.point_tangent(point_index, row.element.point_domain.positions()).map(|t| -t.perp())
					} else {
						None
					};

					let offset = if let Some(normal) = normal {
						normal * (rng.random::<f64>() * 2. - 1.)
					} else {
						DVec2::from_angle(rng.random::<f64>() * TAU) * rng.random::<f64>()
					};

					inverse_linear * offset * max_distance
				})
				.collect();

			apply_point_deltas(&mut row.element, &deltas, row.transform);

			row
		})
		.collect()
}

/// Displaces anchor points along their normal direction (perpendicular to the path) by a set distance.
/// Points with 0 or 3+ segment connections have no well-defined normal and are left in place.
#[node_macro::node(category("Vector: Modifier"), path(core_types::vector))]
async fn offset_points(
	_: impl Ctx,
	/// The vector geometry with points to be offset.
	content: Table<Vector>,
	/// The distance to offset each anchor point along its normal. Positive values move outward, negative values move inward.
	#[default(10.)]
	#[unit(" px")]
	distance: f64,
) -> Table<Vector> {
	content
		.into_iter()
		.map(|mut row| {
			let inverse_linear = inverse_linear_or_repair(row.transform.matrix2);

			let deltas: Vec<_> = (0..row.element.point_domain.positions().len())
				.map(|point_index| {
					let Some(normal) = row.element.segment_domain.point_tangent(point_index, row.element.point_domain.positions()).map(|t| -t.perp()) else {
						return DVec2::ZERO;
					};

					inverse_linear * normal * distance
				})
				.collect();

			apply_point_deltas(&mut row.element, &deltas, row.transform);

			row
		})
		.collect()
}

/// Interpolates the geometry, appearance, and transform between multiple vector layers, producing a single morphed vector shape.
///
/// *Progression* morphs through all objects. Interpolation is linear unless *Path* geometry is provided to control the trajectory between key objects. The **Origins to Polyline** node may be used to create a path with anchor points corresponding to each object. Other nodes can modify its path segments.
#[node_macro::node(category("Vector: Modifier"), path(core_types::vector))]
async fn morph<I: IntoGraphicTable + 'n + Send + Clone>(
	_: impl Ctx,
	/// The vector objects to interpolate between. Mixed graphic content is deeply flattened to keep only vector elements.
	#[implementations(Table<Graphic>, Table<Vector>)]
	content: I,
	/// The fractional part `[0, 1)` traverses the morph uniformly along the path. If the control path has multiple subpaths, each added integer selects the next subpath.
	progression: Progression,
	/// Swap the direction of the progression between objects or along the control path.
	reverse: bool,
	/// The parameter of change that influences the interpolation speed between each object. Equal slices in this parameter correspond to the rate of progression through the morph. This must be set to a parameter that changes.
	///
	/// "Objects" morphs through each group element at an equal rate. "Distances" keeps constant speed with time between objects proportional to their distances. "Angles" keeps constant rotational speed. "Sizes" keeps constant shrink/growth speed. "Slants" keeps constant shearing angle speed.
	distribution: InterpolationDistribution,
	/// An optional control path whose anchor points correspond to each object. Curved segments between points will shape the morph trajectory instead of traveling straight. If there is a break between path segments, the separate subpaths are selected by index from the integer part of the progression value. For example, `[1, 2)` morphs along the segments of the second subpath, and so on.
	path: Table<Vector>,
) -> Table<Vector> {
	/// Promotes a segment's handle pair to cubic-equivalent Bézier control points.
	/// For linear segments (both None), handles are placed at their respective anchors (zero-length)
	/// so that interpolation against another zero-length cubic doesn't introduce unwanted curvature.
	/// For quadratic segments (one handle), degree elevation is applied.
	fn promote_handles_to_cubic(prev_anchor: DVec2, out_handle: Option<DVec2>, in_handle: Option<DVec2>, curr_anchor: DVec2) -> (DVec2, DVec2) {
		match (out_handle, in_handle) {
			(Some(handle_start), Some(handle_end)) => (handle_start, handle_end),
			(None, None) => (prev_anchor, curr_anchor),
			(Some(handle), None) | (None, Some(handle)) => {
				let handle_start = prev_anchor + (handle - prev_anchor) * (2. / 3.);
				let handle_end = curr_anchor + (handle - curr_anchor) * (2. / 3.);
				(handle_start, handle_end)
			}
		}
	}

	/// Subdivides the last segment of a manipulator group list at its midpoint, adding one new manipulator.
	/// For closed paths, the "last segment" is the closing segment from the last back to the first manipulator.
	fn subdivide_last_manipulator_segment(manips: &mut Vec<ManipulatorGroup<PointId>>, closed: bool) {
		let len = manips.len();
		if len < 2 {
			return;
		}

		let (prev_index, next_index) = if closed { (len - 1, 0) } else { (len - 2, len - 1) };

		let prev_anchor = manips[prev_index].anchor;
		let next_anchor = manips[next_index].anchor;
		let (h1, h2) = promote_handles_to_cubic(prev_anchor, manips[prev_index].out_handle, manips[next_index].in_handle, next_anchor);

		// De Casteljau subdivision at t=0.5
		let m01 = prev_anchor.lerp(h1, 0.5);
		let m12 = h1.lerp(h2, 0.5);
		let m23 = h2.lerp(next_anchor, 0.5);
		let m012 = m01.lerp(m12, 0.5);
		let m123 = m12.lerp(m23, 0.5);
		let mid = m012.lerp(m123, 0.5);

		manips[prev_index].out_handle = Some(m01);
		manips[next_index].in_handle = Some(m23);

		let mid_manip = ManipulatorGroup {
			anchor: mid,
			in_handle: Some(m012),
			out_handle: Some(m123),
			id: PointId::ZERO,
		};

		if closed {
			manips.push(mid_manip);
		} else {
			manips.insert(next_index, mid_manip);
		}
	}

	/// Constructs BezierHandles from the out_handle of one manipulator and in_handle of the next.
	fn handles_from_manips(out_handle: Option<DVec2>, in_handle: Option<DVec2>) -> BezierHandles {
		match (out_handle, in_handle) {
			(Some(handle_start), Some(handle_end)) => BezierHandles::Cubic { handle_start, handle_end },
			(None, None) => BezierHandles::Linear,
			(Some(handle), None) | (None, Some(handle)) => BezierHandles::Quadratic { handle },
		}
	}

	/// Pushes a subpath (list of manipulators) directly into a Vector's point, segment, and region domains,
	/// bypassing the BezPath intermediate representation used by `append_bezpath`.
	fn push_manipulators_to_vector(vector: &mut Vector, manips: &[ManipulatorGroup<PointId>], closed: bool, point_id: &mut PointId, segment_id: &mut SegmentId) {
		let Some(first) = manips.first() else { return };

		let first_point_index = vector.point_domain.ids().len();
		vector.point_domain.push_unchecked(point_id.next_id(), first.anchor);
		let mut prev_point_index = first_point_index;
		let mut first_segment_id = None;

		for manip_window in manips.windows(2) {
			let point_index = vector.point_domain.ids().len();
			vector.point_domain.push_unchecked(point_id.next_id(), manip_window[1].anchor);

			let handles = handles_from_manips(manip_window[0].out_handle, manip_window[1].in_handle);
			let seg_id = segment_id.next_id();
			first_segment_id.get_or_insert(seg_id);
			vector.segment_domain.push_unchecked(seg_id, prev_point_index, point_index, handles, StrokeId::ZERO);

			prev_point_index = point_index;
		}

		if closed && manips.len() > 1 {
			let handles = handles_from_manips(manips.last().unwrap().out_handle, manips[0].in_handle);
			let closing_seg_id = segment_id.next_id();
			first_segment_id.get_or_insert(closing_seg_id);
			vector.segment_domain.push_unchecked(closing_seg_id, prev_point_index, first_point_index, handles, StrokeId::ZERO);

			let region_id = vector.region_domain.next_id();
			vector.region_domain.push_unchecked(region_id, first_segment_id.unwrap()..=closing_seg_id, FillId::ZERO);
		}
	}

	// Preserve original graphic table as upstream data so this group layer's nested layers can be edited by the tools.
	let mut graphic_table_content = content.clone().into_graphic_table();

	// If the input isn't a Table<Vector>, we convert it into one by flattening any Table<Graphic> content.
	let content = content.into_flattened_table::<Vector>();

	// Not enough elements to interpolate between, so we return the input as-is
	if content.len() <= 1 {
		return content;
	}

	// Build the control path for the morph trajectory.
	// Collect all subpaths from the path input (applying transforms), or build a default polyline from element origins.
	let default_polyline = || {
		let mut default_path = BezPath::new();
		for (i, row) in content.iter().enumerate() {
			let origin = row.transform.translation;
			let point = kurbo::Point::new(origin.x, origin.y);
			if i == 0 {
				default_path.move_to(point);
			} else {
				default_path.line_to(point);
			}
		}
		vec![default_path]
	};

	let control_bezpaths: Vec<BezPath> = if path.is_empty() {
		default_polyline()
	} else {
		// User-provided path: collect all subpaths with transforms applied
		let paths: Vec<BezPath> = path
			.iter()
			.flat_map(|vector| {
				let transform = *vector.transform;
				vector.element.stroke_bezpath_iter().map(move |mut bezpath| {
					bezpath.apply_affine(Affine::new(transform.to_cols_array()));
					bezpath
				})
			})
			.collect();

		// Fall back to default polyline if the user-provided path has no subpaths
		if paths.is_empty() { default_polyline() } else { paths }
	};

	// Select which subpath to use based on the integer part of progression (like the 'Position on Path' node)
	let progression = progression.max(0.);
	let subpath_count = control_bezpaths.len() as f64;
	let progression = if reverse { subpath_count - progression } else { progression };
	let clamped_progression = progression.clamp(0., subpath_count);
	let subpath_index = if clamped_progression >= subpath_count { subpath_count - 1. } else { clamped_progression } as usize;
	let fractional_progression = if clamped_progression >= subpath_count { 1. } else { clamped_progression.fract() };

	let control_bezpath = &control_bezpaths[subpath_index];
	let segment_count = control_bezpath.segments().count();

	// If the control path has no segments, return the first element
	if segment_count == 0 {
		return content.into_iter().next().into_iter().collect();
	}

	// Determine if the selected subpath is closed (has a closing segment connecting its end back to its start)
	let is_closed = control_bezpath.elements().last() == Some(&PathEl::ClosePath);

	// Number of anchor points (content elements) per subpath: for closed subpaths, the closing
	// segment doesn't add a new anchor, so anchors = segments. For open: anchors = segments + 1.
	let anchor_count = |bp: &BezPath| -> usize {
		let segs = bp.segments().count();
		let closed = bp.elements().last() == Some(&PathEl::ClosePath);
		if closed { segs } else { segs + 1 }
	};

	// Offset source_index by the number of content elements consumed by previous subpaths,
	// so each subpath morphs through its own slice of content (not always starting from element 0).
	let content_offset: usize = control_bezpaths[..subpath_index].iter().map(&anchor_count).sum();
	let subpath_anchors = anchor_count(control_bezpath);
	let max_content_index = content.len().saturating_sub(1);

	// Map the fractional progression to a segment index and local blend time using the chosen weights.
	let (local_source_index, time) = if fractional_progression >= 1. {
		(segment_count - 1, 1.)
	} else if matches!(distribution, InterpolationDistribution::Objects) {
		// Fast path for uniform distribution: direct index calculation without allocation or iteration
		let scaled = fractional_progression * segment_count as f64;
		let index = (scaled.ceil() as usize).saturating_sub(1);
		(index, scaled - index as f64)
	} else {
		// Compute segment weights based on the user's chosen spacing metric
		let segment_weights: Vec<f64> = match distribution {
			InterpolationDistribution::Objects => unreachable!(),
			InterpolationDistribution::Distances => control_bezpath.segments().map(|seg| seg.perimeter(DEFAULT_ACCURACY)).collect(),
			InterpolationDistribution::Angles | InterpolationDistribution::Sizes | InterpolationDistribution::Slants => (0..segment_count)
				.map(|i| {
					let source_index = (content_offset + i).min(max_content_index);
					let target_index = if is_closed && i >= subpath_anchors - 1 {
						content_offset
					} else {
						(content_offset + i + 1).min(max_content_index)
					};

					let (Some(source), Some(target)) = (content.get(source_index), content.get(target_index)) else {
						return 0.;
					};
					let (s_angle, s_scale, s_skew) = source.transform.decompose_rotation_scale_skew();
					let (t_angle, t_scale, t_skew) = target.transform.decompose_rotation_scale_skew();

					match distribution {
						InterpolationDistribution::Angles => {
							let mut diff = t_angle - s_angle;
							if diff > PI {
								diff -= TAU;
							} else if diff < -PI {
								diff += TAU;
							}
							diff.abs()
						}
						InterpolationDistribution::Sizes => (t_scale - s_scale).length(),
						InterpolationDistribution::Slants => (t_skew.atan() - s_skew.atan()).abs(),
						_ => unreachable!(),
					}
				})
				.collect(),
		};

		let total_weight: f64 = segment_weights.iter().sum();

		// When all weights are zero (all elements identical in the chosen metric), there's zero interval to traverse.
		if total_weight <= f64::EPSILON {
			(0, 0.)
		} else {
			let mut accumulator = 0.;
			let mut found_index = segment_count - 1;
			let mut found_t = 1.;
			for (i, weight) in segment_weights.iter().enumerate() {
				let ratio = weight / total_weight;
				if fractional_progression <= accumulator + ratio {
					found_index = i;
					found_t = if ratio > f64::EPSILON { (fractional_progression - accumulator) / ratio } else { 0. };
					break;
				}
				accumulator += ratio;
			}
			(found_index, found_t)
		}
	};

	// Convert the blend time to a parametric t for evaluating spatial position on the control path
	let path_segment_index = local_source_index;
	let parametric_t = {
		let segment_index = path_segment_index.min(segment_count - 1);
		let segment = control_bezpath.get_seg(segment_index + 1).unwrap();
		eval_pathseg_euclidean(segment, time, DEFAULT_ACCURACY)
	};

	let source_index = local_source_index + content_offset;

	// For closed subpaths, the closing segment wraps target back to the first element of this subpath's slice.
	// For open subpaths, target is simply the next element.
	let target_index = if is_closed && local_source_index >= subpath_anchors - 1 {
		content_offset // Wrap to first element of this subpath's slice
	} else {
		source_index + 1
	};

	// Clamp to valid content range
	let source_index = source_index.min(max_content_index);
	let target_index = target_index.min(max_content_index);

	// Use indexed access to borrow only the two rows we need, avoiding collecting the entire table
	let (Some(source_row), Some(target_row)) = (content.get(source_index), content.get(target_index)) else {
		return content;
	};

	// Lerp styles
	let vector_alpha_blending = source_row.alpha_blending.lerp(target_row.alpha_blending, time as f32);

	// Evaluate the spatial position on the control path for the translation component.
	// When the segment has zero arc length (e.g., two objects at the same position), inv_arclen
	// produces NaN (0/0), so we fall back to the segment start point to avoid NaN translation.
	let path_position = {
		let segment_index = path_segment_index.min(segment_count - 1);
		let segment = control_bezpath.get_seg(segment_index + 1).unwrap();
		let parametric_t = if segment.arclen(DEFAULT_ACCURACY) < f64::EPSILON { 0. } else { parametric_t };
		let point = segment.eval(parametric_t);
		DVec2::new(point.x, point.y)
	};

	// Interpolate rotation, scale, and skew between source and target, but use the path position for translation.
	// This decomposition must match the one used in Stroke::lerp so the renderer's stroke_transform.inverse()
	// correctly cancels the element transform, keeping the stroke uniform when Stroke is after Transform.
	let lerped_transform = {
		let (s_angle, s_scale, s_skew) = source_row.transform.decompose_rotation_scale_skew();
		let (t_angle, t_scale, t_skew) = target_row.transform.decompose_rotation_scale_skew();

		let lerp = |a: f64, b: f64| a + (b - a) * time;

		// Shortest-arc rotation interpolation
		let mut rotation_diff = t_angle - s_angle;
		if rotation_diff > PI {
			rotation_diff -= TAU;
		} else if rotation_diff < -PI {
			rotation_diff += TAU;
		}
		let lerped_angle = s_angle + rotation_diff * time;

		let trs = DAffine2::from_scale_angle_translation(s_scale.lerp(t_scale, time), lerped_angle, path_position);
		let skew = DAffine2::from_cols_array(&[1., 0., lerp(s_skew, t_skew), 1., 0., 0.]);
		trs * skew
	};

	// Pre-compensate upstream_data transforms so that when collect_metadata applies
	// the row transform (which will be group_transform * lerped_transform after the
	// pipeline's Transform node runs), the lerped_transform cancels out and children
	// get the correct footprint: parent * group_transform * child_transform.
	// Only pre-compensate if the lerped transform is invertible (non-zero determinant).
	// A zero determinant can occur when interpolated scale passes through zero (e.g., flipped axes),
	// in which case we skip pre-compensation to avoid propagating NaN through upstream_data transforms.
	if lerped_transform.matrix2.determinant().abs() > f64::EPSILON {
		let lerped_inverse = lerped_transform.inverse();
		for row in graphic_table_content.iter_mut() {
			*row.transform = lerped_inverse * *row.transform;
		}
	}

	// Fast path: when exactly at either endpoint, clone the corresponding geometry directly
	// instead of extracting manipulator groups, subdividing, interpolating, and rebuilding.
	if time == 0. || time == 1. {
		let row = if time == 0. { source_row } else { target_row };
		return Table::new_from_row(TableRow {
			element: Vector {
				upstream_data: Some(graphic_table_content),
				..row.element.clone()
			},
			alpha_blending: *row.alpha_blending,
			transform: lerped_transform,
			..Default::default()
		});
	}

	let mut vector = Vector {
		upstream_data: Some(graphic_table_content),
		..Default::default()
	};
	vector.style = source_row.element.style.lerp(&target_row.element.style, time);

	// Work directly with manipulator groups, bypassing the BezPath intermediate representation.
	// This avoids the full Vector → BezPath → interpolate → BezPath → Vector roundtrip each frame.
	let mut source_subpaths: Vec<_> = source_row.element.stroke_manipulator_groups().collect();
	let mut target_subpaths: Vec<_> = target_row.element.stroke_manipulator_groups().collect();

	// Interpolate geometry in local space (no transform baked in) — the lerped transform handles positioning
	let matched_count = source_subpaths.len().min(target_subpaths.len());
	let extra_source = source_subpaths.split_off(matched_count);
	let extra_target = target_subpaths.split_off(matched_count);

	// Pre-allocate domain storage based on total manipulator counts across all subpaths
	let mut total_points = 0;
	let mut total_segments = 0;
	let mut total_regions = 0;
	for ((source_manips, source_closed), (target_manips, _)) in source_subpaths.iter().zip(target_subpaths.iter()) {
		if source_manips.is_empty() || target_manips.is_empty() {
			continue;
		}
		let manip_count = source_manips.len().max(target_manips.len());
		total_points += manip_count;
		total_segments += if *source_closed { manip_count } else { manip_count.saturating_sub(1) };
		if *source_closed {
			total_regions += 1;
		}
	}
	for (manips, closed) in extra_source.iter().chain(extra_target.iter()) {
		total_points += manips.len();
		total_segments += if *closed { manips.len() } else { manips.len().saturating_sub(1) };
		if *closed {
			total_regions += 1;
		}
	}
	vector.point_domain.reserve(total_points);
	vector.segment_domain.reserve(total_segments);
	vector.region_domain.reserve(total_regions);

	let mut point_id = PointId::ZERO;
	let mut segment_id = SegmentId::ZERO;

	for ((mut source_manips, source_closed), (mut target_manips, target_closed)) in source_subpaths.into_iter().zip(target_subpaths) {
		if source_manips.is_empty() || target_manips.is_empty() {
			continue;
		}

		// Align manipulator counts by subdividing the last segment of the shorter subpath
		let source_count = source_manips.len();
		let target_count = target_manips.len();
		for _ in 0..target_count.saturating_sub(source_count) {
			subdivide_last_manipulator_segment(&mut source_manips, source_closed);
		}
		for _ in 0..source_count.saturating_sub(target_count) {
			subdivide_last_manipulator_segment(&mut target_manips, target_closed);
		}

		// Build interpolated manipulator groups
		let mut interpolated: Vec<ManipulatorGroup<PointId>> = source_manips
			.iter()
			.zip(target_manips.iter())
			.map(|(s, t)| ManipulatorGroup {
				anchor: s.anchor.lerp(t.anchor, time),
				in_handle: None,
				out_handle: None,
				id: PointId::ZERO,
			})
			.collect();

		// Interpolate handles per segment, preserving handle type when source and target match
		let segment_count = if source_closed { source_manips.len() } else { source_manips.len().saturating_sub(1) };
		for segment_index in 0..segment_count {
			let next_index = (segment_index + 1) % source_manips.len();

			let source_out = source_manips[segment_index].out_handle;
			let source_in = source_manips[next_index].in_handle;
			let target_out = target_manips[segment_index].out_handle;
			let target_in = target_manips[next_index].in_handle;

			match (source_out, source_in, target_out, target_in) {
				// Both linear — no handles needed
				(None, None, None, None) => {}
				// Both cubic — lerp handle pairs directly
				(Some(s_out), Some(s_in), Some(t_out), Some(t_in)) => {
					interpolated[segment_index].out_handle = Some(s_out.lerp(t_out, time));
					interpolated[next_index].in_handle = Some(s_in.lerp(t_in, time));
				}
				// Both quadratic with handle in the same position — lerp the single handle
				(Some(s_out), None, Some(t_out), None) => {
					interpolated[segment_index].out_handle = Some(s_out.lerp(t_out, time));
				}
				(None, Some(s_in), None, Some(t_in)) => {
					interpolated[next_index].in_handle = Some(s_in.lerp(t_in, time));
				}
				// Linear vs. quadratic — elevate the linear side to a zero-length quadratic in the matching position
				(None, None, Some(t_out), None) => {
					interpolated[segment_index].out_handle = Some(source_manips[segment_index].anchor.lerp(t_out, time));
				}
				(None, None, None, Some(t_in)) => {
					interpolated[next_index].in_handle = Some(source_manips[next_index].anchor.lerp(t_in, time));
				}
				(Some(s_out), None, None, None) => {
					interpolated[segment_index].out_handle = Some(s_out.lerp(target_manips[segment_index].anchor, time));
				}
				(None, Some(s_in), None, None) => {
					interpolated[next_index].in_handle = Some(s_in.lerp(target_manips[next_index].anchor, time));
				}
				// Mismatched types — promote both to cubic and lerp
				_ => {
					let (s_h1, s_h2) = promote_handles_to_cubic(source_manips[segment_index].anchor, source_out, source_in, source_manips[next_index].anchor);
					let (t_h1, t_h2) = promote_handles_to_cubic(target_manips[segment_index].anchor, target_out, target_in, target_manips[next_index].anchor);
					interpolated[segment_index].out_handle = Some(s_h1.lerp(t_h1, time));
					interpolated[next_index].in_handle = Some(s_h2.lerp(t_h2, time));
				}
			}
		}

		push_manipulators_to_vector(&mut vector, &interpolated, source_closed, &mut point_id, &mut segment_id);
	}

	// Deal with unmatched extra source subpaths by collapsing them toward their end point
	for (mut manips, closed) in extra_source {
		let Some(end) = manips.last().map(|m| m.anchor) else { continue };

		for manip in &mut manips {
			manip.anchor = manip.anchor.lerp(end, time);
			manip.in_handle = manip.in_handle.map(|h| h.lerp(end, time));
			manip.out_handle = manip.out_handle.map(|h| h.lerp(end, time));
		}

		push_manipulators_to_vector(&mut vector, &manips, closed, &mut point_id, &mut segment_id);
	}

	// Deal with unmatched extra target subpaths by expanding them from their start point
	for (mut manips, closed) in extra_target {
		let Some(start) = manips.first().map(|m| m.anchor) else { continue };

		for manip in &mut manips {
			manip.anchor = start.lerp(manip.anchor, time);
			manip.in_handle = manip.in_handle.map(|h| start.lerp(h, time));
			manip.out_handle = manip.out_handle.map(|h| start.lerp(h, time));
		}

		push_manipulators_to_vector(&mut vector, &manips, closed, &mut point_id, &mut segment_id);
	}

	Table::new_from_row(TableRow {
		element: vector,
		transform: lerped_transform,
		alpha_blending: vector_alpha_blending,
		..Default::default()
	})
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

		// Zero out points without exactly two connectors. These are ignored.
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

	fn calculate_distance_to_split(bezier1: PathSeg, bezier2: PathSeg, bevel_length: f64) -> f64 {
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

			let calculated_split_distance = calculate_distance_to_split(bezier, next_bezier, distance);

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
				let distance = calculated_split_distance.min(original_length.min(next_original_length) / 2.);
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
				let distance = calculated_split_distance.min(next_original_length.min(original_length) / 2.);
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

#[node_macro::node(category("Vector: Modifier"), path(core_types::vector))]
fn bevel(_: impl Ctx, source: Table<Vector>, #[default(10.)] distance: Length) -> Table<Vector> {
	source
		.into_iter()
		.map(|row| TableRow {
			element: bevel_algorithm(row.element, row.transform, distance),
			..row
		})
		.collect()
}

#[node_macro::node(category("Vector: Modifier"), path(core_types::vector))]
fn close_path(_: impl Ctx, source: Table<Vector>) -> Table<Vector> {
	source
		.into_iter()
		.map(|mut row| {
			row.element.close_subpaths();
			row
		})
		.collect()
}

#[node_macro::node(category("Vector: Measure"), path(core_types::vector))]
fn point_inside(_: impl Ctx, source: Table<Vector>, point: DVec2) -> bool {
	source.into_iter().any(|row| row.element.check_point_inside_shape(row.transform, point))
}

trait Count {
	fn count(&self) -> usize;
}
impl<T> Count for Table<T> {
	fn count(&self) -> usize {
		self.len()
	}
}
impl<T> Count for Vec<T> {
	fn count(&self) -> usize {
		self.len()
	}
}

// TODO: Return u32, u64, or usize instead of f64 after #1621 is resolved and has allowed us to implement automatic type conversion in the node graph for nodes with generic type inputs.
// TODO: (Currently automatic type conversion only works for concrete types, via the Graphene preprocessor and not the full Graphene type system.)
#[node_macro::node(category("General"), path(graphene_core::vector))]
async fn count_elements<I: Count>(
	_: impl Ctx,
	#[implementations(
		Table<Graphic>,
		Table<Vector>,
		Table<Raster<CPU>>,
		Table<Raster<GPU>>,
		Table<Color>,
		Table<GradientStops>,
		Vec<String>,
		Vec<f64>,
		Vec<DVec2>,
	)]
	content: I,
) -> f64 {
	content.count() as f64
}

#[node_macro::node(category("Vector: Measure"), path(graphene_core::vector))]
async fn count_points(_: impl Ctx, content: Table<Vector>) -> f64 {
	content.into_iter().map(|row| row.element.point_domain.positions().len() as f64).sum()
}

/// Retrieves the vec2 position (in local space) of the anchor point at the specified index in table of vector elements.
/// If no value exists at that index, the position (0, 0) is returned.
#[node_macro::node(category("Vector: Measure"), path(graphene_core::vector))]
async fn index_points(
	_: impl Ctx,
	/// The vector element or elements containing the anchor points to be retrieved.
	content: Table<Vector>,
	/// The index of the points to retrieve, starting from 0 for the first point. Negative indices count backwards from the end, starting from -1 for the last item.
	index: f64,
) -> DVec2 {
	let points_count = content.iter().map(|row| row.element.point_domain.positions().len()).sum::<usize>();

	if points_count == 0 {
		return DVec2::ZERO;
	}
	// Clamp and allow negative indexing from the end
	let index = index as isize;
	let index = if index < 0 {
		(points_count as isize + index).max(0) as usize
	} else {
		(index as usize).min(points_count - 1)
	};

	// Find the point at the given index across all vector elements
	let mut accumulated = 0;
	for row in content.iter() {
		let row_point_count = row.element.point_domain.positions().len();
		if index - accumulated < row_point_count {
			return row.element.point_domain.positions()[index - accumulated];
		}
		accumulated += row_point_count;
	}

	DVec2::ZERO
}

#[node_macro::node(category("Vector: Measure"), path(core_types::vector))]
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

#[node_macro::node(category("Vector: Measure"), path(core_types::vector))]
async fn area(ctx: impl Ctx + CloneVarArgs + ExtractAll, content: impl Node<Context<'static>, Output = Table<Vector>>) -> f64 {
	let new_ctx = OwnedContextImpl::from(ctx).with_footprint(Footprint::default()).into_context();
	let vector = content.eval(new_ctx).await;

	vector
		.iter()
		.map(|row| {
			let area_scale = row.transform.matrix2.determinant().abs();
			row.element.stroke_bezpath_iter().map(|subpath| subpath.area() * area_scale).sum::<f64>()
		})
		.sum()
}

#[node_macro::node(category("Vector: Measure"), path(core_types::vector))]
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
	use core_types::Node;
	use kurbo::{CubicBez, Ellipse, Point, Rect};
	use std::future::Future;
	use std::pin::Pin;
	use vector_types::vector::algorithms::bezpath_algorithms::{TValue, trim_pathseg};
	use vector_types::vector::misc::pathseg_abs_diff_eq;

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
		let mut rectangles = vector_node_from_bezpath(Rect::new(0., 0., 100., 100.).to_path(DEFAULT_ACCURACY));
		let mut second_rectangle = rectangles.get(0).unwrap().into_cloned();
		second_rectangle.transform *= DAffine2::from_translation((-100., -100.).into());
		rectangles.push(second_rectangle);

		let morphed = super::morph(Footprint::default(), rectangles, 0.5, false, InterpolationDistribution::default(), Table::default()).await;
		let row = morphed.iter().next().unwrap();
		// Geometry stays in local space (original rectangle coordinates)
		assert_eq!(
			&row.element.point_domain.positions()[..4],
			vec![DVec2::new(0., 0.), DVec2::new(100., 0.), DVec2::new(100., 100.), DVec2::new(0., 100.)]
		);
		// The interpolated transform carries the midpoint translation (approximate due to arc-length parameterization)
		assert!((row.transform.translation - DVec2::new(-50., -50.)).length() < 1e-3);
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

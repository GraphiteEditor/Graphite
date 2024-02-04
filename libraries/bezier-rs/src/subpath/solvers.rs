use super::*;
use crate::consts::MAX_ABSOLUTE_DIFFERENCE;
use crate::utils::{compute_circular_subpath_details, line_intersection, SubpathTValue};
use crate::TValue;

use glam::{DAffine2, DMat2, DVec2};
use std::f64::consts::PI;

impl<ManipulatorGroupId: crate::Identifier> Subpath<ManipulatorGroupId> {
	/// Calculate the point on the subpath based on the parametric `t`-value provided.
	/// Expects `t` to be within the inclusive range `[0, 1]`.
	/// <iframe frameBorder="0" width="100%" height="350px" src="https://graphite.rs/libraries/bezier-rs#subpath/evaluate/solo" title="Evaluate Demo"></iframe>
	pub fn evaluate(&self, t: SubpathTValue) -> DVec2 {
		let (segment_index, t) = self.t_value_to_parametric(t);
		self.get_segment(segment_index).unwrap().evaluate(TValue::Parametric(t))
	}

	/// Calculates the intersection points the subpath has with a given curve and returns a list of `(usize, f64)` tuples,
	/// where the `usize` represents the index of the curve in the subpath, and the `f64` represents the `t`-value local to
	/// that curve where the intersection occurred.
	/// Expects the following:
	/// - `other`: a [Bezier] curve to check intersections against
	/// - `error`: an optional f64 value to provide an error bound
	/// - `minimum_separation`: the minimum difference two adjacent `t`-values must have when comparing adjacent `t`-values in sorted order.
	/// If the comparison condition is not satisfied, the function takes the larger `t`-value of the two.
	/// <iframe frameBorder="0" width="100%" height="375px" src="https://graphite.rs/libraries/bezier-rs#subpath/intersect-linear/solo" title="Intersection Demo"></iframe>
	///
	/// <iframe frameBorder="0" width="100%" height="375px" src="https://graphite.rs/libraries/bezier-rs#subpath/intersect-quadratic/solo" title="Intersection Demo"></iframe>
	///
	/// <iframe frameBorder="0" width="100%" height="375px" src="https://graphite.rs/libraries/bezier-rs#subpath/intersect-cubic/solo" title="Intersection Demo"></iframe>
	pub fn intersections(&self, other: &Bezier, error: Option<f64>, minimum_separation: Option<f64>) -> Vec<(usize, f64)> {
		self.iter()
			.enumerate()
			.flat_map(|(index, bezier)| bezier.intersections(other, error, minimum_separation).into_iter().map(|t| (index, t)).collect::<Vec<(usize, f64)>>())
			.collect()
	}

	/// Calculates the intersection points the subpath has with another given subpath and returns a list of global parametric `t`-values.
	/// This function expects the following:
	/// - other: a [Bezier] curve to check intersections against
	/// - error: an optional f64 value to provide an error bound
	pub fn subpath_intersections(&self, other: &Subpath<ManipulatorGroupId>, error: Option<f64>, minimum_separation: Option<f64>) -> Vec<(usize, f64)> {
		let mut intersection_t_values: Vec<(usize, f64)> = other.iter().flat_map(|bezier| self.intersections(&bezier, error, minimum_separation)).collect();
		intersection_t_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
		intersection_t_values
	}

	/// Returns how many times a given ray intersects with this subpath. (`ray_direction` does not need to be normalized.)
	/// If this needs to be called frequently with a ray of the same rotation angle, consider instead using [`ray_test_crossings_count_prerotated`].
	pub fn ray_test_crossings_count(&self, ray_start: DVec2, ray_direction: DVec2) -> usize {
		self.iter().map(|bezier| bezier.ray_test_crossings(ray_start, ray_direction).count()).sum()
	}

	/// Returns how many times a given ray intersects with this subpath. (`ray_direction` does not need to be normalized.)
	/// This version of the function is for better performance when calling it frequently without needing to change the rotation between each call.
	/// If that isn't important, use [`ray_test_crossings_count`] which provides an easier interface by taking a ray direction vector.
	/// Instead, this version requires a rotation matrix for the ray's rotation and a prerotated version of this subpath that has had its rotation applied.
	pub fn ray_test_crossings_count_prerotated(&self, ray_start: DVec2, rotation_matrix: DMat2, rotated_subpath: &Self) -> usize {
		self.iter()
			.zip(rotated_subpath.iter())
			.map(|(bezier, rotated_bezier)| bezier.ray_test_crossings_prerotated(ray_start, rotation_matrix, rotated_bezier).count())
			.sum()
	}

	/// Returns true if the given point is inside this subpath. Open paths are NOT automatically closed so you'll need to call `set_closed(true)` before calling this.
	/// Self-intersecting subpaths use the `evenodd` fill rule for checking in/outside-ness: <https://developer.mozilla.org/en-US/docs/Web/SVG/Attribute/fill-rule>.
	/// If this needs to be called frequently, consider instead using [`point_inside_prerotated`] and moving this function's setup code into your own logic before the repeated call.
	pub fn point_inside(&self, point: DVec2) -> bool {
		// The directions use prime numbers to reduce the likelihood of running across two anchor points simultaneously
		const SIN_13DEG: f64 = 0.22495105434;
		const COS_13DEG: f64 = 0.97437006478;
		const DIRECTION1: DVec2 = DVec2::new(SIN_13DEG, COS_13DEG);
		const DIRECTION2: DVec2 = DVec2::new(-COS_13DEG, -SIN_13DEG);

		// We (inefficiently) check for odd crossings in two directions and make sure they agree to reduce how often anchor points cause a double-increment
		let test1 = self.ray_test_crossings_count(point, DIRECTION1) % 2 == 1;
		let test2 = self.ray_test_crossings_count(point, DIRECTION2) % 2 == 1;

		test1 && test2
	}

	/// Returns true if the given point is inside this subpath. Open paths are NOT automatically closed so you'll need to call `set_closed(true)` before calling this.
	/// Self-intersecting subpaths use the `evenodd` fill rule for checking in/outside-ness: <https://developer.mozilla.org/en-US/docs/Web/SVG/Attribute/fill-rule>.
	/// This version of the function is for better performance when calling it frequently because it lets the caller precompute the rotations once instead of every call.
	/// If that isn't important, use [`point_inside`] which provides an easier interface.
	/// Instead, this version requires a pair of rotation matrices for the ray's rotation and a pair of prerotated versions of this subpath.
	/// They should face in different directions that are unlikely to align in the real world. Consider using the following rotations:
	/// ```rs
	/// const SIN_13DEG: f64 = 0.22495105434;
	/// const COS_13DEG: f64 = 0.97437006478;
	/// const DIRECTION1: DVec2 = DVec2::new(SIN_13DEG, COS_13DEG);
	/// const DIRECTION2: DVec2 = DVec2::new(-COS_13DEG, -SIN_13DEG);
	/// ```
	pub fn point_inside_prerotated(&self, point: DVec2, rotation_matrix1: DMat2, rotation_matrix2: DMat2, rotated_subpath1: &Self, rotated_subpath2: &Self) -> bool {
		// We (inefficiently) check for odd crossings in two directions and make sure they agree to reduce how often anchor points cause a double-increment
		let test1 = self.ray_test_crossings_count_prerotated(point, rotation_matrix1, rotated_subpath1) % 2 == 1;
		let test2 = self.ray_test_crossings_count_prerotated(point, rotation_matrix2, rotated_subpath2) % 2 == 1;

		test1 && test2
	}

	/// Computes the winding number contribution of the subpath.
	pub fn winding_order(&self, point: DVec2) -> i32 {
		self.iter().map(|segment| segment.winding(point)).sum()
	}

	/// Returns a list of `t` values that correspond to the self intersection points of the subpath. For each intersection point, the returned `t` value is the smaller of the two that correspond to the point.
	/// - `error` - For intersections with non-linear beziers, `error` defines the threshold for bounding boxes to be considered an intersection point.
	/// - `minimum_separation`: the minimum difference two adjacent `t`-values must have when comparing adjacent `t`-values in sorted order.
	/// If the comparison condition is not satisfied, the function takes the larger `t`-value of the two
	///
	/// **NOTE**: if an intersection were to occur within an `error` distance away from an anchor point, the algorithm will filter that intersection out.
	/// <iframe frameBorder="0" width="100%" height="375px" src="https://graphite.rs/libraries/bezier-rs#subpath/intersect-self/solo" title="Self-Intersection Demo"></iframe>
	pub fn self_intersections(&self, error: Option<f64>, minimum_separation: Option<f64>) -> Vec<(usize, f64)> {
		let mut intersections_vec = Vec::new();
		let err = error.unwrap_or(MAX_ABSOLUTE_DIFFERENCE);
		// TODO: optimization opportunity - this for-loop currently compares all intersections with all curve-segments in the subpath collection
		self.iter().enumerate().for_each(|(i, other)| {
			intersections_vec.extend(other.self_intersections(error).iter().map(|value| (i, value[0])));
			self.iter().enumerate().skip(i + 1).for_each(|(j, curve)| {
				intersections_vec.extend(
					curve
						.intersections(&other, error, minimum_separation)
						.iter()
						.filter(|&value| value > &err && (1. - value) > err)
						.map(|value| (j, *value)),
				);
			});
		});
		intersections_vec
	}

	/// Calculates the intersection points the subpath has with a given rectangle and returns a list of `(usize, f64)` tuples,
	/// where the `usize` represents the index of the curve in the subpath, and the `f64` represents the `t`-value local to
	/// that curve where the intersection occurred.
	/// Expects the following:
	/// - `corner1`: any corner of the axis-aligned box to intersect with
	/// - `corner2`: the corner opposite to `corner1`
	/// - `error`: an optional f64 value to provide an error bound
	/// - `minimum_separation`: the minimum difference two adjacent `t`-values must have when comparing adjacent `t`-values in sorted order.
	/// If the comparison condition is not satisfied, the function takes the larger `t`-value of the two.
	/// <iframe frameBorder="0" width="100%" height="375px" src="https://graphite.rs/libraries/bezier-rs#subpath/intersect-rectangle/solo" title="Intersection Demo"></iframe>
	pub fn rectangle_intersections(&self, corner1: DVec2, corner2: DVec2, error: Option<f64>, minimum_separation: Option<f64>) -> Vec<(usize, f64)> {
		[
			Bezier::from_linear_coordinates(corner1.x, corner1.y, corner2.x, corner1.y),
			Bezier::from_linear_coordinates(corner2.x, corner1.y, corner2.x, corner2.y),
			Bezier::from_linear_coordinates(corner2.x, corner2.y, corner1.x, corner2.y),
			Bezier::from_linear_coordinates(corner1.x, corner2.y, corner1.x, corner1.y),
		]
		.iter()
		.flat_map(|bezier| self.intersections(bezier, error, minimum_separation))
		.collect()
	}

	/// Checks if any intersections exist between this subpath and the four edges of the rectangle defined by the top-left `corner1` and bottom-right `corner2`.
	/// This is faster than calling [`rectangle_intersections`]`.len()` because it short-circuits as soon as an intersection is found.
	pub fn rectangle_intersections_exist(&self, corner1: DVec2, corner2: DVec2) -> bool {
		let rotate_by_90deg = |point| DMat2::from_angle(std::f64::consts::FRAC_PI_2) * point;

		for bezier in self.iter() {
			// Check that the two bounding boxes don't intersect, since we can avoid doing intersection's cubic root finding in that case
			let [bezier_corner1, bezier_corner2] = bezier.bounding_box_of_anchors_and_handles();
			if !(((corner1.x < bezier_corner1.x) && (bezier_corner1.x < corner2.x) || (corner1.x < bezier_corner2.x) && (bezier_corner2.x < corner2.x))
				&& corner1.y < bezier_corner2.y
				&& corner2.y > bezier_corner1.y
				|| ((corner1.y < bezier_corner1.y) && (bezier_corner1.y < corner2.y) || (corner1.y < bezier_corner2.y) && (bezier_corner2.y < corner2.y))
					&& corner1.x < bezier_corner2.x
					&& corner2.x > bezier_corner1.x)
			{
				continue;
			}

			// Original rotation axis
			if bezier.line_test_crossings_prerotated(corner1, DMat2::IDENTITY, bezier).any(|intersection_point| {
				let (_, y) = bezier.unrestricted_parametric_evaluate(intersection_point).into();
				y >= corner1.y && y <= corner2.y
			}) {
				return true;
			}
			if bezier.line_test_crossings_prerotated(corner2, DMat2::IDENTITY, bezier).any(|intersection_point| {
				let (_, y) = bezier.unrestricted_parametric_evaluate(intersection_point).into();
				y >= corner1.y && y <= corner2.y
			}) {
				return true;
			}

			// Perpendicular to original rotation axis
			let rotated_bezier = bezier.apply_transformation(rotate_by_90deg);
			if bezier.line_test_crossings_prerotated(corner1, DMat2::IDENTITY, rotated_bezier).any(|intersection_point| {
				let (x, _) = bezier.unrestricted_parametric_evaluate(intersection_point).into();
				x >= corner1.x && x <= corner2.x
			}) {
				return true;
			}
			if bezier.line_test_crossings_prerotated(corner2, DMat2::IDENTITY, rotated_bezier).any(|intersection_point| {
				let (x, _) = bezier.unrestricted_parametric_evaluate(intersection_point).into();
				x >= corner1.x && x <= corner2.x
			}) {
				return true;
			}
		}

		false
	}

	/// Returns a normalized unit vector representing the tangent on the subpath based on the parametric `t`-value provided.
	/// <iframe frameBorder="0" width="100%" height="350px" src="https://graphite.rs/libraries/bezier-rs#subpath/tangent/solo" title="Tangent Demo"></iframe>
	pub fn tangent(&self, t: SubpathTValue) -> DVec2 {
		let (segment_index, t) = self.t_value_to_parametric(t);
		self.get_segment(segment_index).unwrap().tangent(TValue::Parametric(t))
	}

	/// Returns a normalized unit vector representing the direction of the normal on the subpath based on the parametric `t`-value provided.
	/// <iframe frameBorder="0" width="100%" height="350px" src="https://graphite.rs/libraries/bezier-rs#subpath/normal/solo" title="Normal Demo"></iframe>
	pub fn normal(&self, t: SubpathTValue) -> DVec2 {
		let (segment_index, t) = self.t_value_to_parametric(t);
		self.get_segment(segment_index).unwrap().normal(TValue::Parametric(t))
	}

	/// Returns two lists of `t`-values representing the local extrema of the `x` and `y` parametric subpaths respectively.
	/// The list of `t`-values returned are filtered such that they fall within the range `[0, 1]`.
	/// <iframe frameBorder="0" width="100%" height="300px" src="https://graphite.rs/libraries/bezier-rs#subpath/local-extrema/solo" title="Local Extrema Demo"></iframe>
	pub fn local_extrema(&self) -> [Vec<f64>; 2] {
		let number_of_curves = self.len_segments() as f64;

		// TODO: Consider the shared point between adjacent beziers.
		self.iter().enumerate().fold([Vec::new(), Vec::new()], |mut acc, elem| {
			let [x, y] = elem.1.local_extrema();
			// Convert t-values of bezier curve to t-values of subpath
			acc[0].extend(x.map(|t| ((elem.0 as f64) + t) / number_of_curves).collect::<Vec<f64>>());
			acc[1].extend(y.map(|t| ((elem.0 as f64) + t) / number_of_curves).collect::<Vec<f64>>());
			acc
		})
	}

	/// Return the min and max corners that represent the bounding box of the subpath.
	/// <iframe frameBorder="0" width="100%" height="300px" src="https://graphite.rs/libraries/bezier-rs#subpath/bounding-box/solo" title="Bounding Box Demo"></iframe>
	pub fn bounding_box(&self) -> Option<[DVec2; 2]> {
		self.iter().map(|bezier| bezier.bounding_box()).reduce(|bbox1, bbox2| [bbox1[0].min(bbox2[0]), bbox1[1].max(bbox2[1])])
	}

	/// Return the min and max corners that represent the bounding box of the subpath, after a given affine transform.
	pub fn bounding_box_with_transform(&self, transform: glam::DAffine2) -> Option<[DVec2; 2]> {
		self.iter()
			.map(|bezier| bezier.apply_transformation(|v| transform.transform_point2(v)).bounding_box())
			.reduce(|bbox1, bbox2| [bbox1[0].min(bbox2[0]), bbox1[1].max(bbox2[1])])
	}

	/// Returns list of `t`-values representing the inflection points of the subpath.
	/// The list of `t`-values returned are filtered such that they fall within the range `[0, 1]`.
	/// <iframe frameBorder="0" width="100%" height="300px" src="https://graphite.rs/libraries/bezier-rs#subpath/inflections/solo" title="Inflections Demo"></iframe>
	pub fn inflections(&self) -> Vec<f64> {
		let number_of_curves = self.len_segments() as f64;
		let inflection_t_values: Vec<f64> = self
			.iter()
			.enumerate()
			.flat_map(|(index, bezier)| {
				bezier
					.inflections()
					.into_iter()
					// Convert t-values of bezier curve to t-values of subpath
					.map(move |t| ((index as f64) + t) / number_of_curves)
			})
			.collect();

		// TODO: Consider the shared point between adjacent beziers.
		inflection_t_values
	}

	/// Does a path contain a point? Based on the non zero winding
	pub fn contains_point(&self, target_point: DVec2) -> bool {
		self.iter().map(|bezier| bezier.winding(target_point)).sum::<i32>() != 0
	}

	/// Randomly places points across the filled surface of this subpath (which is assumed to be closed).
	/// The `separation_disk_diameter` determines the minimum distance between all points from one another.
	/// Conceptually, this works by "throwing a dart" at the subpath's bounding box and keeping the dart only if:
	/// - It's inside the shape
	/// - It's not closer than `separation_disk_diameter` to any other point from a previous accepted dart throw
	/// This repeats until accepted darts fill all possible areas between one another.
	///
	/// While the conceptual process described above asymptotically slows down and is never guaranteed to produce a maximal set in finite time,
	/// this is implemented with an algorithm that produces a maximal set in O(n) time. The slowest part is actually checking if points are inside the subpath shape.
	pub fn poisson_disk_points(&self, separation_disk_diameter: f64, rng: impl FnMut() -> f64) -> Vec<DVec2> {
		let Some(bounding_box) = self.bounding_box() else { return Vec::new() };
		let (offset_x, offset_y) = bounding_box[0].into();
		let (width, height) = (bounding_box[1] - bounding_box[0]).into();

		// TODO: Optimize the following code and make it more robust

		let mut shape = self.clone();
		shape.set_closed(true);
		shape.apply_transform(DAffine2::from_translation((-offset_x, -offset_y).into()));

		let point_in_shape_checker = |point: DVec2| shape.winding_order(point) != 0;

		let square_edges_intersect_shape_checker = |corner1: DVec2, size: f64| {
			let corner2 = corner1 + DVec2::splat(size);
			self.rectangle_intersections_exist(corner1, corner2)
		};

		let mut points = crate::poisson_disk::poisson_disk_sample(width, height, separation_disk_diameter, point_in_shape_checker, square_edges_intersect_shape_checker, rng);
		for point in &mut points {
			point.x += offset_x;
			point.y += offset_y;
		}
		points
	}

	/// Returns the manipulator point that is needed for a miter join if it is possible.
	/// - `miter_limit`: Defines a limit for the ratio between the miter length and the stroke width.
	/// Alternatively, this can be interpreted as limiting the angle that the miter can form.
	/// When the limit is exceeded, no manipulator group will be returned.
	/// This value should be at least 1. If not, the default of 4 will be used.
	pub(crate) fn miter_line_join(&self, other: &Subpath<ManipulatorGroupId>, miter_limit: Option<f64>) -> Option<ManipulatorGroup<ManipulatorGroupId>> {
		let miter_limit = match miter_limit {
			Some(miter_limit) if miter_limit >= 1. => miter_limit,
			_ => 4.,
		};
		let in_segment = self.get_segment(self.len_segments() - 1).unwrap();
		let out_segment = other.get_segment(0).unwrap();
		let in_tangent = in_segment.tangent(TValue::Parametric(1.));
		let out_tangent = out_segment.tangent(TValue::Parametric(0.));

		let normalized_in_tangent = in_tangent.normalize();
		let normalized_out_tangent = out_tangent.normalize();

		// The tangents must not be parallel for the miter join
		if !normalized_in_tangent.abs_diff_eq(normalized_out_tangent, MAX_ABSOLUTE_DIFFERENCE) && !normalized_in_tangent.abs_diff_eq(-normalized_out_tangent, MAX_ABSOLUTE_DIFFERENCE) {
			let intersection = line_intersection(in_segment.end(), in_tangent, out_segment.start(), out_tangent);

			let start_to_intersection = intersection - in_segment.end();
			let intersection_to_end = out_segment.start() - intersection;

			// Draw the miter join if the intersection occurs in the correct direction with respect to the path
			if start_to_intersection.normalize().abs_diff_eq(in_tangent, MAX_ABSOLUTE_DIFFERENCE)
				&& intersection_to_end.normalize().abs_diff_eq(out_tangent, MAX_ABSOLUTE_DIFFERENCE)
				&& miter_limit >= 1. / (start_to_intersection.angle_between(-intersection_to_end).abs() / 2.).sin()
			{
				return Some(ManipulatorGroup {
					anchor: intersection,
					in_handle: None,
					out_handle: None,
					id: ManipulatorGroupId::new(),
				});
			}
		}
		// If we can't draw the miter join, default to a bevel join
		None
	}

	/// Returns the necessary information to create a round join with the provided center.
	/// The returned items correspond to:
	/// - The `out_handle` for the last manipulator group of `self`
	/// - The new manipulator group to be added
	/// - The `in_handle` for the first manipulator group of `other`
	pub(crate) fn round_line_join(&self, other: &Subpath<ManipulatorGroupId>, center: DVec2) -> (DVec2, ManipulatorGroup<ManipulatorGroupId>, DVec2) {
		let left = self.manipulator_groups[self.len() - 1].anchor;
		let right = other.manipulator_groups[0].anchor;

		let center_to_right = right - center;
		let center_to_left = left - center;

		let in_segment = self.get_segment(self.len_segments() - 1).unwrap();
		let in_tangent = in_segment.tangent(TValue::Parametric(1.));

		let mut angle = center_to_right.angle_between(center_to_left) / 2.;
		let mut arc_point = center + DMat2::from_angle(angle).mul_vec2(center_to_right);

		if (arc_point - left).angle_between(in_tangent).abs() > PI / 2. {
			angle = angle - PI * (if angle < 0. { -1. } else { 1. });
			arc_point = center + DMat2::from_angle(angle).mul_vec2(center_to_right);
		}

		compute_circular_subpath_details(left, arc_point, right, center, Some(angle))
	}

	/// Returns the necessary information to create a round cap between the end of `self` and the beginning of `other`.
	/// The returned items correspond to:
	/// - The `out_handle` for the last manipulator group of `self`
	/// - The new manipulator group to be added
	/// - The `in_handle` for the first manipulator group of `other`
	pub(crate) fn round_cap(&self, other: &Subpath<ManipulatorGroupId>) -> (DVec2, ManipulatorGroup<ManipulatorGroupId>, DVec2) {
		let left = self.manipulator_groups[self.len() - 1].anchor;
		let right = other.manipulator_groups[0].anchor;

		let center = (right + left) / 2.;
		let center_to_right = right - center;

		let arc_point = center + center_to_right.perp();

		compute_circular_subpath_details(left, arc_point, right, center, None)
	}

	/// Returns the two manipulator groups that create a square cap between the end of `self` and the beginning of `other`.
	pub(crate) fn square_cap(&self, other: &Subpath<ManipulatorGroupId>) -> [ManipulatorGroup<ManipulatorGroupId>; 2] {
		let left = self.manipulator_groups[self.len() - 1].anchor;
		let right = other.manipulator_groups[0].anchor;

		let center = (right + left) / 2.;
		let center_to_right = right - center;

		let translation = center_to_right.perp();

		[ManipulatorGroup::new_anchor(left + translation), ManipulatorGroup::new_anchor(right + translation)]
	}

	/// Returns the curvature, a scalar value for the derivative at the point `t` along the subpath.
	/// Curvature is 1 over the radius of a circle with an equivalent derivative.
	/// <iframe frameBorder="0" width="100%" height="350px" src="https://graphite.rs/libraries/bezier-rs#subpath/curvature/solo" title="Curvature Demo"></iframe>
	pub fn curvature(&self, t: SubpathTValue) -> f64 {
		let (segment_index, t) = self.t_value_to_parametric(t);
		self.get_segment(segment_index).unwrap().curvature(TValue::Parametric(t))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::Bezier;
	use glam::DVec2;

	use crate::consts::MAX_ABSOLUTE_DIFFERENCE;
	use crate::utils;

	fn normalize_t(n: i64, t: f64) -> f64 {
		t * (n as f64) % 1.
	}

	#[test]
	fn evaluate_one_subpath_curve() {
		let start = DVec2::new(20., 30.);
		let end = DVec2::new(60., 45.);
		let handle = DVec2::new(75., 85.);

		let bezier = Bezier::from_quadratic_dvec2(start, handle, end);
		let subpath = Subpath::new(
			vec![
				ManipulatorGroup {
					anchor: start,
					in_handle: None,
					out_handle: Some(handle),
					id: EmptyId,
				},
				ManipulatorGroup {
					anchor: end,
					in_handle: None,
					out_handle: Some(handle),
					id: EmptyId,
				},
			],
			false,
		);

		let t0 = 0.;
		assert_eq!(subpath.evaluate(SubpathTValue::GlobalParametric(t0)), bezier.evaluate(TValue::Parametric(t0)));

		let t1 = 0.25;
		assert_eq!(subpath.evaluate(SubpathTValue::GlobalParametric(t1)), bezier.evaluate(TValue::Parametric(t1)));

		let t2 = 0.50;
		assert_eq!(subpath.evaluate(SubpathTValue::GlobalParametric(t2)), bezier.evaluate(TValue::Parametric(t2)));

		let t3 = 1.;
		assert_eq!(subpath.evaluate(SubpathTValue::GlobalParametric(t3)), bezier.evaluate(TValue::Parametric(t3)));
	}

	#[test]
	fn evaluate_multiple_subpath_curves() {
		let start = DVec2::new(20., 30.);
		let middle = DVec2::new(70., 70.);
		let end = DVec2::new(60., 45.);
		let handle1 = DVec2::new(75., 85.);
		let handle2 = DVec2::new(40., 30.);
		let handle3 = DVec2::new(10., 10.);

		let linear_bezier = Bezier::from_linear_dvec2(start, middle);
		let quadratic_bezier = Bezier::from_quadratic_dvec2(middle, handle1, end);
		let cubic_bezier = Bezier::from_cubic_dvec2(end, handle2, handle3, start);

		let mut subpath = Subpath::new(
			vec![
				ManipulatorGroup {
					anchor: start,
					in_handle: Some(handle3),
					out_handle: None,
					id: EmptyId,
				},
				ManipulatorGroup {
					anchor: middle,
					in_handle: None,
					out_handle: Some(handle1),
					id: EmptyId,
				},
				ManipulatorGroup {
					anchor: end,
					in_handle: None,
					out_handle: Some(handle2),
					id: EmptyId,
				},
			],
			false,
		);

		// Test open subpath

		let mut n = (subpath.len() as i64) - 1;

		let t0 = 0.;
		assert!(utils::dvec2_compare(
			subpath.evaluate(SubpathTValue::GlobalParametric(t0)),
			linear_bezier.evaluate(TValue::Parametric(normalize_t(n, t0))),
			MAX_ABSOLUTE_DIFFERENCE
		)
		.all());

		let t1 = 0.25;
		assert!(utils::dvec2_compare(
			subpath.evaluate(SubpathTValue::GlobalParametric(t1)),
			linear_bezier.evaluate(TValue::Parametric(normalize_t(n, t1))),
			MAX_ABSOLUTE_DIFFERENCE
		)
		.all());

		let t2 = 0.50;
		assert!(utils::dvec2_compare(
			subpath.evaluate(SubpathTValue::GlobalParametric(t2)),
			quadratic_bezier.evaluate(TValue::Parametric(normalize_t(n, t2))),
			MAX_ABSOLUTE_DIFFERENCE
		)
		.all());

		let t3 = 0.75;
		assert!(utils::dvec2_compare(
			subpath.evaluate(SubpathTValue::GlobalParametric(t3)),
			quadratic_bezier.evaluate(TValue::Parametric(normalize_t(n, t3))),
			MAX_ABSOLUTE_DIFFERENCE
		)
		.all());

		let t4 = 1.0;
		assert!(utils::dvec2_compare(
			subpath.evaluate(SubpathTValue::GlobalParametric(t4)),
			quadratic_bezier.evaluate(TValue::Parametric(1.)),
			MAX_ABSOLUTE_DIFFERENCE
		)
		.all());

		// Test closed subpath

		subpath.closed = true;
		n = subpath.len() as i64;

		let t5 = 2. / 3.;
		assert!(utils::dvec2_compare(
			subpath.evaluate(SubpathTValue::GlobalParametric(t5)),
			cubic_bezier.evaluate(TValue::Parametric(normalize_t(n, t5))),
			MAX_ABSOLUTE_DIFFERENCE
		)
		.all());

		let t6 = 1.;
		assert!(utils::dvec2_compare(
			subpath.evaluate(SubpathTValue::GlobalParametric(t6)),
			cubic_bezier.evaluate(TValue::Parametric(1.)),
			MAX_ABSOLUTE_DIFFERENCE
		)
		.all());
	}

	#[test]
	fn intersection_linear_multiple_subpath_curves_test_one() {
		// M 35 125 C 40 40 120 120 43 43 Q 175 90 145 150 Q 70 185 35 125 Z

		let cubic_start = DVec2::new(35., 125.);
		let cubic_handle_1 = DVec2::new(40., 40.);
		let cubic_handle_2 = DVec2::new(120., 120.);
		let cubic_end = DVec2::new(43., 43.);

		let quadratic_1_handle = DVec2::new(175., 90.);
		let quadratic_end = DVec2::new(145., 150.);

		let quadratic_2_handle = DVec2::new(70., 185.);

		let cubic_bezier = Bezier::from_cubic_dvec2(cubic_start, cubic_handle_1, cubic_handle_2, cubic_end);
		let quadratic_bezier_1 = Bezier::from_quadratic_dvec2(cubic_end, quadratic_1_handle, quadratic_end);

		let subpath = Subpath::new(
			vec![
				ManipulatorGroup {
					anchor: cubic_start,
					in_handle: None,
					out_handle: Some(cubic_handle_1),
					id: EmptyId,
				},
				ManipulatorGroup {
					anchor: cubic_end,
					in_handle: Some(cubic_handle_2),
					out_handle: None,
					id: EmptyId,
				},
				ManipulatorGroup {
					anchor: quadratic_end,
					in_handle: Some(quadratic_1_handle),
					out_handle: Some(quadratic_2_handle),
					id: EmptyId,
				},
			],
			true,
		);

		let line = Bezier::from_linear_coordinates(150., 150., 20., 20.);

		let cubic_intersections = cubic_bezier.intersections(&line, None, None);
		let quadratic_1_intersections = quadratic_bezier_1.intersections(&line, None, None);
		let subpath_intersections = subpath.intersections(&line, None, None);

		assert!(utils::dvec2_compare(
			cubic_bezier.evaluate(TValue::Parametric(cubic_intersections[0])),
			subpath.evaluate(SubpathTValue::Parametric {
				segment_index: subpath_intersections[0].0,
				t: subpath_intersections[0].1
			}),
			MAX_ABSOLUTE_DIFFERENCE
		)
		.all());

		assert!(utils::dvec2_compare(
			quadratic_bezier_1.evaluate(TValue::Parametric(quadratic_1_intersections[0])),
			subpath.evaluate(SubpathTValue::Parametric {
				segment_index: subpath_intersections[1].0,
				t: subpath_intersections[1].1
			}),
			MAX_ABSOLUTE_DIFFERENCE
		)
		.all());

		assert!(utils::dvec2_compare(
			quadratic_bezier_1.evaluate(TValue::Parametric(quadratic_1_intersections[1])),
			subpath.evaluate(SubpathTValue::Parametric {
				segment_index: subpath_intersections[2].0,
				t: subpath_intersections[2].1
			}),
			MAX_ABSOLUTE_DIFFERENCE
		)
		.all());
	}

	#[test]
	fn intersection_linear_multiple_subpath_curves_test_two() {
		// M34 107 C40 40 120 120 102 29 Q175 90 129 171 Q70 185 34 107 Z
		// M150 150 L 20 20

		let cubic_start = DVec2::new(34., 107.);
		let cubic_handle_1 = DVec2::new(40., 40.);
		let cubic_handle_2 = DVec2::new(120., 120.);
		let cubic_end = DVec2::new(102., 29.);

		let quadratic_1_handle = DVec2::new(175., 90.);
		let quadratic_end = DVec2::new(129., 171.);

		let quadratic_2_handle = DVec2::new(70., 185.);

		let cubic_bezier = Bezier::from_cubic_dvec2(cubic_start, cubic_handle_1, cubic_handle_2, cubic_end);
		let quadratic_bezier_1 = Bezier::from_quadratic_dvec2(cubic_end, quadratic_1_handle, quadratic_end);

		let subpath = Subpath::new(
			vec![
				ManipulatorGroup {
					anchor: cubic_start,
					in_handle: None,
					out_handle: Some(cubic_handle_1),
					id: EmptyId,
				},
				ManipulatorGroup {
					anchor: cubic_end,
					in_handle: Some(cubic_handle_2),
					out_handle: None,
					id: EmptyId,
				},
				ManipulatorGroup {
					anchor: quadratic_end,
					in_handle: Some(quadratic_1_handle),
					out_handle: Some(quadratic_2_handle),
					id: EmptyId,
				},
			],
			true,
		);

		let line = Bezier::from_linear_coordinates(150., 150., 20., 20.);

		let cubic_intersections = cubic_bezier.intersections(&line, None, None);
		let quadratic_1_intersections = quadratic_bezier_1.intersections(&line, None, None);
		let subpath_intersections = subpath.intersections(&line, None, None);

		assert!(utils::dvec2_compare(
			cubic_bezier.evaluate(TValue::Parametric(cubic_intersections[0])),
			subpath.evaluate(SubpathTValue::Parametric {
				segment_index: subpath_intersections[0].0,
				t: subpath_intersections[0].1
			}),
			MAX_ABSOLUTE_DIFFERENCE
		)
		.all());

		assert!(utils::dvec2_compare(
			quadratic_bezier_1.evaluate(TValue::Parametric(quadratic_1_intersections[0])),
			subpath.evaluate(SubpathTValue::Parametric {
				segment_index: subpath_intersections[1].0,
				t: subpath_intersections[1].1
			}),
			MAX_ABSOLUTE_DIFFERENCE
		)
		.all());
	}

	#[test]
	fn intersection_linear_multiple_subpath_curves_test_three() {
		// M35 125 C40 40 120 120 44 44 Q175 90 145 150 Q70 185 35 125 Z

		let cubic_start = DVec2::new(35., 125.);
		let cubic_handle_1 = DVec2::new(40., 40.);
		let cubic_handle_2 = DVec2::new(120., 120.);
		let cubic_end = DVec2::new(44., 44.);

		let quadratic_1_handle = DVec2::new(175., 90.);
		let quadratic_end = DVec2::new(145., 150.);

		let quadratic_2_handle = DVec2::new(70., 185.);

		let cubic_bezier = Bezier::from_cubic_dvec2(cubic_start, cubic_handle_1, cubic_handle_2, cubic_end);
		let quadratic_bezier_1 = Bezier::from_quadratic_dvec2(cubic_end, quadratic_1_handle, quadratic_end);

		let subpath = Subpath::new(
			vec![
				ManipulatorGroup {
					anchor: cubic_start,
					in_handle: None,
					out_handle: Some(cubic_handle_1),
					id: EmptyId,
				},
				ManipulatorGroup {
					anchor: cubic_end,
					in_handle: Some(cubic_handle_2),
					out_handle: None,
					id: EmptyId,
				},
				ManipulatorGroup {
					anchor: quadratic_end,
					in_handle: Some(quadratic_1_handle),
					out_handle: Some(quadratic_2_handle),
					id: EmptyId,
				},
			],
			true,
		);

		let line = Bezier::from_linear_coordinates(150., 150., 20., 20.);

		let cubic_intersections = cubic_bezier.intersections(&line, None, None);
		let quadratic_1_intersections = quadratic_bezier_1.intersections(&line, None, None);
		let subpath_intersections = subpath.intersections(&line, None, None);

		assert!(utils::dvec2_compare(
			cubic_bezier.evaluate(TValue::Parametric(cubic_intersections[0])),
			subpath.evaluate(SubpathTValue::Parametric {
				segment_index: subpath_intersections[0].0,
				t: subpath_intersections[0].1
			}),
			MAX_ABSOLUTE_DIFFERENCE
		)
		.all());

		assert!(utils::dvec2_compare(
			quadratic_bezier_1.evaluate(TValue::Parametric(quadratic_1_intersections[0])),
			subpath.evaluate(SubpathTValue::Parametric {
				segment_index: subpath_intersections[1].0,
				t: subpath_intersections[1].1
			}),
			MAX_ABSOLUTE_DIFFERENCE
		)
		.all());

		assert!(utils::dvec2_compare(
			quadratic_bezier_1.evaluate(TValue::Parametric(quadratic_1_intersections[1])),
			subpath.evaluate(SubpathTValue::Parametric {
				segment_index: subpath_intersections[2].0,
				t: subpath_intersections[2].1
			}),
			MAX_ABSOLUTE_DIFFERENCE
		)
		.all());
	}

	// TODO: add more intersection tests

	#[test]
	fn round_join_counter_clockwise_rotation() {
		// Test case where the round join is drawn in the counter clockwise direction between two consecutive offsets
		let subpath = Subpath::new(
			vec![
				ManipulatorGroup {
					anchor: DVec2::new(20., 20.),
					out_handle: Some(DVec2::new(10., 90.)),
					in_handle: None,
					id: EmptyId,
				},
				ManipulatorGroup {
					anchor: DVec2::new(114., 159.),
					out_handle: None,
					in_handle: Some(DVec2::new(60., 40.)),
					id: EmptyId,
				},
				ManipulatorGroup {
					anchor: DVec2::new(148., 155.),
					out_handle: None,
					in_handle: None,
					id: EmptyId,
				},
			],
			false,
		);

		let offset = subpath.offset(10., utils::Join::Round);
		let offset_len = offset.len();

		let manipulator_groups = offset.manipulator_groups();
		let round_start = manipulator_groups[offset_len - 4].anchor;
		let round_point = manipulator_groups[offset_len - 3].anchor;
		let round_end = manipulator_groups[offset_len - 2].anchor;

		let middle = (round_start + round_end) / 2.;

		assert!((round_point - middle).angle_between(round_start - middle) > 0.);
		assert!((round_end - middle).angle_between(round_point - middle) > 0.);
	}

	#[test]
	fn round_join_clockwise_rotation() {
		// Test case where the round join is drawn in the clockwise direction between two consecutive offsets
		let subpath = Subpath::new(
			vec![
				ManipulatorGroup {
					anchor: DVec2::new(20., 20.),
					out_handle: Some(DVec2::new(10., 90.)),
					in_handle: None,
					id: EmptyId,
				},
				ManipulatorGroup {
					anchor: DVec2::new(150., 40.),
					out_handle: None,
					in_handle: Some(DVec2::new(60., 40.)),
					id: EmptyId,
				},
				ManipulatorGroup {
					anchor: DVec2::new(78., 36.),
					out_handle: None,
					in_handle: None,
					id: EmptyId,
				},
			],
			false,
		);

		let offset = subpath.offset(-15., utils::Join::Round);
		let offset_len = offset.len();

		let manipulator_groups = offset.manipulator_groups();
		let round_start = manipulator_groups[offset_len - 4].anchor;
		let round_point = manipulator_groups[offset_len - 3].anchor;
		let round_end = manipulator_groups[offset_len - 2].anchor;

		let middle = (round_start + round_end) / 2.;

		assert!((round_point - middle).angle_between(round_start - middle) < 0.);
		assert!((round_end - middle).angle_between(round_point - middle) < 0.);
	}
}

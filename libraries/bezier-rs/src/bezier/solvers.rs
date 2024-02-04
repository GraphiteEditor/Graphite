use super::*;
use crate::utils::{solve_cubic, solve_quadratic, TValue};
use crate::{to_symmetrical_basis_pair, SymmetricalBasis};

use glam::DMat2;
use std::ops::Range;

/// Functionality that solve for various curve information such as derivative, tangent, intersect, etc.
impl Bezier {
	/// Get roots as [[x], [y]]
	#[must_use]
	pub fn roots(self) -> [Vec<f64>; 2] {
		let s_basis = to_symmetrical_basis_pair(self);
		[s_basis.x.roots(), s_basis.y.roots()]
	}

	/// Returns a list of lists of points representing the De Casteljau points for all iterations at the point `t` along the curve using De Casteljau's algorithm.
	/// The `i`th element of the list represents the set of points in the `i`th iteration.
	/// More information on the algorithm can be found in the [De Casteljau section](https://pomax.github.io/bezierinfo/#decasteljau) in Pomax's primer.
	/// <iframe frameBorder="0" width="100%" height="350px" src="https://graphite.rs/libraries/bezier-rs#bezier/de-casteljau-points/solo" title="De Casteljau Demo"></iframe>
	pub fn de_casteljau_points(&self, t: TValue) -> Vec<Vec<DVec2>> {
		let t = self.t_value_to_parametric(t);
		let bezier_points = match self.handles {
			BezierHandles::Linear => vec![self.start, self.end],
			BezierHandles::Quadratic { handle } => vec![self.start, handle, self.end],
			BezierHandles::Cubic { handle_start, handle_end } => vec![self.start, handle_start, handle_end, self.end],
		};
		let mut de_casteljau_points = vec![bezier_points];
		let mut current_points = de_casteljau_points.last().unwrap();

		// Iterate until one point is left, that point will be equal to `evaluate(t)`
		while current_points.len() > 1 {
			// Map from every adjacent pair of points to their respective midpoints, which decrements by 1 the number of points for the next iteration
			let next_points: Vec<DVec2> = current_points.as_slice().windows(2).map(|pair| DVec2::lerp(pair[0], pair[1], t)).collect();
			de_casteljau_points.push(next_points);

			current_points = de_casteljau_points.last().unwrap();
		}

		de_casteljau_points
	}

	/// Returns a [Bezier] representing the derivative of the original curve.
	/// - This function returns `None` for a linear segment.
	/// <iframe frameBorder="0" width="100%" height="300px" src="https://graphite.rs/libraries/bezier-rs#bezier/derivative/solo" title="Derivative Demo"></iframe>
	pub fn derivative(&self) -> Option<Bezier> {
		match self.handles {
			BezierHandles::Linear => None,
			BezierHandles::Quadratic { handle } => {
				let p1_minus_p0 = handle - self.start;
				let p2_minus_p1 = self.end - handle;
				Some(Bezier::from_linear_dvec2(2. * p1_minus_p0, 2. * p2_minus_p1))
			}
			BezierHandles::Cubic { handle_start, handle_end } => {
				let p1_minus_p0 = handle_start - self.start;
				let p2_minus_p1 = handle_end - handle_start;
				let p3_minus_p2 = self.end - handle_end;
				Some(Bezier::from_quadratic_dvec2(3. * p1_minus_p0, 3. * p2_minus_p1, 3. * p3_minus_p2))
			}
		}
	}

	/// Returns the non-normalized vector representing the tangent at the point `t` along the curve.
	pub(crate) fn non_normalized_tangent(&self, t: f64) -> DVec2 {
		match self.handles {
			BezierHandles::Linear => self.end - self.start,
			_ => self.derivative().unwrap().evaluate(TValue::Parametric(t)),
		}
	}

	/// Returns a normalized unit vector representing the tangent at the point `t` along the curve.
	/// <iframe frameBorder="0" width="100%" height="350px" src="https://graphite.rs/libraries/bezier-rs#bezier/tangent/solo" title="Tangent Demo"></iframe>
	pub fn tangent(&self, t: TValue) -> DVec2 {
		let t = self.t_value_to_parametric(t);
		let tangent = self.non_normalized_tangent(t);
		if tangent.length() > 0. {
			tangent.normalize()
		} else {
			tangent
		}
	}

	/// Find the `t`-value(s) such that the tangent(s) at `t` pass through the specified point.
	/// <iframe frameBorder="0" width="100%" height="300px" src="https://graphite.rs/libraries/bezier-rs#bezier/tangents-to-point/solo" title="Tangents to Point Demo"></iframe>
	#[must_use]
	pub fn tangents_to_point(self, point: DVec2) -> Vec<f64> {
		let sbasis: crate::SymmetricalBasisPair = to_symmetrical_basis_pair(self);
		let derivative = sbasis.derivative();
		let cross = (sbasis - point).cross(&derivative);
		SymmetricalBasis::roots(&cross)
	}

	/// Returns a normalized unit vector representing the direction of the normal at the point `t` along the curve.
	/// <iframe frameBorder="0" width="100%" height="350px" src="https://graphite.rs/libraries/bezier-rs#bezier/normal/solo" title="Normal Demo"></iframe>
	pub fn normal(&self, t: TValue) -> DVec2 {
		self.tangent(t).perp()
	}

	/// Find the `t`-value(s) such that the normal(s) at `t` pass through the specified point.
	/// <iframe frameBorder="0" width="100%" height="300px" src="https://graphite.rs/libraries/bezier-rs#bezier/normals-to-point/solo" title="Normals to Point Demo"></iframe>
	#[must_use]
	pub fn normals_to_point(self, point: DVec2) -> Vec<f64> {
		let sbasis = to_symmetrical_basis_pair(self);
		let derivative = sbasis.derivative();
		let cross = (sbasis - point).dot(&derivative);
		SymmetricalBasis::roots(&cross)
	}

	/// Returns the curvature, a scalar value for the derivative at the point `t` along the curve.
	/// Curvature is 1 over the radius of a circle with an equivalent derivative.
	/// <iframe frameBorder="0" width="100%" height="350px" src="https://graphite.rs/libraries/bezier-rs#bezier/curvature/solo" title="Curvature Demo"></iframe>
	pub fn curvature(&self, t: TValue) -> f64 {
		let t = self.t_value_to_parametric(t);
		let (d, dd) = match &self.derivative() {
			Some(first_derivative) => match first_derivative.derivative() {
				Some(second_derivative) => (first_derivative.evaluate(TValue::Parametric(t)), second_derivative.evaluate(TValue::Parametric(t))),
				None => (first_derivative.evaluate(TValue::Parametric(t)), first_derivative.end - first_derivative.start),
			},
			None => (self.end - self.start, DVec2::new(0., 0.)),
		};

		let numerator = d.x * dd.y - d.y * dd.x;
		let denominator = (d.x.powf(2.) + d.y.powf(2.)).powf(1.5);
		if denominator.abs() < MAX_ABSOLUTE_DIFFERENCE {
			0.
		} else {
			numerator / denominator
		}
	}

	/// Returns two lists of `t`-values representing the local extrema of the `x` and `y` parametric curves respectively.
	/// The local extrema are defined to be points at which the derivative of the curve is equal to zero.
	fn unrestricted_local_extrema(&self) -> [[Option<f64>; 3]; 2] {
		match self.handles {
			BezierHandles::Linear => [[None; 3]; 2],
			BezierHandles::Quadratic { handle } => {
				let d0 = handle - self.start;
				let d1 = self.end - handle;
				let dd = d1 - d0;
				let a = (dd.x != 0.).then(|| -d0.x / dd.x);
				let b = (dd.y != 0.).then(|| -d0.y / dd.y);
				[[a, None, None], [b, None, None]]
			}
			BezierHandles::Cubic { handle_start, handle_end } => {
				let d0 = handle_start - self.start;
				let d1 = handle_end - handle_start;
				let d2 = self.end - handle_end;
				let a = d0 - 2. * d1 + d2;
				let b = 2. * (d1 - d0);
				let c = d0;
				let discriminant = b * b - 4. * a * c;
				let two_times_a = 2. * a;
				[
					utils::solve_quadratic(discriminant.x, two_times_a.x, b.x, c.x),
					utils::solve_quadratic(discriminant.y, two_times_a.y, b.y, c.y),
				]
			}
		}
	}

	/// Returns two lists of `t`-values representing the local extrema of the `x` and `y` parametric curves respectively.
	/// The list of `t`-values returned are filtered such that they fall within the range `[0, 1]`.
	/// <iframe frameBorder="0" width="100%" height="300px" src="https://graphite.rs/libraries/bezier-rs#bezier/local-extrema/solo" title="Local Extrema Demo"></iframe>
	pub fn local_extrema(&self) -> [impl Iterator<Item = f64>; 2] {
		self.unrestricted_local_extrema().map(|t_values| t_values.into_iter().flatten().filter(|&t| t > 0. && t < 1.))
	}

	/// Return the min and max corners that represent the bounding box of the curve.
	/// <iframe frameBorder="0" width="100%" height="300px" src="https://graphite.rs/libraries/bezier-rs#bezier/bounding-box/solo" title="Bounding Box Demo"></iframe>
	pub fn bounding_box(&self) -> [DVec2; 2] {
		// Start by taking min/max of endpoints.
		let mut endpoints_min = self.start.min(self.end);
		let mut endpoints_max = self.start.max(self.end);

		// Iterate through extrema points.
		let extrema = self.local_extrema();
		for t_values in extrema {
			for t in t_values {
				let point = self.evaluate(TValue::Parametric(t));
				// Update bounding box if new min/max is found.
				endpoints_min = endpoints_min.min(point);
				endpoints_max = endpoints_max.max(point);
			}
		}

		[endpoints_min, endpoints_max]
	}

	/// Return the min and max corners that represent the bounding box enclosing this Bezier's two anchor points and any handles.
	pub fn bounding_box_of_anchors_and_handles(&self) -> [DVec2; 2] {
		match self.handles {
			BezierHandles::Linear => [self.start.min(self.end), self.start.max(self.end)],
			BezierHandles::Quadratic { handle } => [self.start.min(self.end).min(handle), self.start.max(self.end).max(handle)],
			BezierHandles::Cubic { handle_start, handle_end } => [self.start.min(self.end).min(handle_start).min(handle_end), self.start.max(self.end).max(handle_start).max(handle_end)],
		}
	}

	/// Returns `true` if the bounding box of the bezier is contained entirely within a rectangle defined by its minimum and maximum corners.
	pub fn is_contained_within(&self, min_corner: DVec2, max_corner: DVec2) -> bool {
		let [bounding_box_min, bounding_box_max] = self.bounding_box();
		min_corner.x <= bounding_box_min.x && min_corner.y <= bounding_box_min.y && bounding_box_max.x <= max_corner.x && bounding_box_max.y <= max_corner.y
	}

	/// Returns an `Iterator` containing all possible parametric `t`-values at the given `x`-coordinate.
	pub fn find_tvalues_for_x(&self, x: f64) -> impl Iterator<Item = f64> {
		// Compute the roots of the resulting bezier curve
		match self.handles {
			BezierHandles::Linear => {
				// If the transformed linear bezier is on the x-axis, `a` and `b` will both be zero and `solve_linear` will return no roots
				let a = self.end.x - self.start.x;
				let b = self.start.x - x;
				utils::solve_linear(a, b)
			}
			BezierHandles::Quadratic { handle } => {
				let a = self.start.x - 2. * handle.x + self.end.x;
				let b = 2. * (handle.x - self.start.x);
				let c = self.start.x - x;

				let discriminant = b * b - 4. * a * c;
				let two_times_a = 2. * a;

				utils::solve_quadratic(discriminant, two_times_a, b, c)
			}
			BezierHandles::Cubic { handle_start, handle_end } => {
				let start_x = self.start.x;
				let a = -start_x + 3. * handle_start.x - 3. * handle_end.x + self.end.x;
				let b = 3. * start_x - 6. * handle_start.x + 3. * handle_end.x;
				let c = -3. * start_x + 3. * handle_start.x;
				let d = start_x - x;

				utils::solve_cubic(a, b, c, d)
			}
		}
		.into_iter()
		.flatten()
		.filter(|&t| utils::f64_approximately_in_range(t, 0., 1., MAX_ABSOLUTE_DIFFERENCE))
	}

	// TODO: Use an `impl Iterator` return type instead of a `Vec`
	/// Returns list of `t`-values representing the inflection points of the curve.
	/// The inflection points are defined to be points at which the second derivative of the curve is equal to zero.
	pub fn unrestricted_inflections(&self) -> impl Iterator<Item = f64> {
		match self.handles {
			// There exists no inflection points for linear and quadratic beziers.
			BezierHandles::Linear => [None; 3],
			BezierHandles::Quadratic { .. } => [None; 3],
			BezierHandles::Cubic { .. } => {
				// Axis align the curve.
				let translated_bezier = self.translate(-self.start);
				let angle = translated_bezier.end.angle_between(DVec2::new(1., 0.));
				let rotated_bezier = translated_bezier.rotate(angle);
				if let BezierHandles::Cubic { handle_start, handle_end } = rotated_bezier.handles {
					// These formulas and naming conventions follows https://pomax.github.io/bezierinfo/#inflections
					let a = handle_end.x * handle_start.y;
					let b = rotated_bezier.end.x * handle_start.y;
					let c = handle_start.x * handle_end.y;
					let d = rotated_bezier.end.x * handle_end.y;

					let x = -3. * a + 2. * b + 3. * c - d;
					let y = 3. * a - b - 3. * c;
					let z = c - a;

					let discriminant = y * y - 4. * x * z;
					utils::solve_quadratic(discriminant, 2. * x, y, z)
				} else {
					unreachable!("shouldn't happen")
				}
			}
		}
		.into_iter()
		.flatten()
	}

	/// Returns list of parametric `t`-values representing the inflection points of the curve.
	/// The list of `t`-values returned are filtered such that they fall within the range `[0, 1]`.
	/// <iframe frameBorder="0" width="100%" height="300px" src="https://graphite.rs/libraries/bezier-rs#bezier/inflections/solo" title="Inflections Demo"></iframe>
	pub fn inflections(&self) -> Vec<f64> {
		self.unrestricted_inflections().filter(|&t| t > 0. && t < 1.).collect::<Vec<f64>>()
	}

	/// Implementation of the algorithm to find curve intersections by iterating on bounding boxes.
	/// - `self_original_t_interval` - Used to identify the `t` values of the original parent of `self` that the current iteration is representing.
	/// - `other_original_t_interval` - Used to identify the `t` values of the original parent of `other` that the current iteration is representing.
	pub(crate) fn intersections_between_subcurves(&self, self_original_t_interval: Range<f64>, other: &Bezier, other_original_t_interval: Range<f64>, error: f64) -> Vec<[f64; 2]> {
		let bounding_box1 = self.bounding_box();
		let bounding_box2 = other.bounding_box();

		// Get the `t` interval of the original parent of `self` and determine the middle `t` value
		let Range { start: self_start_t, end: self_end_t } = self_original_t_interval;
		let self_mid_t = (self_start_t + self_end_t) / 2.;

		// Get the `t` interval of the original parent of `other` and determine the middle `t` value
		let Range {
			start: other_start_t,
			end: other_end_t,
		} = other_original_t_interval;
		let other_mid_t = (other_start_t + other_end_t) / 2.;

		let error_threshold = DVec2::new(error, error);

		// Check if the bounding boxes overlap
		if utils::do_rectangles_overlap(bounding_box1, bounding_box2) {
			// If bounding boxes are within the error threshold (i.e. are small enough), we have found an intersection
			if (bounding_box1[1] - bounding_box1[0]).cmplt(error_threshold).all() && (bounding_box2[1] - bounding_box2[0]).cmplt(error_threshold).all() {
				// Use the middle t value, return the corresponding `t` value for `self` and `other`
				return vec![[self_mid_t, other_mid_t]];
			}

			// Split curves in half and repeat with the combinations of the two halves of each curve
			let [split_1_a, split_1_b] = self.split(TValue::Parametric(0.5));
			let [split_2_a, split_2_b] = other.split(TValue::Parametric(0.5));

			[
				split_1_a.intersections_between_subcurves(self_start_t..self_mid_t, &split_2_a, other_start_t..other_mid_t, error),
				split_1_a.intersections_between_subcurves(self_start_t..self_mid_t, &split_2_b, other_mid_t..other_end_t, error),
				split_1_b.intersections_between_subcurves(self_mid_t..self_end_t, &split_2_a, other_start_t..other_mid_t, error),
				split_1_b.intersections_between_subcurves(self_mid_t..self_end_t, &split_2_b, other_mid_t..other_end_t, error),
			]
			.concat()
		} else {
			vec![]
		}
	}

	// TODO: Use an `impl Iterator` return type instead of a `Vec`
	/// Returns a list of filtered parametric `t` values that correspond to intersection points between the current bezier curve and the provided one
	/// such that the difference between adjacent `t` values in sorted order is greater than some minimum separation value. If the difference
	/// between 2 adjacent `t` values is less than the minimum difference, the filtering takes the larger `t` value and discards the smaller `t` value.
	/// The returned `t` values are with respect to the current bezier, not the provided parameter.
	/// If the provided curve is linear, then zero intersection points will be returned along colinear segments.
	/// - `error` - For intersections where the provided bezier is non-linear, `error` defines the threshold for bounding boxes to be considered an intersection point.
	/// - `minimum_separation` - The minimum difference between adjacent `t` values in sorted order
	/// <iframe frameBorder="0" width="100%" height="375px" src="https://graphite.rs/libraries/bezier-rs#bezier/intersect-cubic/solo" title="Intersections Demo"></iframe>
	pub fn intersections(&self, other: &Bezier, error: Option<f64>, minimum_separation: Option<f64>) -> Vec<f64> {
		// TODO: Consider using the `intersections_between_vectors_of_curves` helper function here
		// Otherwise, use bounding box to determine intersections
		let mut intersection_t_values = self.unfiltered_intersections(other, error);
		intersection_t_values.sort_by(|a, b| a.partial_cmp(b).unwrap());

		intersection_t_values.iter().fold(Vec::new(), |mut accumulator, t| {
			if !accumulator.is_empty() && (accumulator.last().unwrap() - t).abs() < minimum_separation.unwrap_or(MIN_SEPARATION_VALUE) {
				accumulator.pop();
			}
			accumulator.push(*t);
			accumulator
		})
	}

	// TODO: Use an `impl Iterator` return type instead of a `Vec`
	/// Returns a list of `t` values that correspond to intersection points between the current bezier curve and the provided one. The returned `t` values are with respect to the current bezier, not the provided parameter.
	/// If the provided curve is linear, then zero intersection points will be returned along colinear segments.
	/// - `error` - For intersections where the provided bezier is non-linear, `error` defines the threshold for bounding boxes to be considered an intersection point.
	fn unfiltered_intersections(&self, other: &Bezier, error: Option<f64>) -> Vec<f64> {
		let error = error.unwrap_or(0.5);
		if other.handles == BezierHandles::Linear {
			// Rotate the bezier and the line by the angle that the line makes with the x axis
			let line_directional_vector = other.end - other.start;
			let angle = line_directional_vector.angle_between(DVec2::new(0., 1.));
			let rotation_matrix = DMat2::from_angle(angle);
			let rotated_bezier = self.apply_transformation(|point| rotation_matrix * point);

			// Translate the bezier such that the line becomes aligned on top of the x-axis
			let vertical_distance = (rotation_matrix * other.start).x;
			let translated_bezier = rotated_bezier.translate(DVec2::new(-vertical_distance, 0.));

			// Compute the roots of the resulting bezier curve
			let list_intersection_t = translated_bezier.find_tvalues_for_x(0.);

			// Calculate line's bounding box
			let [min_corner, max_corner] = other.bounding_box_of_anchors_and_handles();

			return list_intersection_t
				// Accept the t value if it is approximately in [0, 1] and if the corresponding coordinates are within the range of the linear line
				.filter(|&t| utils::dvec2_approximately_in_range(self.unrestricted_parametric_evaluate(t), min_corner, max_corner, MAX_ABSOLUTE_DIFFERENCE).all())
				// Ensure the returned value is within the correct range
				.map(|t| t.clamp(0., 1.))
				.collect::<Vec<f64>>();
		}

		// TODO: Consider using the `intersections_between_vectors_of_curves` helper function here
		// Otherwise, use bounding box to determine intersections
		self.intersections_between_subcurves(0. ..1., other, 0. ..1., error).iter().map(|t_values| t_values[0]).collect()
	}

	/// Returns a list of `t` values that correspond to points on this Bezier segment where they intersect with the given line. (`direction_vector` does not need to be normalized.)
	/// If this needs to be called frequently with a line of the same rotation angle, consider instead using [`line_test_crossings_prerotated`] and moving this function's setup code into your own logic before the repeated call.
	pub fn line_test_crossings(&self, point_on_line: DVec2, direction_vector: DVec2) -> impl Iterator<Item = f64> + '_ {
		// Rotate the bezier and the line by the angle that the line makes with the x axis
		let angle = direction_vector.angle_between(DVec2::new(0., 1.));
		let rotation_matrix = DMat2::from_angle(angle);
		let rotated_bezier = self.apply_transformation(|point| rotation_matrix * point);

		self.line_test_crossings_prerotated(point_on_line, rotation_matrix, rotated_bezier)
	}

	/// Returns a list of `t` values that correspond to points on this Bezier segment where they intersect with the given infinite line.
	/// This version of the function is for better performance when calling it frequently without needing to change the rotation between each call.
	/// If that isn't important, use [`line_test_crossings`] which wraps this and provides an easier interface by taking a line rotation vector.
	/// Instead, this version requires a rotation matrix for the line's rotation and a version of this Bezier segment that has had its rotation already applied.
	pub fn line_test_crossings_prerotated(&self, point_on_line: DVec2, rotation_matrix: DMat2, rotated_bezier: Self) -> impl Iterator<Item = f64> + '_ {
		// Translate the bezier such that the line becomes aligned on top of the x-axis
		let vertical_distance = (rotation_matrix.x_axis.x * point_on_line.x) + (rotation_matrix.y_axis.x * point_on_line.y);
		let translated_bezier = rotated_bezier.translate(DVec2::new(-vertical_distance, 0.));

		// Compute the roots of the resulting bezier curve
		translated_bezier.find_tvalues_for_x(0.)
	}

	/// Returns a list of `t` values that correspond to points on this Bezier segment where they intersect with the given ray. (`ray_direction` does not need to be normalized.)
	/// If this needs to be called frequently with a ray of the same rotation angle, consider instead using [`ray_test_crossings_prerotated`] and moving this function's setup code into your own logic before the repeated call.
	pub fn ray_test_crossings(&self, ray_start: DVec2, ray_direction: DVec2) -> impl Iterator<Item = f64> + '_ {
		// Rotate the bezier and the line by the angle that the line makes with the x axis
		let angle = ray_direction.angle_between(DVec2::new(0., 1.));
		let rotation_matrix = DMat2::from_angle(angle);
		let rotated_bezier = self.apply_transformation(|point| rotation_matrix * point);

		self.ray_test_crossings_prerotated(ray_start, rotation_matrix, rotated_bezier)
	}

	/// Returns a list of `t` values that correspond to points on this Bezier segment where they intersect with the given infinite ray.
	/// This version of the function is for better performance when calling it frequently without needing to change the rotation between each call.
	/// If that isn't important, use [`ray_test_crossings`] which wraps this and provides an easier interface by taking a ray direction vector.
	/// Instead, this version requires a rotation matrix for the ray's rotation and a version of this Bezier segment that has had its rotation already applied.
	pub fn ray_test_crossings_prerotated(&self, ray_start: DVec2, rotation_matrix: DMat2, rotated_bezier: Self) -> impl Iterator<Item = f64> + '_ {
		// Intersection t-values include those beyond the [0-1] range where the segment's ends extend through the X-axis
		let intersection_t_values_on_rotated_bezier = self.line_test_crossings_prerotated(ray_start, rotation_matrix, rotated_bezier);

		intersection_t_values_on_rotated_bezier
			// Accept the t value if it is approximately in [0, 1] and if the corresponding coordinates are within the range of the linear line
			.filter(move |&t| {
				let point = self.unrestricted_parametric_evaluate(t);
				// Ensure the returned value is within the correct range
				let in_bounds = point.cmpge(ray_start) | utils::dvec2_compare(point, ray_start, MAX_ABSOLUTE_DIFFERENCE);
				in_bounds.x && in_bounds.y
			})
	}

	/// Helper function to compute intersections between lists of subcurves.
	/// This function uses the algorithm implemented in `intersections_between_subcurves`.
	fn intersections_between_vectors_of_curves(subcurves1: &[(Bezier, Range<f64>)], subcurves2: &[(Bezier, Range<f64>)], error: f64) -> Vec<[f64; 2]> {
		let segment_pairs = subcurves1.iter().flat_map(move |(curve1, curve1_t_pair)| {
			subcurves2
				.iter()
				.filter_map(move |(curve2, curve2_t_pair)| utils::do_rectangles_overlap(curve1.bounding_box(), curve2.bounding_box()).then_some((curve1, curve1_t_pair, curve2, curve2_t_pair)))
		});
		segment_pairs
			.flat_map(|(curve1, curve1_t_pair, curve2, curve2_t_pair)| curve1.intersections_between_subcurves(curve1_t_pair.clone(), curve2, curve2_t_pair.clone(), error))
			.collect::<Vec<[f64; 2]>>()
	}

	// TODO: Use an `impl Iterator` return type instead of a `Vec`
	/// Returns a list of parametric `t` values that correspond to the self intersection points of the current bezier curve. For each intersection point, the returned `t` value is the smaller of the two that correspond to the point.
	/// - `error` - For intersections with non-linear beziers, `error` defines the threshold for bounding boxes to be considered an intersection point.
	/// <iframe frameBorder="0" width="100%" height="325px" src="https://graphite.rs/libraries/bezier-rs#bezier/intersect-self/solo" title="Self Intersection Demo"></iframe>
	pub fn self_intersections(&self, error: Option<f64>) -> Vec<[f64; 2]> {
		if self.handles == BezierHandles::Linear || matches!(self.handles, BezierHandles::Quadratic { .. }) {
			return vec![];
		}

		let error = error.unwrap_or(0.5);

		// Get 2 copies of the reduced curves
		let (self1, self1_t_values) = self.reduced_curves_and_t_values(None);
		let (self2, self2_t_values) = (self1.clone(), self1_t_values.clone());
		let num_curves = self1.len();

		// Adjacent reduced curves cannot intersect
		if num_curves <= 2 {
			return vec![];
		}

		// Create iterators that combine a subcurve with the `t` value pair that it was trimmed with
		let combined_iterator1 = self1.into_iter().zip(self1_t_values.iter().map(|t_pair| Range { start: t_pair[0], end: t_pair[1] }));
		// Second one needs to be a list because Iterator does not implement copy
		let combined_list2: Vec<(Bezier, Range<f64>)> = self2.into_iter().zip(self2_t_values.iter().map(|t_pair| Range { start: t_pair[0], end: t_pair[1] })).collect();

		// For each curve, look for intersections with every curve that is at least 2 indices away
		combined_iterator1
			.take(num_curves - 2)
			.enumerate()
			.flat_map(|(index, (subcurve, t_pair))| Bezier::intersections_between_vectors_of_curves(&[(subcurve, t_pair)], &combined_list2[index + 2..], error))
			.collect()
	}

	/// Returns a list of parametric `t` values that correspond to the intersection points between the curve and a rectangle defined by opposite corners.
	/// <iframe frameBorder="0" width="100%" height="300px" src="https://graphite.rs/libraries/bezier-rs#bezier/intersect-rectangle/solo" title="Intersection (Rectangle) Demo"></iframe>
	pub fn rectangle_intersections(&self, corner1: DVec2, corner2: DVec2) -> Vec<f64> {
		[
			Bezier::from_linear_coordinates(corner1.x, corner1.y, corner2.x, corner1.y),
			Bezier::from_linear_coordinates(corner2.x, corner1.y, corner2.x, corner2.y),
			Bezier::from_linear_coordinates(corner2.x, corner2.y, corner1.x, corner2.y),
			Bezier::from_linear_coordinates(corner1.x, corner2.y, corner1.x, corner1.y),
		]
		.iter()
		.flat_map(|bezier| self.intersections(bezier, None, None))
		.collect()
	}

	/// Returns a cubic bezier which joins this with the provided bezier curve.
	/// The resulting path formed by the Bezier curves is continuous up to the first derivative.
	/// <iframe frameBorder="0" width="100%" height="300px" src="https://graphite.rs/libraries/bezier-rs#bezier/join/solo" title="Join Demo"></iframe>
	pub fn join(&self, other: &Bezier) -> Bezier {
		let handle1 = self.non_normalized_tangent(1.) / 3. + self.end;
		let handle2 = other.start - other.non_normalized_tangent(0.) / 3.;
		Bezier::from_cubic_dvec2(self.end, handle1, handle2, other.start)
	}

	/// Compute the winding order (number of times crossing an infinate line to the left of the point)
	///
	/// Assumes curve is split at the extrema.
	fn pre_split_winding_number(&self, target_point: DVec2) -> i32 {
		// Clockwise is -1, anticlockwise is +1 (with +y as up)
		// Looking only to the left (-x) of the target_point
		let resulting_sign = if self.end.y > self.start.y {
			if target_point.y < self.start.y || target_point.y >= self.end.y {
				return 0;
			}
			-1
		} else if self.end.y < self.start.y {
			if target_point.y < self.end.y || target_point.y >= self.start.y {
				return 0;
			}
			1
		} else {
			return 0;
		};
		match &self.handles {
			BezierHandles::Linear => {
				if target_point.x < self.start.x.min(self.end.x) {
					return 0;
				}
				if target_point.x >= self.start.x.max(self.end.x) {
					return resulting_sign;
				}
				// line equation ax + by = c
				let a = self.end.y - self.start.y;
				let b = self.start.x - self.end.x;
				let c = a * self.start.x + b * self.start.y;
				if (a * target_point.x + b * target_point.y - c) * (resulting_sign as f64) <= 0. {
					resulting_sign
				} else {
					0
				}
			}
			BezierHandles::Quadratic { handle: p1 } => {
				if target_point.x < self.start.x.min(self.end.x).min(p1.x) {
					return 0;
				}
				if target_point.x >= self.start.x.max(self.end.x).max(p1.x) {
					return resulting_sign;
				}
				let a = self.end.y - 2. * p1.y + self.start.y;
				let b = 2. * (p1.y - self.start.y);
				let c = self.start.y - target_point.y;

				let discriminant = b * b - 4. * a * c;
				let two_times_a = 2. * a;
				for t in solve_quadratic(discriminant, two_times_a, b, c).into_iter().flatten() {
					if (0.0..=1.).contains(&t) {
						let x = self.evaluate(TValue::Parametric(t)).x;
						if target_point.x >= x {
							return resulting_sign;
						} else {
							return 0;
						}
					}
				}
				0
			}
			BezierHandles::Cubic { handle_start: p1, handle_end: p2 } => {
				if target_point.x < self.start.x.min(self.end.x).min(p1.x).min(p2.x) {
					return 0;
				}
				if target_point.x >= self.start.x.max(self.end.x).max(p1.x).max(p2.x) {
					return resulting_sign;
				}
				let a = self.end.y - 3. * p2.y + 3. * p1.y - self.start.y;
				let b = 3. * (p2.y - 2. * p1.y + self.start.y);
				let c = 3. * (p1.y - self.start.y);
				let d = self.start.y - target_point.y;
				for t in solve_cubic(a, b, c, d).into_iter().flatten() {
					if (0.0..=1.).contains(&t) {
						let x = self.evaluate(TValue::Parametric(t)).x;
						if target_point.x >= x {
							return resulting_sign;
						} else {
							return 0;
						}
					}
				}
				0
			}
		}
	}

	/// Compute the winding number contribution of a single segment.
	///
	/// Cast a ray to the left and count intersections.
	pub fn winding(&self, target_point: DVec2) -> i32 {
		let [x_extrema_t, y_extrema_t] = self.unrestricted_local_extrema();
		let mut x_extrema_t = x_extrema_t.map(|t| t.filter(|&t| t > 0. && t < 1.));
		let mut y_extrema_t = y_extrema_t.map(|t| t.filter(|&t| t > 0. && t < 1.));

		let mut results = [None; 8];
		results[7] = Some(1.);
		for i in (0..7).rev() {
			let Some(min) = x_extrema_t.iter_mut().chain(y_extrema_t.iter_mut()).max_by(|a, b| a.partial_cmp(b).unwrap()) else {
				results[i] = Some(0.);
				break;
			};
			if let Some(value) = min.take() {
				results[i] = Some(value);
			} else {
				results[i] = Some(0.);
				break;
			}
		}
		results
			.windows(2)
			.flat_map(|t| t[0].and_then(|first| t[1].map(|second| [first, second])))
			.map(|t| self.trim(TValue::Parametric(t[0]), TValue::Parametric(t[1])).pre_split_winding_number(target_point))
			.sum()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::compare::{compare_f64s, compare_points, compare_vec_of_points};

	#[test]
	fn test_de_casteljau_points() {
		let bezier = Bezier::from_cubic_coordinates(0., 0., 0., 100., 100., 100., 100., 0.);
		let de_casteljau_points = bezier.de_casteljau_points(TValue::Parametric(0.5));
		let expected_de_casteljau_points = vec![
			vec![DVec2::new(0., 0.), DVec2::new(0., 100.), DVec2::new(100., 100.), DVec2::new(100., 0.)],
			vec![DVec2::new(0., 50.), DVec2::new(50., 100.), DVec2::new(100., 50.)],
			vec![DVec2::new(25., 75.), DVec2::new(75., 75.)],
			vec![DVec2::new(50., 75.)],
		];
		assert_eq!(&de_casteljau_points, &expected_de_casteljau_points);

		assert_eq!(expected_de_casteljau_points[3][0], bezier.evaluate(TValue::Parametric(0.5)));
	}

	#[test]
	fn test_derivative() {
		// Test derivatives of each Bezier curve type
		let p1 = DVec2::new(10., 10.);
		let p2 = DVec2::new(40., 30.);
		let p3 = DVec2::new(60., 60.);
		let p4 = DVec2::new(70., 100.);

		let linear = Bezier::from_linear_dvec2(p1, p2);
		assert!(linear.derivative().is_none());

		let quadratic = Bezier::from_quadratic_dvec2(p1, p2, p3);
		let derivative_quadratic = quadratic.derivative().unwrap();
		assert_eq!(derivative_quadratic, Bezier::from_linear_coordinates(60., 40., 40., 60.));

		let cubic = Bezier::from_cubic_dvec2(p1, p2, p3, p4);
		let derivative_cubic = cubic.derivative().unwrap();
		assert_eq!(derivative_cubic, Bezier::from_quadratic_coordinates(90., 60., 60., 90., 30., 120.));

		// Cases where the all manipulator points are the same
		let quadratic_point = Bezier::from_quadratic_dvec2(p1, p1, p1);
		assert_eq!(quadratic_point.derivative().unwrap(), Bezier::from_linear_dvec2(DVec2::ZERO, DVec2::ZERO));

		let cubic_point = Bezier::from_cubic_dvec2(p1, p1, p1, p1);
		assert_eq!(cubic_point.derivative().unwrap(), Bezier::from_quadratic_dvec2(DVec2::ZERO, DVec2::ZERO, DVec2::ZERO));
	}

	#[test]
	fn test_tangent() {
		// Test tangents at start and end points of each Bezier curve type
		let p1 = DVec2::new(10., 10.);
		let p2 = DVec2::new(40., 30.);
		let p3 = DVec2::new(60., 60.);
		let p4 = DVec2::new(70., 100.);

		let linear = Bezier::from_linear_dvec2(p1, p2);
		let unit_slope = DVec2::new(30., 20.).normalize();
		assert_eq!(linear.tangent(TValue::Parametric(0.)), unit_slope);
		assert_eq!(linear.tangent(TValue::Parametric(1.)), unit_slope);

		let quadratic = Bezier::from_quadratic_dvec2(p1, p2, p3);
		assert_eq!(quadratic.tangent(TValue::Parametric(0.)), DVec2::new(60., 40.).normalize());
		assert_eq!(quadratic.tangent(TValue::Parametric(1.)), DVec2::new(40., 60.).normalize());

		let cubic = Bezier::from_cubic_dvec2(p1, p2, p3, p4);
		assert_eq!(cubic.tangent(TValue::Parametric(0.)), DVec2::new(90., 60.).normalize());
		assert_eq!(cubic.tangent(TValue::Parametric(1.)), DVec2::new(30., 120.).normalize());
	}

	#[test]
	fn tangent_at_point() {
		let validate = |bz: Bezier, p: DVec2| {
			let solutions = bz.tangents_to_point(p);
			assert_ne!(solutions.len(), 0);
			for t in solutions {
				let pos = bz.evaluate(TValue::Parametric(t));
				let expected_tangent = (pos - p).normalize();
				let tangent = bz.tangent(TValue::Parametric(t));
				assert!(expected_tangent.perp_dot(tangent).abs() < 0.2, "Expected tangent {expected_tangent} found {tangent} pos {pos}")
			}
		};
		let bz = Bezier::from_quadratic_coordinates(55., 50., 165., 30., 185., 170.);
		let p = DVec2::new(193., 83.);
		validate(bz, p);

		let bz = Bezier::from_cubic_coordinates(55., 30., 18., 139., 175., 30., 185., 160.);
		let p = DVec2::new(127., 121.);
		validate(bz, p);
	}

	#[test]
	fn test_normal() {
		// Test normals at start and end points of each Bezier curve type
		let p1 = DVec2::new(10., 10.);
		let p2 = DVec2::new(40., 30.);
		let p3 = DVec2::new(60., 60.);
		let p4 = DVec2::new(70., 100.);

		let linear = Bezier::from_linear_dvec2(p1, p2);
		let unit_slope = DVec2::new(-20., 30.).normalize();
		assert_eq!(linear.normal(TValue::Parametric(0.)), unit_slope);
		assert_eq!(linear.normal(TValue::Parametric(1.)), unit_slope);

		let quadratic = Bezier::from_quadratic_dvec2(p1, p2, p3);
		assert_eq!(quadratic.normal(TValue::Parametric(0.)), DVec2::new(-40., 60.).normalize());
		assert_eq!(quadratic.normal(TValue::Parametric(1.)), DVec2::new(-60., 40.).normalize());

		let cubic = Bezier::from_cubic_dvec2(p1, p2, p3, p4);
		assert_eq!(cubic.normal(TValue::Parametric(0.)), DVec2::new(-60., 90.).normalize());
		assert_eq!(cubic.normal(TValue::Parametric(1.)), DVec2::new(-120., 30.).normalize());
	}

	#[test]
	fn normal_at_point() {
		let validate = |bz: Bezier, p: DVec2| {
			let solutions = bz.normals_to_point(p);
			assert_ne!(solutions.len(), 0);
			for t in solutions {
				let pos = bz.evaluate(TValue::Parametric(t));
				let expected_normal = (pos - p).normalize();
				let normal = bz.normal(TValue::Parametric(t));
				assert!(expected_normal.perp_dot(normal).abs() < 0.2, "Expected normal {expected_normal} found {normal} pos {pos}")
			}
		};

		let bz = Bezier::from_linear_coordinates(50., 50., 100., 100.);
		let p = DVec2::new(100., 50.);
		validate(bz, p);

		let bz = Bezier::from_quadratic_coordinates(55., 50., 165., 30., 185., 170.);
		let p = DVec2::new(193., 83.);
		validate(bz, p);

		let bz = Bezier::from_cubic_coordinates(55., 30., 18., 139., 175., 30., 185., 160.);
		let p = DVec2::new(127., 121.);
		validate(bz, p);

		let bz = Bezier::from_cubic_coordinates(55., 30., 85., 140., 175., 30., 185., 160.);
		let p = DVec2::new(17., 172.);
		validate(bz, p);
	}

	#[test]
	fn test_curvature() {
		let p1 = DVec2::new(10., 10.);
		let p2 = DVec2::new(50., 10.);
		let p3 = DVec2::new(50., 50.);
		let p4 = DVec2::new(50., 10.);

		let linear = Bezier::from_linear_dvec2(p1, p2);
		assert_eq!(linear.curvature(TValue::Parametric(0.)), 0.);
		assert_eq!(linear.curvature(TValue::Parametric(0.5)), 0.);
		assert_eq!(linear.curvature(TValue::Parametric(1.)), 0.);

		let quadratic = Bezier::from_quadratic_dvec2(p1, p2, p3);
		assert!(compare_f64s(quadratic.curvature(TValue::Parametric(0.)), 0.0125));
		assert!(compare_f64s(quadratic.curvature(TValue::Parametric(0.5)), 0.035355));
		assert!(compare_f64s(quadratic.curvature(TValue::Parametric(1.)), 0.0125));

		let cubic = Bezier::from_cubic_dvec2(p1, p2, p3, p4);
		assert!(compare_f64s(cubic.curvature(TValue::Parametric(0.)), 0.016667));
		assert!(compare_f64s(cubic.curvature(TValue::Parametric(0.5)), 0.));
		assert!(compare_f64s(cubic.curvature(TValue::Parametric(1.)), 0.));

		// The curvature at an inflection point is zero
		let inflection_curve = Bezier::from_cubic_coordinates(30., 30., 30., 150., 150., 30., 150., 150.);
		let inflections = inflection_curve.inflections();
		assert_eq!(inflection_curve.curvature(TValue::Parametric(inflections[0])), 0.);
	}

	#[test]
	fn test_extrema_linear() {
		// Linear bezier cannot have extrema
		let line = Bezier::from_linear_dvec2(DVec2::new(10., 10.), DVec2::new(50., 50.));
		let [x_extrema, y_extrema] = line.local_extrema();
		assert_eq!(y_extrema.count(), 0);
		assert_eq!(x_extrema.count(), 0);
	}

	#[test]
	fn test_extrema_quadratic() {
		// Test with no x-extrema, no y-extrema
		let bezier1 = Bezier::from_quadratic_coordinates(40., 35., 149., 54., 155., 170.);
		let [x_extrema1, y_extrema1] = bezier1.local_extrema();
		assert_eq!(x_extrema1.count(), 0);
		assert_eq!(y_extrema1.count(), 0);

		// Test with 1 x-extrema, no y-extrema
		let bezier2 = Bezier::from_quadratic_coordinates(45., 30., 170., 90., 45., 150.);
		let [x_extrema2, y_extrema2] = bezier2.local_extrema();
		assert_eq!(x_extrema2.count(), 1);
		assert_eq!(y_extrema2.count(), 0);

		// Test with no x-extrema, 1 y-extrema
		let bezier3 = Bezier::from_quadratic_coordinates(30., 130., 100., 25., 150., 130.);
		let [x_extrema3, y_extrema3] = bezier3.local_extrema();
		assert_eq!(x_extrema3.count(), 0);
		assert_eq!(y_extrema3.count(), 1);

		// Test with 1 x-extrema, 1 y-extrema
		let bezier4 = Bezier::from_quadratic_coordinates(50., 70., 170., 35., 60., 150.);
		let [x_extrema4, y_extrema4] = bezier4.local_extrema();
		assert_eq!(x_extrema4.count(), 1);
		assert_eq!(y_extrema4.count(), 1);
	}

	#[test]
	fn test_extrema_cubic() {
		// 0 x-extrema, 0 y-extrema
		let bezier1 = Bezier::from_cubic_coordinates(100., 105., 250., 250., 110., 150., 260., 260.);
		let [x_extrema1, y_extrema1] = bezier1.local_extrema();
		assert_eq!(x_extrema1.count(), 0);
		assert_eq!(y_extrema1.count(), 0);

		// 1 x-extrema, 0 y-extrema
		let bezier2 = Bezier::from_cubic_coordinates(55., 145., 40., 40., 110., 110., 180., 40.);
		let [x_extrema2, y_extrema2] = bezier2.local_extrema();
		assert_eq!(x_extrema2.count(), 1);
		assert_eq!(y_extrema2.count(), 0);

		// 1 x-extrema, 1 y-extrema
		let bezier3 = Bezier::from_cubic_coordinates(100., 105., 170., 10., 25., 20., 20., 120.);
		let [x_extrema3, y_extrema3] = bezier3.local_extrema();
		assert_eq!(x_extrema3.count(), 1);
		assert_eq!(y_extrema3.count(), 1);

		// 1 x-extrema, 2 y-extrema
		let bezier4 = Bezier::from_cubic_coordinates(50., 90., 120., 16., 150., 190., 45., 150.);
		let [x_extrema4, y_extrema4] = bezier4.local_extrema();
		assert_eq!(x_extrema4.count(), 1);
		assert_eq!(y_extrema4.count(), 2);

		// 2 x-extrema, 0 y-extrema
		let bezier5 = Bezier::from_cubic_coordinates(40., 170., 150., 160., 10., 10., 170., 10.);
		let [x_extrema5, y_extrema5] = bezier5.local_extrema();
		assert_eq!(x_extrema5.count(), 2);
		assert_eq!(y_extrema5.count(), 0);

		// 2 x-extrema, 1 y-extrema
		let bezier6 = Bezier::from_cubic_coordinates(40., 170., 150., 160., 10., 10., 160., 45.);
		let [x_extrema6, y_extrema6] = bezier6.local_extrema();
		assert_eq!(x_extrema6.count(), 2);
		assert_eq!(y_extrema6.count(), 1);

		// 2 x-extrema, 2 y-extrema
		let bezier7 = Bezier::from_cubic_coordinates(46., 60., 140., 10., 50., 160., 120., 120.);
		let [x_extrema7, y_extrema7] = bezier7.local_extrema();
		assert_eq!(x_extrema7.count(), 2);
		assert_eq!(y_extrema7.count(), 2);
	}

	#[test]
	fn test_bounding_box() {
		// Case where the start and end points dictate the bounding box
		let bezier_simple = Bezier::from_linear_coordinates(0., 0., 10., 10.);
		assert_eq!(bezier_simple.bounding_box(), [DVec2::new(0., 0.), DVec2::new(10., 10.)]);

		// Case where the curve's extrema dictate the bounding box
		let bezier_complex = Bezier::from_cubic_coordinates(90., 70., 25., 25., 175., 175., 110., 130.);
		assert!(compare_vec_of_points(
			bezier_complex.bounding_box().to_vec(),
			vec![DVec2::new(73.2774, 61.4755), DVec2::new(126.7226, 138.5245)],
			1e-3
		));
	}

	#[test]
	fn test_find_tvalues_for_x() {
		struct Assertion {
			bezier: Bezier,
			x: f64,
			ys: &'static [f64],
		}

		let assertions = [
			Assertion {
				bezier: Bezier::from_linear_coordinates(0., 0., 20., 10.),
				x: 5.,
				ys: &[2.5],
			},
			Assertion {
				bezier: Bezier::from_quadratic_coordinates(0., 0., 10., 5., 20., 10.),
				x: 5.,
				ys: &[2.5],
			},
			Assertion {
				bezier: Bezier::from_cubic_coordinates(0., 0., 10., 5., 10., 5., 20., 10.),
				x: 5.,
				ys: &[2.5],
			},
			Assertion {
				bezier: Bezier::from_cubic_coordinates(90., 70., 25., 25., 175., 175., 110., 130.),
				x: 100.,
				ys: &[100.],
			},
			Assertion {
				bezier: Bezier::from_cubic_coordinates(90., 70., 25., 25., 175., 175., 110., 130.),
				x: 80.,
				ys: &[63.62683, 74.53867],
			},
			Assertion {
				bezier: Bezier::from_cubic_coordinates(110., 70., 25., 25., 175., 175., 90., 130.),
				x: 100.,
				ys: &[65.11345, 100., 134.88655],
			},
		];

		for Assertion { bezier, x, ys } in assertions {
			let mut got: Vec<f64> = bezier
				.find_tvalues_for_x(x)
				.map(|t| bezier.evaluate(TValue::Parametric(t)))
				.inspect(|p| assert!((p.x - x).abs() < 1e-4, "wrong x-coordinate, got {} expected {x}", p.x))
				.map(|p| p.y)
				.collect();
			assert_eq!(got.len(), ys.len());
			got.sort_by(f64::total_cmp);
			got.into_iter()
				.zip(ys)
				.for_each(|(got, &expected)| assert!((got - expected).abs() < 1e-4, "wrong y-coordinate, got {got} expected {expected}"));
		}
	}

	#[test]
	fn test_inflections() {
		let bezier = Bezier::from_cubic_coordinates(30., 30., 30., 150., 150., 30., 150., 150.);
		let inflections = bezier.inflections();
		assert_eq!(inflections.len(), 1);
		assert_eq!(inflections[0], 0.5);
	}

	#[test]
	fn test_intersect_line_segment_linear() {
		let p1 = DVec2::new(30., 60.);
		let p2 = DVec2::new(140., 120.);

		// Intersection at edge of curve
		let bezier = Bezier::from_linear_dvec2(p1, p2);
		let line1 = Bezier::from_linear_coordinates(20., 60., 70., 60.);
		let intersections1 = bezier.intersections(&line1, None, None);
		assert!(intersections1.len() == 1);
		assert!(compare_points(bezier.evaluate(TValue::Parametric(intersections1[0])), DVec2::new(30., 60.)));

		// Intersection in the middle of curve
		let line2 = Bezier::from_linear_coordinates(150., 150., 30., 30.);
		let intersections2 = bezier.intersections(&line2, None, None);
		assert!(compare_points(bezier.evaluate(TValue::Parametric(intersections2[0])), DVec2::new(96., 96.)));
	}

	#[test]
	fn test_intersect_line_segment_quadratic() {
		let p1 = DVec2::new(30., 50.);
		let p2 = DVec2::new(140., 30.);
		let p3 = DVec2::new(160., 170.);

		// Intersection at edge of curve
		let bezier = Bezier::from_quadratic_dvec2(p1, p2, p3);
		let line1 = Bezier::from_linear_coordinates(20., 50., 40., 50.);
		let intersections1 = bezier.intersections(&line1, None, None);
		assert!(intersections1.len() == 1);
		assert!(compare_points(bezier.evaluate(TValue::Parametric(intersections1[0])), p1));

		// Intersection in the middle of curve
		let line2 = Bezier::from_linear_coordinates(150., 150., 30., 30.);
		let intersections2 = bezier.intersections(&line2, None, None);
		assert!(compare_points(bezier.evaluate(TValue::Parametric(intersections2[0])), DVec2::new(47.77355, 47.77354)));
	}

	#[test]
	fn test_intersect_line_segment_cubic() {
		let p1 = DVec2::new(30., 30.);
		let p2 = DVec2::new(60., 140.);
		let p3 = DVec2::new(150., 30.);
		let p4 = DVec2::new(160., 160.);

		let bezier = Bezier::from_cubic_dvec2(p1, p2, p3, p4);
		// Intersection at edge of curve, Discriminant > 0
		let line1 = Bezier::from_linear_coordinates(20., 30., 40., 30.);
		let intersections1 = bezier.intersections(&line1, None, None);
		assert!(intersections1.len() == 1);
		assert!(compare_points(bezier.evaluate(TValue::Parametric(intersections1[0])), p1));

		// Intersection at edge and in middle of curve, Discriminant < 0
		let line2 = Bezier::from_linear_coordinates(150., 150., 30., 30.);
		let intersections2 = bezier.intersections(&line2, None, None);
		assert!(intersections2.len() == 2);
		assert!(compare_points(bezier.evaluate(TValue::Parametric(intersections2[0])), p1));
		assert!(compare_points(bezier.evaluate(TValue::Parametric(intersections2[1])), DVec2::new(85.84, 85.84)));
	}

	#[test]
	fn test_intersect_curve_cubic_anchor_handle_overlap() {
		// M31 94 C40 40 107 107 106 106

		let p1 = DVec2::new(31., 94.);
		let p2 = DVec2::new(40., 40.);
		let p3 = DVec2::new(107., 107.);
		let p4 = DVec2::new(106., 106.);
		let bezier = Bezier::from_cubic_dvec2(p1, p2, p3, p4);

		let line = Bezier::from_linear_coordinates(150., 150., 20., 20.);
		let intersections = bezier.intersections(&line, None, None);

		assert_eq!(intersections.len(), 1);
		assert!(compare_points(bezier.evaluate(TValue::Parametric(intersections[0])), p4));
	}

	#[test]
	fn test_intersect_curve_cubic_edge_case() {
		// M34 107 C40 40 120 120 102 29

		let p1 = DVec2::new(34., 107.);
		let p2 = DVec2::new(40., 40.);
		let p3 = DVec2::new(120., 120.);
		let p4 = DVec2::new(102., 29.);
		let bezier = Bezier::from_cubic_dvec2(p1, p2, p3, p4);

		let line = Bezier::from_linear_coordinates(150., 150., 20., 20.);
		let intersections = bezier.intersections(&line, None, None);

		assert_eq!(intersections.len(), 1);
	}

	#[test]
	fn test_intersect_curve() {
		let bezier1 = Bezier::from_cubic_coordinates(30., 30., 60., 140., 150., 30., 160., 160.);
		let bezier2 = Bezier::from_quadratic_coordinates(175., 140., 20., 20., 120., 20.);

		let intersections1 = bezier1.intersections(&bezier2, None, None);
		let intersections2 = bezier2.intersections(&bezier1, None, None);

		let intersections1_points: Vec<DVec2> = intersections1.iter().map(|&t| bezier1.evaluate(TValue::Parametric(t))).collect();
		let intersections2_points: Vec<DVec2> = intersections2.iter().map(|&t| bezier2.evaluate(TValue::Parametric(t))).rev().collect();

		assert!(compare_vec_of_points(intersections1_points, intersections2_points, 2.));
	}

	#[test]
	fn test_intersect_with_self() {
		let bezier = Bezier::from_cubic_coordinates(160., 180., 170., 10., 30., 90., 180., 140.);
		let intersections = bezier.self_intersections(Some(0.5));
		assert!(compare_vec_of_points(
			intersections.iter().map(|&t| bezier.evaluate(TValue::Parametric(t[0]))).collect(),
			intersections.iter().map(|&t| bezier.evaluate(TValue::Parametric(t[1]))).collect(),
			2.
		));
		assert!(Bezier::from_linear_coordinates(160., 180., 170., 10.).self_intersections(None).is_empty());
		assert!(Bezier::from_quadratic_coordinates(160., 180., 170., 10., 30., 90.).self_intersections(None).is_empty());
	}
}

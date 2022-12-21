use super::*;
use crate::utils::ComputeType;

use glam::DMat2;
use std::ops::Range;

/// Functionality that solve for various curve information such as derivative, tangent, intersect, etc.
impl Bezier {
	/// Returns a list of lists of points representing the De Casteljau points for all iterations at the point corresponding to `t` using De Casteljau's algorithm.
	/// The `i`th element of the list represents the set of points in the `i`th iteration.
	/// More information on the algorithm can be found in the [De Casteljau section](https://pomax.github.io/bezierinfo/#decasteljau) in Pomax's primer.
	pub fn de_casteljau_points(&self, t: f64) -> Vec<Vec<DVec2>> {
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

	/// Returns a Bezier representing the derivative of the original curve.
	/// - This function returns `None` for a linear segment.
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

	/// Returns a normalized unit vector representing the tangent at the point designated by `t` on the curve.
	pub fn tangent(&self, t: f64) -> DVec2 {
		match self.handles {
			BezierHandles::Linear => self.end - self.start,
			_ => self.derivative().unwrap().evaluate(ComputeType::Parametric(t)),
		}
		.normalize()
	}

	/// Returns a normalized unit vector representing the direction of the normal at the point designated by `t` on the curve.
	pub fn normal(&self, t: f64) -> DVec2 {
		self.tangent(t).perp()
	}

	/// Returns the curvature, a scalar value for the derivative at the given `t`-value along the curve.
	/// Curvature is 1 over the radius of a circle with an equivalent derivative.
	pub fn curvature(&self, t: f64) -> f64 {
		let (d, dd) = match &self.derivative() {
			Some(first_derivative) => match first_derivative.derivative() {
				Some(second_derivative) => (first_derivative.evaluate(ComputeType::Parametric(t)), second_derivative.evaluate(ComputeType::Parametric(t))),
				None => (first_derivative.evaluate(ComputeType::Parametric(t)), first_derivative.end - first_derivative.start),
			},
			None => (self.end - self.start, DVec2::new(0., 0.)),
		};

		let numerator = d.x * dd.y - d.y * dd.x;
		let denominator = (d.x.powf(2.) + d.y.powf(2.)).powf(1.5);
		if denominator == 0. {
			0.
		} else {
			numerator / denominator
		}
	}

	/// Returns two lists of `t`-values representing the local extrema of the `x` and `y` parametric curves respectively.
	/// The local extrema are defined to be points at which the derivative of the curve is equal to zero.
	fn unrestricted_local_extrema(&self) -> [Vec<f64>; 2] {
		match self.handles {
			BezierHandles::Linear => [Vec::new(), Vec::new()],
			BezierHandles::Quadratic { handle } => {
				let a = handle - self.start;
				let b = self.end - handle;
				let b_minus_a = b - a;
				[utils::solve_linear(b_minus_a.x, a.x), utils::solve_linear(b_minus_a.y, a.y)]
			}
			BezierHandles::Cubic { handle_start, handle_end } => {
				let a = 3. * (-self.start + 3. * handle_start - 3. * handle_end + self.end);
				let b = 6. * (self.start - 2. * handle_start + handle_end);
				let c = 3. * (handle_start - self.start);
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
	pub fn local_extrema(&self) -> [Vec<f64>; 2] {
		self.unrestricted_local_extrema()
			.into_iter()
			.map(|t_values| t_values.into_iter().filter(|&t| t > 0. && t < 1.).collect::<Vec<f64>>())
			.collect::<Vec<Vec<f64>>>()
			.try_into()
			.unwrap()
	}

	/// Return the min and max corners that represent the bounding box of the curve.
	pub fn bounding_box(&self) -> [DVec2; 2] {
		// Start by taking min/max of endpoints.
		let mut endpoints_min = self.start.min(self.end);
		let mut endpoints_max = self.start.max(self.end);

		// Iterate through extrema points.
		let extrema = self.local_extrema();
		for t_values in extrema {
			for t in t_values {
				let point = self.evaluate(ComputeType::Parametric(t));
				// Update bounding box if new min/max is found.
				endpoints_min = endpoints_min.min(point);
				endpoints_max = endpoints_max.max(point);
			}
		}

		[endpoints_min, endpoints_max]
	}

	/// Returns `true` if the bounding box of the bezier is contained entirely within a rectangle defined by its minimum and maximum corners.
	pub fn is_contained_within(&self, min_corner: DVec2, max_corner: DVec2) -> bool {
		let [bounding_box_min, bounding_box_max] = self.bounding_box();
		min_corner.x <= bounding_box_min.x && min_corner.y <= bounding_box_min.y && bounding_box_max.x <= max_corner.x && bounding_box_max.y <= max_corner.y
	}

	// TODO: Use an `impl Iterator` return type instead of a `Vec`
	/// Returns list of `t`-values representing the inflection points of the curve.
	/// The inflection points are defined to be points at which the second derivative of the curve is equal to zero.
	pub fn unrestricted_inflections(&self) -> Vec<f64> {
		match self.handles {
			// There exists no inflection points for linear and quadratic beziers.
			BezierHandles::Linear => Vec::new(),
			BezierHandles::Quadratic { .. } => Vec::new(),
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
	}

	/// Returns list of `t`-values representing the inflection points of the curve.
	/// The list of `t`-values returned are filtered such that they fall within the range `[0, 1]`.
	pub fn inflections(&self) -> Vec<f64> {
		self.unrestricted_inflections().into_iter().filter(|&t| t > 0. && t < 1.).collect::<Vec<f64>>()
	}

	/// Implementation of the algorithm to find curve intersections by iterating on bounding boxes.
	/// - `self_original_t_interval` - Used to identify the `t` values of the original parent of `self` that the current iteration is representing.
	/// - `other_original_t_interval` - Used to identify the `t` values of the original parent of `other` that the current iteration is representing.
	fn intersections_between_subcurves(&self, self_original_t_interval: Range<f64>, other: &Bezier, other_original_t_interval: Range<f64>, error: f64) -> Vec<[f64; 2]> {
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
			let [split_1_a, split_1_b] = self.split(0.5);
			let [split_2_a, split_2_b] = other.split(0.5);

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
	/// Returns a list of filtered `t` values that correspond to intersection points between the current bezier curve and the provided one
	/// such that the difference between adjacent `t` values in sorted order is greater than some minimum seperation value. If the difference
	/// between 2 adjacent `t` values is lesss than the minimum difference, the filtering takes the larger `t` value and discards the smaller `t` value.
	/// The returned `t` values are with respect to the current bezier, not the provided parameter.
	/// If the provided curve is linear, then zero intersection points will be returned along colinear segments.
	/// - `error` - For intersections where the provided bezier is non-linear, `error` defines the threshold for bounding boxes to be considered an intersection point.
	/// - `minimum_seperation` - The minimum difference between adjacent `t` values in sorted order
	pub fn intersections(&self, other: &Bezier, error: Option<f64>, minimum_seperation: Option<f64>) -> Vec<f64> {
		// TODO: Consider using the `intersections_between_vectors_of_curves` helper function here
		// Otherwise, use bounding box to determine intersections
		let mut intersection_t_values = self.unfiltered_intersections(other, error);
		intersection_t_values.sort_by(|a, b| a.partial_cmp(b).unwrap());

		// println!("<<<<< intersection_t_values :: {:?}", intersection_t_values);

		intersection_t_values.iter().fold(Vec::new(), |mut accumulator, t| {
			if !accumulator.is_empty() && (accumulator.last().unwrap() - t).abs() < minimum_seperation.unwrap_or(MIN_SEPERATION_VALUE) {
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
			let angle = line_directional_vector.angle_between(DVec2::new(1., 0.));
			let rotation_matrix = DMat2::from_angle(angle);
			let rotated_bezier = self.apply_transformation(&|point| rotation_matrix.mul_vec2(point));
			let rotated_line = [rotation_matrix.mul_vec2(other.start), rotation_matrix.mul_vec2(other.end)];

			// Translate the bezier such that the line becomes aligned on top of the x-axis
			let vertical_distance = rotated_line[0].y;
			let translated_bezier = rotated_bezier.translate(DVec2::new(0., -vertical_distance));

			// Compute the roots of the resulting bezier curve
			let list_intersection_t = match translated_bezier.handles {
				BezierHandles::Linear => {
					// If the transformed linear bezier is on the x-axis, `a` and `b` will both be zero and `solve_linear` will return no roots
					let a = translated_bezier.end.y - translated_bezier.start.y;
					let b = translated_bezier.start.y;
					utils::solve_linear(a, b)
				}
				BezierHandles::Quadratic { handle } => {
					let a = translated_bezier.start.y - 2. * handle.y + translated_bezier.end.y;
					let b = 2. * (handle.y - translated_bezier.start.y);
					let c = translated_bezier.start.y;

					let discriminant = b * b - 4. * a * c;
					let two_times_a = 2. * a;

					utils::solve_quadratic(discriminant, two_times_a, b, c)
				}
				BezierHandles::Cubic { handle_start, handle_end } => {
					let start_y = translated_bezier.start.y;
					let a = -start_y + 3. * handle_start.y - 3. * handle_end.y + translated_bezier.end.y;
					let b = 3. * start_y - 6. * handle_start.y + 3. * handle_end.y;
					let c = -3. * start_y + 3. * handle_start.y;
					let d = start_y;

					utils::solve_cubic(a, b, c, d)
				}
			};

			let min = other.start.min(other.end);
			let max = other.start.max(other.end);

			return list_intersection_t
				.into_iter()
				// Accept the t value if it is approximately in [0, 1] and if the corresponding coordinates are within the range of the linear line
				.filter(|&t| {
					utils::f64_approximately_in_range(t, 0., 1., MAX_ABSOLUTE_DIFFERENCE)
						&& utils::dvec2_approximately_in_range(self.unrestricted_parametric_evaluate(t), min, max, MAX_ABSOLUTE_DIFFERENCE).all()
				})
				// Ensure the returned value is within the correct range
				.map(|t| t.clamp(0., 1.))
				.collect::<Vec<f64>>();
		}

		// TODO: Consider using the `intersections_between_vectors_of_curves` helper function here
		// Otherwise, use bounding box to determine intersections
		self.intersections_between_subcurves(0. ..1., other, 0. ..1., error).iter().map(|t_values| t_values[0]).collect()
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
	/// Returns a list of `t` values that correspond to the self intersection points of the current bezier curve. For each intersection point, the returned `t` value is the smaller of the two that correspond to the point.
	/// - `error` - For intersections with non-linear beziers, `error` defines the threshold for bounding boxes to be considered an intersection point.
	pub fn self_intersections(&self, error: Option<f64>) -> Vec<[f64; 2]> {
		if self.handles == BezierHandles::Linear || matches!(self.handles, BezierHandles::Quadratic { .. }) {
			return vec![];
		}

		let error = error.unwrap_or(0.5);

		// Get 2 copies of the reduced curves
		let (self1, self1_t_values) = self.reduced_curves_and_t_values(None);
		let (self2, self2_t_values) = (self1.clone(), self1_t_values.clone());
		let num_curves = self1.len();

		// Create iterators that combine a subcurve with the `t` value pair that it was trimmed with
		let combined_iterator1 = self1.into_iter().zip(self1_t_values.windows(2).map(|t_pair| Range { start: t_pair[0], end: t_pair[1] }));
		// Second one needs to be a list because Iterator does not implement copy
		let combined_list2: Vec<(Bezier, Range<f64>)> = self2.into_iter().zip(self2_t_values.windows(2).map(|t_pair| Range { start: t_pair[0], end: t_pair[1] })).collect();

		// Adjacent reduced curves cannot intersect
		// So for each curve, look for intersections with every curve that is at least 2 indices away
		combined_iterator1
			.take(num_curves - 2)
			.enumerate()
			.flat_map(|(index, (subcurve, t_pair))| Bezier::intersections_between_vectors_of_curves(&[(subcurve, t_pair)], &combined_list2[index + 2..], error))
			.collect()
	}

	/// Returns a list of `t` values that correspond to the intersection points between the curve and a rectangle defined by opposite corners.
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
}

#[cfg(test)]
mod tests {
	use super::compare::{compare_f64s, compare_points, compare_vec_of_points};
	use super::*;

	#[test]
	fn test_de_casteljau_points() {
		let bezier = Bezier::from_cubic_coordinates(0., 0., 0., 100., 100., 100., 100., 0.);
		let de_casteljau_points = bezier.de_casteljau_points(0.5);
		let expected_de_casteljau_points = vec![
			vec![DVec2::new(0., 0.), DVec2::new(0., 100.), DVec2::new(100., 100.), DVec2::new(100., 0.)],
			vec![DVec2::new(0., 50.), DVec2::new(50., 100.), DVec2::new(100., 50.)],
			vec![DVec2::new(25., 75.), DVec2::new(75., 75.)],
			vec![DVec2::new(50., 75.)],
		];
		assert_eq!(&de_casteljau_points, &expected_de_casteljau_points);

		assert_eq!(expected_de_casteljau_points[3][0], bezier.evaluate(ComputeType::Parametric(0.5)));
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
		assert_eq!(linear.tangent(0.), unit_slope);
		assert_eq!(linear.tangent(1.), unit_slope);

		let quadratic = Bezier::from_quadratic_dvec2(p1, p2, p3);
		assert_eq!(quadratic.tangent(0.), DVec2::new(60., 40.).normalize());
		assert_eq!(quadratic.tangent(1.), DVec2::new(40., 60.).normalize());

		let cubic = Bezier::from_cubic_dvec2(p1, p2, p3, p4);
		assert_eq!(cubic.tangent(0.), DVec2::new(90., 60.).normalize());
		assert_eq!(cubic.tangent(1.), DVec2::new(30., 120.).normalize());
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
		assert_eq!(linear.normal(0.), unit_slope);
		assert_eq!(linear.normal(1.), unit_slope);

		let quadratic = Bezier::from_quadratic_dvec2(p1, p2, p3);
		assert_eq!(quadratic.normal(0.), DVec2::new(-40., 60.).normalize());
		assert_eq!(quadratic.normal(1.), DVec2::new(-60., 40.).normalize());

		let cubic = Bezier::from_cubic_dvec2(p1, p2, p3, p4);
		assert_eq!(cubic.normal(0.), DVec2::new(-60., 90.).normalize());
		assert_eq!(cubic.normal(1.), DVec2::new(-120., 30.).normalize());
	}

	#[test]
	fn test_curvature() {
		let p1 = DVec2::new(10., 10.);
		let p2 = DVec2::new(50., 10.);
		let p3 = DVec2::new(50., 50.);
		let p4 = DVec2::new(50., 10.);

		let linear = Bezier::from_linear_dvec2(p1, p2);
		assert_eq!(linear.curvature(0.), 0.);
		assert_eq!(linear.curvature(0.5), 0.);
		assert_eq!(linear.curvature(1.), 0.);

		let quadratic = Bezier::from_quadratic_dvec2(p1, p2, p3);
		assert!(compare_f64s(quadratic.curvature(0.), 0.0125));
		assert!(compare_f64s(quadratic.curvature(0.5), 0.035355));
		assert!(compare_f64s(quadratic.curvature(1.), 0.0125));

		let cubic = Bezier::from_cubic_dvec2(p1, p2, p3, p4);
		assert!(compare_f64s(cubic.curvature(0.), 0.016667));
		assert!(compare_f64s(cubic.curvature(0.5), 0.));
		assert!(compare_f64s(cubic.curvature(1.), 0.));

		// The curvature at an inflection point is zero
		let inflection_curve = Bezier::from_cubic_coordinates(30., 30., 30., 150., 150., 30., 150., 150.);
		let inflections = inflection_curve.inflections();
		assert_eq!(inflection_curve.curvature(inflections[0]), 0.);
	}

	#[test]
	fn test_extrema_linear() {
		// Linear bezier cannot have extrema
		let line = Bezier::from_linear_dvec2(DVec2::new(10., 10.), DVec2::new(50., 50.));
		let [x_extrema, y_extrema] = line.local_extrema();
		assert!(x_extrema.is_empty());
		assert!(y_extrema.is_empty());
	}

	#[test]
	fn test_extrema_quadratic() {
		// Test with no x-extrema, no y-extrema
		let bezier1 = Bezier::from_quadratic_coordinates(40., 35., 149., 54., 155., 170.);
		let [x_extrema1, y_extrema1] = bezier1.local_extrema();
		assert!(x_extrema1.is_empty());
		assert!(y_extrema1.is_empty());

		// Test with 1 x-extrema, no y-extrema
		let bezier2 = Bezier::from_quadratic_coordinates(45., 30., 170., 90., 45., 150.);
		let [x_extrema2, y_extrema2] = bezier2.local_extrema();
		assert_eq!(x_extrema2.len(), 1);
		assert!(y_extrema2.is_empty());

		// Test with no x-extrema, 1 y-extrema
		let bezier3 = Bezier::from_quadratic_coordinates(30., 130., 100., 25., 150., 130.);
		let [x_extrema3, y_extrema3] = bezier3.local_extrema();
		assert!(x_extrema3.is_empty());
		assert_eq!(y_extrema3.len(), 1);

		// Test with 1 x-extrema, 1 y-extrema
		let bezier4 = Bezier::from_quadratic_coordinates(50., 70., 170., 35., 60., 150.);
		let [x_extrema4, y_extrema4] = bezier4.local_extrema();
		assert_eq!(x_extrema4.len(), 1);
		assert_eq!(y_extrema4.len(), 1);
	}

	#[test]
	fn test_extrema_cubic() {
		// 0 x-extrema, 0 y-extrema
		let bezier1 = Bezier::from_cubic_coordinates(100., 105., 250., 250., 110., 150., 260., 260.);
		let [x_extrema1, y_extrema1] = bezier1.local_extrema();
		assert!(x_extrema1.is_empty());
		assert!(y_extrema1.is_empty());

		// 1 x-extrema, 0 y-extrema
		let bezier2 = Bezier::from_cubic_coordinates(55., 145., 40., 40., 110., 110., 180., 40.);
		let [x_extrema2, y_extrema2] = bezier2.local_extrema();
		assert_eq!(x_extrema2.len(), 1);
		assert!(y_extrema2.is_empty());

		// 1 x-extrema, 1 y-extrema
		let bezier3 = Bezier::from_cubic_coordinates(100., 105., 170., 10., 25., 20., 20., 120.);
		let [x_extrema3, y_extrema3] = bezier3.local_extrema();
		assert_eq!(x_extrema3.len(), 1);
		assert_eq!(y_extrema3.len(), 1);

		// 1 x-extrema, 2 y-extrema
		let bezier4 = Bezier::from_cubic_coordinates(50., 90., 120., 16., 150., 190., 45., 150.);
		let [x_extrema4, y_extrema4] = bezier4.local_extrema();
		assert_eq!(x_extrema4.len(), 1);
		assert_eq!(y_extrema4.len(), 2);

		// 2 x-extrema, 0 y-extrema
		let bezier5 = Bezier::from_cubic_coordinates(40., 170., 150., 160., 10., 10., 170., 10.);
		let [x_extrema5, y_extrema5] = bezier5.local_extrema();
		assert_eq!(x_extrema5.len(), 2);
		assert!(y_extrema5.is_empty());

		// 2 x-extrema, 1 y-extrema
		let bezier6 = Bezier::from_cubic_coordinates(40., 170., 150., 160., 10., 10., 160., 45.);
		let [x_extrema6, y_extrema6] = bezier6.local_extrema();
		assert_eq!(x_extrema6.len(), 2);
		assert_eq!(y_extrema6.len(), 1);

		// 2 x-extrema, 2 y-extrema
		let bezier7 = Bezier::from_cubic_coordinates(46., 60., 140., 10., 50., 160., 120., 120.);
		let [x_extrema7, y_extrema7] = bezier7.local_extrema();
		assert_eq!(x_extrema7.len(), 2);
		assert_eq!(y_extrema7.len(), 2);
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
		assert!(compare_points(bezier.evaluate(ComputeType::Parametric(intersections1[0])), DVec2::new(30., 60.)));

		// Intersection in the middle of curve
		let line2 = Bezier::from_linear_coordinates(150., 150., 30., 30.);
		let intersections2 = bezier.intersections(&line2, None, None);
		assert!(compare_points(bezier.evaluate(ComputeType::Parametric(intersections2[0])), DVec2::new(96., 96.)));
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
		assert!(compare_points(bezier.evaluate(ComputeType::Parametric(intersections1[0])), p1));

		// Intersection in the middle of curve
		let line2 = Bezier::from_linear_coordinates(150., 150., 30., 30.);
		let intersections2 = bezier.intersections(&line2, None, None);
		assert!(compare_points(bezier.evaluate(ComputeType::Parametric(intersections2[0])), DVec2::new(47.77355, 47.77354)));
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
		assert!(compare_points(bezier.evaluate(ComputeType::Parametric(intersections1[0])), p1));

		// Intersection at edge and in middle of curve, Discriminant < 0
		let line2 = Bezier::from_linear_coordinates(150., 150., 30., 30.);
		let intersections2 = bezier.intersections(&line2, None, None);
		assert!(intersections2.len() == 2);
		assert!(compare_points(bezier.evaluate(ComputeType::Parametric(intersections2[0])), p1));
		assert!(compare_points(bezier.evaluate(ComputeType::Parametric(intersections2[1])), DVec2::new(85.84, 85.84)));
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
		assert!(compare_points(bezier.evaluate(ComputeType::Parametric(intersections[0])), p4));
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

		let intersections1_points: Vec<DVec2> = intersections1.iter().map(|&t| bezier1.evaluate(ComputeType::Parametric(t))).collect();
		let intersections2_points: Vec<DVec2> = intersections2.iter().map(|&t| bezier2.evaluate(ComputeType::Parametric(t))).rev().collect();

		assert!(compare_vec_of_points(intersections1_points, intersections2_points, 2.));
	}

	#[test]
	fn test_intersect_with_self() {
		let bezier = Bezier::from_cubic_coordinates(160., 180., 170., 10., 30., 90., 180., 140.);
		let intersections = bezier.self_intersections(Some(0.5));
		assert!(compare_vec_of_points(
			intersections.iter().map(|&t| bezier.evaluate(ComputeType::Parametric(t[0]))).collect(),
			intersections.iter().map(|&t| bezier.evaluate(ComputeType::Parametric(t[1]))).collect(),
			2.
		));
		assert!(Bezier::from_linear_coordinates(160., 180., 170., 10.).self_intersections(None).is_empty());
		assert!(Bezier::from_quadratic_coordinates(160., 180., 170., 10., 30., 90.).self_intersections(None).is_empty());
	}
}

use super::*;
use crate::consts::{DEFAULT_EUCLIDEAN_ERROR_BOUND, DEFAULT_LUT_STEP_SIZE, MAX_ABSOLUTE_DIFFERENCE};
use crate::utils::{SubpathTValue, TValue, TValueType};
use glam::DVec2;

/// Functionality relating to looking up properties of the `Subpath` or points along the `Subpath`.
impl<PointId: crate::Identifier> Subpath<PointId> {
	/// Return a selection of equidistant points on the bezier curve.
	/// If no value is provided for `steps`, then the function will default `steps` to be 10.
	/// <iframe frameBorder="0" width="100%" height="350px" src="https://graphite.rs/libraries/bezier-rs#subpath/lookup-table/solo" title="Lookup-Table Demo"></iframe>
	pub fn compute_lookup_table(&self, steps: Option<usize>, tvalue_type: Option<TValueType>) -> Vec<DVec2> {
		let steps = steps.unwrap_or(DEFAULT_LUT_STEP_SIZE);
		let tvalue_type = tvalue_type.unwrap_or(TValueType::Parametric);

		(0..=steps)
			.map(|t| {
				let tvalue = match tvalue_type {
					TValueType::Parametric => SubpathTValue::GlobalParametric(t as f64 / steps as f64),
					TValueType::Euclidean => SubpathTValue::GlobalEuclidean(t as f64 / steps as f64),
				};
				self.evaluate(tvalue)
			})
			.collect()
	}

	/// Return the sum of the approximation of the length of each `Bezier` curve along the `Subpath`.
	/// - `tolerance` - Tolerance used to approximate the curve.
	/// <iframe frameBorder="0" width="100%" height="300px" src="https://graphite.rs/libraries/bezier-rs#subpath/length/solo" title="Length Demo"></iframe>
	pub fn length(&self, tolerance: Option<f64>) -> f64 {
		self.iter().map(|bezier| bezier.length(tolerance)).sum()
	}

	/// Return the approximation of the length centroid, together with the length, of the `Subpath`.
	///
	/// The length centroid is the center of mass for the arc length of the solid shape's perimeter.
	/// An infinitely thin wire forming the subpath's closed shape would balance at this point.
	///
	/// It will return `None` if no manipulator is present.
	/// - `tolerance` - Tolerance used to approximate the curve.
	/// - `always_closed` - consider the subpath as closed always.
	pub fn length_centroid_and_length(&self, tolerance: Option<f64>, always_closed: bool) -> Option<(DVec2, f64)> {
		if always_closed { self.iter_closed() } else { self.iter() }
			.map(|bezier| bezier.length_centroid_and_length(tolerance))
			.map(|(centroid, length)| (centroid * length, length))
			.reduce(|(centroid_part1, length1), (centroid_part2, length2)| (centroid_part1 + centroid_part2, length1 + length2))
			.map(|(centroid_part, length)| (centroid_part / length, length))
	}

	/// Return the approximation of the length centroid of the `Subpath`.
	///
	/// The length centroid is the center of mass for the arc length of the solid shape's perimeter.
	/// An infinitely thin wire forming the subpath's closed shape would balance at this point.
	///
	/// It will return `None` if no manipulator is present.
	/// - `tolerance` - Tolerance used to approximate the curve.
	/// - `always_closed` - consider the subpath as closed always.
	/// <iframe frameBorder="0" width="100%" height="300px" src="https://graphite.rs/libraries/bezier-rs#subpath/length-centroid/solo" title="Length Centroid Demo"></iframe>
	pub fn length_centroid(&self, tolerance: Option<f64>, always_closed: bool) -> Option<DVec2> {
		self.length_centroid_and_length(tolerance, always_closed).map(|(centroid, _)| centroid)
	}

	/// Return the area enclosed by the `Subpath` always considering it as a closed subpath. It will always give a positive value.
	///
	/// If the area is less than `error`, it will return zero.
	/// Because the calculation of area for self-intersecting path requires finding the intersections, the following parameters are used:
	/// - `error` - For intersections with non-linear beziers, `error` defines the threshold for bounding boxes to be considered an intersection point.
	/// - `minimum_separation` - the minimum difference two adjacent `t`-values must have when comparing adjacent `t`-values in sorted order.
	///
	/// If the comparison condition is not satisfied, the function takes the larger `t`-value of the two
	///
	/// **NOTE**: if an intersection were to occur within an `error` distance away from an anchor point, the algorithm will filter that intersection out.
	pub fn area(&self, error: Option<f64>, minimum_separation: Option<f64>) -> f64 {
		let all_intersections = self.all_self_intersections(error, minimum_separation);
		let mut current_sign: f64 = 1.;

		let area: f64 = self
			.iter_closed()
			.enumerate()
			.map(|(index, bezier)| {
				let (f_x, f_y) = bezier.parametric_polynomial();
				let (f_x, mut f_y) = (f_x.as_size::<7>().unwrap(), f_y.as_size::<7>().unwrap());
				f_y.derivative_mut();
				f_y *= &f_x;
				f_y.antiderivative_mut();

				let mut curve_sum = -current_sign * f_y.eval(0.);
				for (_, t) in all_intersections.iter().filter(|(i, _)| *i == index) {
					curve_sum += 2. * current_sign * f_y.eval(*t);
					current_sign *= -1.;
				}
				curve_sum += current_sign * f_y.eval(1.);
				curve_sum
			})
			.sum();

		if area.abs() < error.unwrap_or(MAX_ABSOLUTE_DIFFERENCE) {
			return 0.;
		}

		area.abs()
	}

	/// Return the area centroid, together with the area, of the `Subpath` always considering it as a closed subpath. The area will always be a positive value.
	///
	/// The area centroid is the center of mass for the area of a solid shape's interior.
	/// An infinitely flat material forming the subpath's closed shape would balance at this point.
	///
	/// It will return `None` if no manipulator is present. If the area is less than `error`, it will return `Some((DVec2::NAN, 0.))`.
	///
	/// Because the calculation of area and centroid for self-intersecting path requires finding the intersections, the following parameters are used:
	/// - `error` - For intersections with non-linear beziers, `error` defines the threshold for bounding boxes to be considered an intersection point.
	/// - `minimum_separation` - the minimum difference two adjacent `t`-values must have when comparing adjacent `t`-values in sorted order.
	///
	/// If the comparison condition is not satisfied, the function takes the larger `t`-value of the two.
	///
	/// **NOTE**: if an intersection were to occur within an `error` distance away from an anchor point, the algorithm will filter that intersection out.
	pub fn area_centroid_and_area(&self, error: Option<f64>, minimum_separation: Option<f64>) -> Option<(DVec2, f64)> {
		let all_intersections = self.all_self_intersections(error, minimum_separation);
		let mut current_sign: f64 = 1.;

		let (x_sum, y_sum, area) = self
			.iter_closed()
			.enumerate()
			.map(|(index, bezier)| {
				let (f_x, f_y) = bezier.parametric_polynomial();
				let (f_x, f_y) = (f_x.as_size::<10>().unwrap(), f_y.as_size::<10>().unwrap());
				let f_y_prime = f_y.derivative();
				let f_x_prime = f_x.derivative();
				let f_xy = &f_x * &f_y;

				let mut x_part = &f_xy * &f_x_prime;
				let mut y_part = &f_xy * &f_y_prime;
				let mut area_part = &f_x * &f_y_prime;
				x_part.antiderivative_mut();
				y_part.antiderivative_mut();
				area_part.antiderivative_mut();

				let mut curve_sum_x = -current_sign * x_part.eval(0.);
				let mut curve_sum_y = -current_sign * y_part.eval(0.);
				let mut curve_sum_area = -current_sign * area_part.eval(0.);
				for (_, t) in all_intersections.iter().filter(|(i, _)| *i == index) {
					curve_sum_x += 2. * current_sign * x_part.eval(*t);
					curve_sum_y += 2. * current_sign * y_part.eval(*t);
					curve_sum_area += 2. * current_sign * area_part.eval(*t);
					current_sign *= -1.;
				}
				curve_sum_x += current_sign * x_part.eval(1.);
				curve_sum_y += current_sign * y_part.eval(1.);
				curve_sum_area += current_sign * area_part.eval(1.);

				(-curve_sum_x, curve_sum_y, curve_sum_area)
			})
			.reduce(|(x1, y1, area1), (x2, y2, area2)| (x1 + x2, y1 + y2, area1 + area2))?;

		if area.abs() < error.unwrap_or(MAX_ABSOLUTE_DIFFERENCE) {
			return Some((DVec2::NAN, 0.));
		}

		Some((DVec2::new(x_sum / area, y_sum / area), area.abs()))
	}

	/// Attempts to return the area centroid of the `Subpath` always considering it as a closed subpath. Falls back to length centroid if the area is zero.
	///
	/// The area centroid is the center of mass for the area of a solid shape's interior.
	/// An infinitely flat material forming the subpath's closed shape would balance at this point.
	///
	/// It will return `None` if no manipulator is present.
	/// Because the calculation of centroid for self-intersecting path requires finding the intersections, the following parameters are used:
	/// - `error` - For intersections with non-linear beziers, `error` defines the threshold for bounding boxes to be considered an intersection point.
	/// - `minimum_separation` - the minimum difference two adjacent `t`-values must have when comparing adjacent `t`-values in sorted order.
	/// - `tolerance` - Tolerance used to approximate the curve if it falls back to length centroid.
	///
	/// If the comparison condition is not satisfied, the function takes the larger `t`-value of the two
	///
	/// **NOTE**: if an intersection were to occur within an `error` distance away from an anchor point, the algorithm will filter that intersection out.
	/// <iframe frameBorder="0" width="100%" height="300px" src="https://graphite.rs/libraries/bezier-rs#subpath/area-centroid/solo" title="Area Centroid Demo"></iframe>
	pub fn area_centroid(&self, error: Option<f64>, minimum_separation: Option<f64>, tolerance: Option<f64>) -> Option<DVec2> {
		let (centroid, area) = self.area_centroid_and_area(error, minimum_separation)?;

		if area != 0. {
			Some(centroid)
		} else {
			self.length_centroid_and_length(tolerance, true).map(|(centroid, _)| centroid)
		}
	}

	/// Converts from a subpath (composed of multiple segments) to a point along a certain segment represented.
	/// The returned tuple represents the segment index and the `t` value along that segment.
	/// Both the input global `t` value and the output `t` value are in euclidean space, meaning there is a constant rate of change along the arc length.
	pub fn global_euclidean_to_local_euclidean(&self, global_t: f64, lengths: &[f64], total_length: f64) -> (usize, f64) {
		let mut accumulator = 0.;
		for (index, length) in lengths.iter().enumerate() {
			let length_ratio = length / total_length;
			if (index == 0 || accumulator <= global_t) && global_t <= accumulator + length_ratio {
				return (index, ((global_t - accumulator) / length_ratio).clamp(0., 1.));
			}
			accumulator += length_ratio;
		}
		(self.len() - 2, 1.)
	}

	/// Convert a [SubpathTValue] to a parametric `(segment_index, t)` tuple.
	/// - Asserts that `t` values contained within the `SubpathTValue` argument lie in the range [0, 1].
	/// - If the argument is a variant containing a `segment_index`, asserts that the index references a valid segment on the curve.
	pub(crate) fn t_value_to_parametric(&self, t: SubpathTValue) -> (usize, f64) {
		assert!(self.len_segments() >= 1);

		match t {
			SubpathTValue::Parametric { segment_index, t } => {
				assert!((0.0..=1.).contains(&t));
				assert!((0..self.len_segments()).contains(&segment_index));
				(segment_index, t)
			}
			SubpathTValue::GlobalParametric(global_t) => {
				assert!((0.0..=1.).contains(&global_t));

				if global_t == 1. {
					return (self.len_segments() - 1, 1.);
				}

				let scaled_t = global_t * self.len_segments() as f64;
				let segment_index = scaled_t.floor() as usize;
				let t = scaled_t - segment_index as f64;

				(segment_index, t)
			}
			SubpathTValue::Euclidean { segment_index, t } => {
				assert!((0.0..=1.).contains(&t));
				assert!((0..self.len_segments()).contains(&segment_index));
				(segment_index, self.get_segment(segment_index).unwrap().euclidean_to_parametric(t, DEFAULT_EUCLIDEAN_ERROR_BOUND))
			}
			SubpathTValue::GlobalEuclidean(t) => {
				let lengths = self.iter().map(|bezier| bezier.length(None)).collect::<Vec<f64>>();
				let total_length: f64 = lengths.iter().sum();
				let (segment_index, segment_t_euclidean) = self.global_euclidean_to_local_euclidean(t, lengths.as_slice(), total_length);
				let segment_t_parametric = self.get_segment(segment_index).unwrap().euclidean_to_parametric(segment_t_euclidean, DEFAULT_EUCLIDEAN_ERROR_BOUND);
				(segment_index, segment_t_parametric)
			}
			SubpathTValue::EuclideanWithinError { segment_index, t, error } => {
				assert!((0.0..=1.).contains(&t));
				assert!((0..self.len_segments()).contains(&segment_index));
				(segment_index, self.get_segment(segment_index).unwrap().euclidean_to_parametric(t, error))
			}
			SubpathTValue::GlobalEuclideanWithinError { t, error } => {
				let lengths = self.iter().map(|bezier| bezier.length(None)).collect::<Vec<f64>>();
				let total_length: f64 = lengths.iter().sum();
				let (segment_index, segment_t) = self.global_euclidean_to_local_euclidean(t, lengths.as_slice(), total_length);
				(segment_index, self.get_segment(segment_index).unwrap().euclidean_to_parametric(segment_t, error))
			}
		}
	}

	/// Returns the segment index and `t` value that corresponds to the closest point on the curve to the provided point.
	/// <iframe frameBorder="0" width="100%" height="300px" src="https://graphite.rs/libraries/bezier-rs#subpath/project/solo" title="Project Demo"></iframe>
	pub fn project(&self, point: DVec2) -> Option<(usize, f64)> {
		if self.is_empty() {
			return None;
		}

		// TODO: Optimization opportunity: Filter out segments which are *definitely* not the closest to the given point
		let (index, (_, project_t)) = self
			.iter()
			.map(|bezier| {
				let project_t = bezier.project(point);
				(bezier.evaluate(TValue::Parametric(project_t)).distance(point), project_t)
			})
			.enumerate()
			.min_by(|(_, (distance1, _)), (_, (distance2, _))| distance1.total_cmp(distance2))
			.unwrap_or((0, (0., 0.))); // If the Subpath contains only a single manipulator group, returns (0, 0.)

		Some((index, project_t))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::consts::MAX_ABSOLUTE_DIFFERENCE;
	use crate::utils::f64_compare;

	#[test]
	fn length_quadratic() {
		let start = DVec2::new(20., 30.);
		let middle = DVec2::new(80., 90.);
		let end = DVec2::new(60., 45.);
		let handle1 = DVec2::new(75., 85.);
		let handle2 = DVec2::new(40., 30.);
		let handle3 = DVec2::new(10., 10.);

		let bezier1 = Bezier::from_quadratic_dvec2(start, handle1, middle);
		let bezier2 = Bezier::from_quadratic_dvec2(middle, handle2, end);
		let bezier3 = Bezier::from_quadratic_dvec2(end, handle3, start);

		let mut subpath = Subpath::new(
			vec![
				ManipulatorGroup {
					anchor: start,
					in_handle: None,
					out_handle: Some(handle1),
					id: EmptyId,
				},
				ManipulatorGroup {
					anchor: middle,
					in_handle: None,
					out_handle: Some(handle2),
					id: EmptyId,
				},
				ManipulatorGroup {
					anchor: end,
					in_handle: None,
					out_handle: Some(handle3),
					id: EmptyId,
				},
			],
			false,
		);
		assert_eq!(subpath.length(None), bezier1.length(None) + bezier2.length(None));

		subpath.closed = true;
		assert_eq!(subpath.length(None), bezier1.length(None) + bezier2.length(None) + bezier3.length(None));
	}

	#[test]
	fn length_mixed() {
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
		assert_eq!(subpath.length(None), linear_bezier.length(None) + quadratic_bezier.length(None));

		subpath.closed = true;
		assert_eq!(subpath.length(None), linear_bezier.length(None) + quadratic_bezier.length(None) + cubic_bezier.length(None));
	}

	#[test]
	fn length_centroid() {
		let start = DVec2::new(0., 0.);
		let end = DVec2::new(1., 1.);
		let handle = DVec2::new(0., 1.);

		let mut subpath = Subpath::new(
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
					out_handle: None,
					id: EmptyId,
				},
			],
			false,
		);

		let expected_centroid = DVec2::new(0.4153039799983826, 0.5846960200016174);
		let epsilon = 0.00001;

		assert!(subpath.length_centroid_and_length(None, true).unwrap().0.abs_diff_eq(expected_centroid, epsilon));

		subpath.closed = true;
		assert!(subpath.length_centroid_and_length(None, true).unwrap().0.abs_diff_eq(expected_centroid, epsilon));
	}

	#[test]
	fn area() {
		let start = DVec2::new(0., 0.);
		let end = DVec2::new(1., 1.);
		let handle = DVec2::new(0., 1.);

		let mut subpath = Subpath::new(
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
					out_handle: None,
					id: EmptyId,
				},
			],
			false,
		);

		let expected_area = 1. / 3.;
		let epsilon = 0.00001;

		assert!((subpath.area(Some(0.001), Some(0.001)) - expected_area).abs() < epsilon);

		subpath.closed = true;
		assert!((subpath.area(Some(0.001), Some(0.001)) - expected_area).abs() < epsilon);
	}

	#[test]
	fn area_centroid() {
		let start = DVec2::new(0., 0.);
		let end = DVec2::new(1., 1.);
		let handle = DVec2::new(0., 1.);

		let mut subpath = Subpath::new(
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
					out_handle: None,
					id: EmptyId,
				},
			],
			false,
		);

		let expected_centroid = DVec2::new(0.4, 0.6);
		let epsilon = 0.00001;

		assert!(subpath.area_centroid(Some(0.001), Some(0.001), None).unwrap().abs_diff_eq(expected_centroid, epsilon));

		subpath.closed = true;
		assert!(subpath.area_centroid(Some(0.001), Some(0.001), None).unwrap().abs_diff_eq(expected_centroid, epsilon));
	}

	#[test]
	fn t_value_to_parametric_global_parametric_open_subpath() {
		let mock_manipulator_group = ManipulatorGroup {
			anchor: DVec2::new(0., 0.),
			in_handle: None,
			out_handle: None,
			id: EmptyId,
		};
		let open_subpath = Subpath {
			manipulator_groups: vec![mock_manipulator_group; 5],
			closed: false,
		};

		let (segment_index, t) = open_subpath.t_value_to_parametric(SubpathTValue::GlobalParametric(0.7));
		assert_eq!(segment_index, 2);
		assert!(f64_compare(t, 0.8, MAX_ABSOLUTE_DIFFERENCE));

		// The start and end points of an open subpath are NOT equivalent
		assert_eq!(open_subpath.t_value_to_parametric(SubpathTValue::GlobalParametric(0.)), (0, 0.));
		assert_eq!(open_subpath.t_value_to_parametric(SubpathTValue::GlobalParametric(1.)), (3, 1.));
	}

	#[test]
	fn t_value_to_parametric_global_parametric_closed_subpath() {
		let mock_manipulator_group = ManipulatorGroup {
			anchor: DVec2::new(0., 0.),
			in_handle: None,
			out_handle: None,
			id: EmptyId,
		};
		let closed_subpath = Subpath {
			manipulator_groups: vec![mock_manipulator_group; 5],
			closed: true,
		};

		let (segment_index, t) = closed_subpath.t_value_to_parametric(SubpathTValue::GlobalParametric(0.7));
		assert_eq!(segment_index, 3);
		assert!(f64_compare(t, 0.5, MAX_ABSOLUTE_DIFFERENCE));

		// The start and end points of a closed subpath are equivalent
		assert_eq!(closed_subpath.t_value_to_parametric(SubpathTValue::GlobalParametric(0.)), (0, 0.));
		assert_eq!(closed_subpath.t_value_to_parametric(SubpathTValue::GlobalParametric(1.)), (4, 1.));
	}

	#[test]
	fn exact_start_end() {
		let start = DVec2::new(20., 30.);
		let end = DVec2::new(60., 45.);
		let handle = DVec2::new(75., 85.);

		let subpath: Subpath<EmptyId> = Subpath::from_bezier(&Bezier::from_quadratic_dvec2(start, handle, end));

		assert_eq!(subpath.evaluate(SubpathTValue::GlobalEuclidean(0.)), start);
		assert_eq!(subpath.evaluate(SubpathTValue::GlobalEuclidean(1.)), end);
	}
}

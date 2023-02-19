use super::*;
use crate::consts::MAX_ABSOLUTE_DIFFERENCE;
use crate::utils::SubpathTValue;
use crate::TValue;

use glam::DVec2;

impl<ManipulatorGroupId: crate::Identifier> Subpath<ManipulatorGroupId> {
	/// Calculate the point on the subpath based on the parametric `t`-value provided.
	/// Expects `t` to be within the inclusive range `[0, 1]`.
	/// <iframe frameBorder="0" width="100%" height="400px" src="https://graphite.rs/bezier-rs-demos#subpath/evaluate/solo" title="Evaluate Demo"></iframe>
	pub fn evaluate(&self, t: SubpathTValue) -> DVec2 {
		let (segment_index, t) = self.t_value_to_parametric(t);
		self.get_segment(segment_index).unwrap().evaluate(TValue::Parametric(t))
	}

	/// Calculates the intersection points the subpath has with a given curve and returns a list of `(usize, f64)` tuples,
	/// where the `usize` represents the index of the curve in the subpath, and the `f64` represents the `t`-value local to
	/// that curve where the intersection occured.
	/// Expects the following:
	/// - `other`: a [Bezier] curve to check intersections against
	/// - `error`: an optional f64 value to provide an error bound
	/// - `minimum_seperation`: the minimum difference two adjacent `t`-values must have when comparing adjacent `t`-values in sorted order.
	/// If the comparison condition is not satisfied, the function takes the larger `t`-value of the two.
	/// <iframe frameBorder="0" width="100%" height="325px" src="https://graphite.rs/bezier-rs-demos#subpath/intersect-cubic/solo" title="Intersection Demo"></iframe>
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
	/// <iframe frameBorder="0" width="100%" height="325px" src="https://graphite.rs/bezier-rs-demos#subpath/intersect-cubic/solo" title="Intersection Demo"></iframe>
	pub fn subpath_intersections(&self, other: &Subpath<ManipulatorGroupId>, error: Option<f64>, minimum_separation: Option<f64>) -> Vec<(usize, f64)> {
		let mut intersection_t_values: Vec<(usize, f64)> = other.iter().flat_map(|bezier| self.intersections(&bezier, error, minimum_separation)).collect();
		intersection_t_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
		intersection_t_values
	}

	/// Returns a normalized unit vector representing the tangent on the subpath based on the parametric `t`-value provided.
	/// <iframe frameBorder="0" width="100%" height="400px" src="https://graphite.rs/bezier-rs-demos#subpath/tangent/solo" title="Tangent Demo"></iframe>
	pub fn tangent(&self, t: SubpathTValue) -> DVec2 {
		let (segment_index, t) = self.t_value_to_parametric(t);
		self.get_segment(segment_index).unwrap().tangent(TValue::Parametric(t))
	}

	/// Returns a list of `t` values that correspond to the self intersection points of the subpath. For each intersection point, the returned `t` value is the smaller of the two that correspond to the point.
	/// - `error` - For intersections with non-linear beziers, `error` defines the threshold for bounding boxes to be considered an intersection point.
	/// - `minimum_seperation`: the minimum difference two adjacent `t`-values must have when comparing adjacent `t`-values in sorted order.
	/// If the comparison condition is not satisfied, the function takes the larger `t`-value of the two
	///
	/// **NOTE**: if an intersection were to occur within an `error` distance away from an anchor point, the algorithm will filter that intersection out.
	/// <iframe frameBorder="0" width="100%" height="325px" src="https://graphite.rs/bezier-rs-demos#subpath/self-intersect/solo" title="Self-Intersection Demo"></iframe>
	pub fn self_intersections(&self, error: Option<f64>, minimum_seperation: Option<f64>) -> Vec<(usize, f64)> {
		let mut intersections_vec = Vec::new();
		let err = error.unwrap_or(MAX_ABSOLUTE_DIFFERENCE);
		// TODO: optimization opportunity - this for-loop currently compares all intersections with all curve-segments in the subpath collection
		self.iter().enumerate().for_each(|(i, other)| {
			intersections_vec.extend(other.self_intersections(error).iter().map(|value| (i, value[0])));
			self.iter().enumerate().skip(i + 1).for_each(|(j, curve)| {
				intersections_vec.extend(
					curve
						.intersections(&other, error, minimum_seperation)
						.iter()
						.filter(|&value| value > &err && (1. - value) > err)
						.map(|value| (j, *value)),
				);
			});
		});
		intersections_vec
	}

	/// Returns a normalized unit vector representing the direction of the normal on the subpath based on the parametric `t`-value provided.
	/// <iframe frameBorder="0" width="100%" height="400px" src="https://graphite.rs/bezier-rs-demos#subpath/normal/solo" title="Normal Demo"></iframe>
	pub fn normal(&self, t: SubpathTValue) -> DVec2 {
		let (segment_index, t) = self.t_value_to_parametric(t);
		self.get_segment(segment_index).unwrap().normal(TValue::Parametric(t))
	}

	/// Returns two lists of `t`-values representing the local extrema of the `x` and `y` parametric subpaths respectively.
	/// The list of `t`-values returned are filtered such that they fall within the range `[0, 1]`.
	/// <iframe frameBorder="0" width="100%" height="400px" src="https://graphite.rs/bezier-rs-demos#subpath/local-extrema/solo" title="Local Extrema Demo"></iframe>
	pub fn local_extrema(&self) -> [Vec<f64>; 2] {
		let number_of_curves = self.len_segments() as f64;

		// TODO: Consider the shared point between adjacent beziers.
		self.iter().enumerate().fold([Vec::new(), Vec::new()], |mut acc, elem| {
			let extremas = elem.1.local_extrema();
			// Convert t-values of bezier curve to t-values of subpath
			acc[0].extend(extremas[0].iter().map(|t| ((elem.0 as f64) + t) / number_of_curves).collect::<Vec<f64>>());
			acc[1].extend(extremas[1].iter().map(|t| ((elem.0 as f64) + t) / number_of_curves).collect::<Vec<f64>>());
			acc
		})
	}

	/// Return the min and max corners that represent the bounding box of the subpath.
	/// <iframe frameBorder="0" width="100%" height="400px" src="https://graphite.rs/bezier-rs-demos#subpath/bounding-box/solo" title="Bounding Box Demo"></iframe>
	pub fn bounding_box(&self) -> Option<[DVec2; 2]> {
		self.iter().map(|bezier| bezier.bounding_box()).reduce(|bbox1, bbox2| [bbox1[0].min(bbox2[0]), bbox1[1].max(bbox2[1])])
	}

	/// Returns list of `t`-values representing the inflection points of the subpath.
	/// The list of `t`-values returned are filtered such that they fall within the range `[0, 1]`.
	/// <iframe frameBorder="0" width="100%" height="400px" src="https://graphite.rs/bezier-rs-demos#subpath/inflections/solo" title="Inflections Demo"></iframe>
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
}

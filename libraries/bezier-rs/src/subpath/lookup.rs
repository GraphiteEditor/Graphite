use super::*;
use crate::consts::{DEFAULT_EUCLIDEAN_ERROR_BOUND, DEFAULT_LUT_STEP_SIZE};
use crate::utils::{SubpathTValue, TValue, TValueType};
use crate::ProjectionOptions;
use glam::DVec2;

/// Functionality relating to looking up properties of the `Subpath` or points along the `Subpath`.
impl<ManipulatorGroupId: crate::Identifier> Subpath<ManipulatorGroupId> {
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
	/// - `num_subdivisions` - Number of subdivisions used to approximate the curve. The default value is `1000`.
	/// <iframe frameBorder="0" width="100%" height="300px" src="https://graphite.rs/libraries/bezier-rs#subpath/length/solo" title="Length Demo"></iframe>
	pub fn length(&self, num_subdivisions: Option<usize>) -> f64 {
		self.iter().map(|bezier| bezier.length(num_subdivisions)).sum()
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
	/// Uses a searching algorithm akin to binary search that can be customized using the [ProjectionOptions] structure.
	/// <iframe frameBorder="0" width="100%" height="300px" src="https://graphite.rs/libraries/bezier-rs#subpath/project/solo" title="Project Demo"></iframe>
	pub fn project(&self, point: DVec2, options: Option<ProjectionOptions>) -> Option<(usize, f64)> {
		if self.is_empty() {
			return None;
		}

		// TODO: Optimization opportunity: Filter out segments which are *definitely* not the closest to the given point
		let (index, (_, project_t)) = self
			.iter()
			.map(|bezier| {
				let project_t = bezier.project(point, options);
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
	use crate::consts::MAX_ABSOLUTE_DIFFERENCE;
	use crate::utils::f64_compare;

	use super::*;

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

		assert_eq!(subpath.evaluate(SubpathTValue::GlobalEuclidean(0.0)), start);
		assert_eq!(subpath.evaluate(SubpathTValue::GlobalEuclidean(1.0)), end);
	}
}

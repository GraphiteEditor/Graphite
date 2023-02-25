use super::*;
use crate::consts::DEFAULT_EUCLIDEAN_ERROR_BOUND;
use crate::utils::{SubpathTValue, TValue};
use crate::ProjectionOptions;
use glam::DVec2;

/// Functionality relating to looking up properties of the `Subpath` or points along the `Subpath`.
impl<ManipulatorGroupId: crate::Identifier> Subpath<ManipulatorGroupId> {
	/// Return the sum of the approximation of the length of each `Bezier` curve along the `Subpath`.
	/// - `num_subdivisions` - Number of subdivisions used to approximate the curve. The default value is `1000`.
	/// <iframe frameBorder="0" width="100%" height="325px" src="https://graphite.rs/bezier-rs-demos#subpath/length/solo" title="Length Demo"></iframe>
	pub fn length(&self, num_subdivisions: Option<usize>) -> f64 {
		self.iter().fold(0., |accumulator, bezier| accumulator + bezier.length(num_subdivisions))
	}

	fn global_euclidean_to_local_euclidean(&self, global_t: f64) -> (usize, f64) {
		let lengths = self.iter().map(|bezier| bezier.length(None)).collect::<Vec<f64>>();
		let total_length: f64 = lengths.iter().sum();

		let mut accumulator = 0.;
		for (index, length) in lengths.iter().enumerate() {
			let length_ratio = length / total_length;
			if accumulator <= global_t && global_t <= accumulator + length_ratio {
				return (index, (global_t - accumulator) / length_ratio);
			}
			accumulator += length_ratio;
		}
		(0, 0.)
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
				let (segment_index, segment_t) = self.global_euclidean_to_local_euclidean(t);
				(
					segment_index,
					self.get_segment(segment_index).unwrap().euclidean_to_parametric(segment_t, DEFAULT_EUCLIDEAN_ERROR_BOUND),
				)
			}
			SubpathTValue::EuclideanWithinError { segment_index, t, error } => {
				assert!((0.0..=1.).contains(&t));
				assert!((0..self.len_segments()).contains(&segment_index));
				(segment_index, self.get_segment(segment_index).unwrap().euclidean_to_parametric(t, error))
			}
			SubpathTValue::GlobalEuclideanWithinError { t, error } => {
				let (segment_index, segment_t) = self.global_euclidean_to_local_euclidean(t);
				(segment_index, self.get_segment(segment_index).unwrap().euclidean_to_parametric(segment_t, error))
			}
		}
	}

	/// Returns the segment index and `t` value that corresponds to the closest point on the curve to the provided point.
	/// Uses a searching algorithm akin to binary search that can be customized using the [ProjectionOptions] structure.
	/// <iframe frameBorder="0" width="100%" height="325px" src="https://graphite.rs/bezier-rs-demos#subpath/project/solo" title="Project Demo"></iframe>
	pub fn project(&self, point: DVec2, options: ProjectionOptions) -> Option<(usize, f64)> {
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
}

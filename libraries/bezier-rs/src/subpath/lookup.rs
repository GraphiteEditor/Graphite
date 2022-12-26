use super::*;
use crate::{ComputeType, ProjectionOptions};
use glam::DVec2;

/// Functionality relating to looking up properties of the `Subpath` or points along the `Subpath`.
impl Subpath {
	/// Return the sum of the approximation of the length of each `Bezier` curve along the `Subpath`.
	/// - `num_subdivisions` - Number of subdivisions used to approximate the curve. The default value is `1000`.
	pub fn length(&self, num_subdivisions: Option<usize>) -> f64 {
		self.iter().fold(0., |accumulator, bezier| accumulator + bezier.length(num_subdivisions))
	}

	/// Returns the segment index and `t` value that corresponds to the closest point on the curve to the provided point.
	/// Uses a searching algorithm akin to binary search that can be customized using the [ProjectionOptions] structure.
	pub fn project(&self, point: DVec2, options: ProjectionOptions) -> (usize, f64) {
		if self.is_empty() {
			panic!("Can not project to an empty Subpath")
		}

		// TODO: Optimization opportunity: Filter out segments which are *definitely* not the closest to the given point
		let (index, (_, project_t)) = self
			.iter()
			.map(|bezier| {
				let project_t = bezier.project(point, options);
				(bezier.evaluate(ComputeType::Parametric(project_t)).distance(point), project_t)
			})
			.enumerate()
			.fold(
				(0, (f64::INFINITY, 0.)), // If the Subpath contains only a single manipulator group, returns (0, 0.)
				|(i1, (d1, t1)), (i2, (d2, t2))| if d1 < d2 { (i1, (d1, t1)) } else { (i2, (d2, t2)) },
			);

		(index, project_t)
	}
}

#[cfg(test)]
mod tests {
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
				},
				ManipulatorGroup {
					anchor: middle,
					in_handle: None,
					out_handle: Some(handle2),
				},
				ManipulatorGroup {
					anchor: end,
					in_handle: None,
					out_handle: Some(handle3),
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
				},
				ManipulatorGroup {
					anchor: middle,
					in_handle: None,
					out_handle: Some(handle1),
				},
				ManipulatorGroup {
					anchor: end,
					in_handle: None,
					out_handle: Some(handle2),
				},
			],
			false,
		);
		assert_eq!(subpath.length(None), linear_bezier.length(None) + quadratic_bezier.length(None));

		subpath.closed = true;
		assert_eq!(subpath.length(None), linear_bezier.length(None) + quadratic_bezier.length(None) + cubic_bezier.length(None));
	}
}

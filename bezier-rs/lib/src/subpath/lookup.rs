use super::*;

impl SubPath {
	/// Return the sum of the approximation of the length of each bezier curve along the subpath.
	/// - `num_subdivisions` - Number of subdivisions used to approximate the curve. The default value is `1000`.
	pub fn length(&self, num_subdivisions: Option<i32>) -> f64 {
		self.iter().map(|bezier| bezier.length(num_subdivisions)).sum()
	}
}

#[cfg(test)]
mod tests {

	use crate::Bezier;

	use super::*;

	use glam::DVec2;

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

		let mut subpath = SubPath::new(
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

		let bezier1 = Bezier::from_linear_dvec2(start, middle);
		let bezier2 = Bezier::from_quadratic_dvec2(middle, handle1, end);
		let bezier3 = Bezier::from_cubic_dvec2(end, handle2, handle3, start);

		let mut subpath = SubPath::new(
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
		assert_eq!(subpath.length(None), bezier1.length(None) + bezier2.length(None));

		subpath.closed = true;
		assert_eq!(subpath.length(None), bezier1.length(None) + bezier2.length(None) + bezier3.length(None));
	}
}

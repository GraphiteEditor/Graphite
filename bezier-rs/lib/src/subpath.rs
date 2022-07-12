use glam::DVec2;

use crate::Bezier;

struct ManipulatorGroup {
	pub anchor: DVec2,
	pub in_handle: Option<DVec2>,
	pub out_handle: Option<DVec2>,
}

// TODO: Enforce that a Subpath cannot be closed if it has 0 or 1 points
struct SubPath {
	manipulator_groups: Vec<ManipulatorGroup>,
	closed: bool,
}

struct SubPathIter<'a> {
	index: usize,
	sub_path: &'a SubPath,
}

impl Iterator for SubPathIter<'_> {
	type Item = Bezier;

	fn next(&mut self) -> Option<Self::Item> {
		if self.index >= self.sub_path.manipulator_groups.len() - 1 + (self.sub_path.closed as usize) {
			return None;
		}
		let start_index = self.index;
		let end_index = (self.index + 1) % self.sub_path.manipulator_groups.len();
		self.index += 1;

		let start = self.sub_path.manipulator_groups[start_index].anchor;
		let end = self.sub_path.manipulator_groups[end_index].anchor;
		let handle1 = self.sub_path.manipulator_groups[start_index].out_handle;
		let handle2 = self.sub_path.manipulator_groups[end_index].in_handle;

		if handle1.is_none() {
			return Some(Bezier::from_linear_dvec2(start, end));
		}
		if handle2.is_none() {
			return Some(Bezier::from_quadratic_dvec2(start, handle1.unwrap(), end));
		}
		Some(Bezier::from_cubic_dvec2(start, handle1.unwrap(), handle2.unwrap(), end))
	}
}

impl SubPath {
	/// Create a subpath of length 2, using a bezier
	pub fn from_bezier(bezier: Bezier) -> Self {
		SubPath {
			manipulator_groups: vec![
				ManipulatorGroup {
					anchor: bezier.start(),
					in_handle: None,
					out_handle: bezier.handle_start(),
				},
				ManipulatorGroup {
					anchor: bezier.end(),
					in_handle: bezier.handle_end(),
					out_handle: None,
				},
			],
			closed: false,
		}
	}

	fn iter(&self) -> SubPathIter {
		SubPathIter { sub_path: self, index: 0 }
	}

	pub fn length(&self, num_subdivisions: Option<i32>) -> f64 {
		self.iter().map(|bezier| bezier.length(num_subdivisions)).sum()
	}
}

#[cfg(test)]
mod tests {

	use glam::DVec2;

	use crate::Bezier;

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

		let mut subpath = SubPath {
			manipulator_groups: vec![
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
			closed: false,
		};

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

		let mut subpath = SubPath {
			manipulator_groups: vec![
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
			closed: false,
		};

		assert_eq!(subpath.length(None), bezier1.length(None) + bezier2.length(None));

		subpath.closed = true;

		assert_eq!(subpath.length(None), bezier1.length(None) + bezier2.length(None) + bezier3.length(None));
	}
}

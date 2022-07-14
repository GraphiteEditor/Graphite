use glam::DVec2;

use crate::Bezier;

pub struct ManipulatorGroup {
	pub anchor: DVec2,
	pub in_handle: Option<DVec2>,
	pub out_handle: Option<DVec2>,
}

pub struct SubPath {
	manipulator_groups: Vec<ManipulatorGroup>,
	closed: bool,
}

pub struct SubPathIter<'a> {
	index: usize,
	sub_path: &'a SubPath,
}

impl Iterator for SubPathIter<'_> {
	type Item = Bezier;

	fn next(&mut self) -> Option<Self::Item> {
		if self.index >= self.sub_path.len() - 1 + (self.sub_path.closed as usize) {
			return None;
		}
		let start_index = self.index;
		let end_index = (self.index + 1) % self.sub_path.len();
		self.index += 1;

		let start = self.sub_path.manipulator_groups[start_index].anchor;
		let end = self.sub_path.manipulator_groups[end_index].anchor;
		let handle1 = self.sub_path.manipulator_groups[start_index].out_handle;
		let handle2 = self.sub_path.manipulator_groups[end_index].in_handle;

		if handle1.is_none() && handle2.is_none() {
			return Some(Bezier::from_linear_dvec2(start, end));
		}
		if handle1.is_none() || handle2.is_none() {
			return Some(Bezier::from_quadratic_dvec2(start, handle1.or(handle2).unwrap(), end));
		}
		Some(Bezier::from_cubic_dvec2(start, handle1.unwrap(), handle2.unwrap(), end))
	}
}

impl SubPath {
	/// Create a new SubPath using a list of ManipulatorGroups.
	/// A SubPath with less than 2 ManipulatorGroups may not be closed.
	pub fn new(manipulator_groups: Vec<ManipulatorGroup>, closed: bool) -> SubPath {
		assert!(!closed || manipulator_groups.len() > 1);
		SubPath { manipulator_groups, closed }
	}

	/// Create a subpath consisting of 2 manipulator groups from a bezier.
	pub fn from_bezier(bezier: Bezier) -> Self {
		SubPath::new(
			vec![
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
			false,
		)
	}

	/// Returns true if and only if the subpath contains at least one manipulator point
	pub fn is_empty(&self) -> bool {
		self.manipulator_groups.is_empty()
	}

	/// Returns the number of ManipulatorGroups contained within the subpath.
	pub fn len(&self) -> usize {
		self.manipulator_groups.len()
	}

	/// Returns an iterator of the Beziers along the subpath.
	pub fn iter(&self) -> SubPathIter {
		SubPathIter { sub_path: self, index: 0 }
	}

	pub fn to_svg(&self) -> String {
		if self.is_empty() {
			return String::new();
		}
		let mut path_pieces = vec![format!("M {} {}", self.manipulator_groups[0].anchor.x, self.manipulator_groups[0].anchor.y)];
		let mut handle_pieces = Vec::new();
		let mut handle_circles = Vec::new();

		for start_index in 0..self.len() + (self.closed as usize) - 1 {
			let start = self.manipulator_groups[start_index].anchor;
			let end_index = (start_index + 1) % self.len();
			let end = self.manipulator_groups[end_index].anchor;
			let handle1 = self.manipulator_groups[start_index].out_handle;
			let handle2 = self.manipulator_groups[end_index].in_handle;

			if handle1.is_some() {
				handle_pieces.push(format!("M {} {} L {} {} ", start.x, start.y, handle1.unwrap().x, handle1.unwrap().y));
				handle_circles.push(format!(r#"<circle cx="{}" cy="{}" r="{}"/>"#, handle1.unwrap().x, handle1.unwrap().y, 3));
			}
			if handle2.is_some() {
				handle_pieces.push(format!("M {} {} L {} {}", end.x, end.y, handle2.unwrap().x, handle2.unwrap().y));
				handle_circles.push(format!(r#"<circle cx="{}" cy="{}" r="{}"/>"#, handle2.unwrap().x, handle2.unwrap().y, 3));
			}

			let main_path = {
				if handle1.is_none() && handle2.is_none() {
					String::from("L")
				} else if handle1.is_none() || handle2.is_none() {
					let handle = handle1.or(handle2).unwrap();
					format!("Q {} {}", handle.x, handle.y)
				} else {
					format!("C {} {} {} {}", handle1.unwrap().x, handle1.unwrap().y, handle2.unwrap().x, handle2.unwrap().y)
				}
			};
			path_pieces.push(format!("{} {} {}", main_path, end.x, end.y));
		}

		format!(
			r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {} {}" width="{}px" height="{}px"><path d="{} {}" stroke="black" fill="transparent"/><path d="{} {}" stroke="red" fill="transparent"/>{}</svg>"#,
			0,
			0,
			100,
			100,
			100,
			100,
			"\n",
			path_pieces.join(" "),
			"\n",
			handle_pieces.join(" "),
			handle_circles.join(""),
		)
	}

	/// Return the sum of the approximation of the length of each bezier curve along the subpath.
	/// - `num_subdivisions` - Number of subdivisions used to approximate the curve. The default value is 1000.
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

use super::*;
use crate::utils::SubpathTValue;
use crate::utils::TValue;

/// Functionality that transforms Subpaths, such as split, reduce, offset, etc.
impl Subpath {
	/// Returns either one or two Subpaths that result from splitting the original Subpath at the point corresponding to `t`.
	/// If the original Subpath was closed, a single open Subpath will be returned.
	/// If the original Subpath was open, two open Subpaths will be returned.
	/// <iframe frameBorder="0" width="100%" height="400px" src="https://graphite.rs/bezier-rs-demos#subpath/split/solo" title="Split Demo"></iframe>
	pub fn split(&self, t: SubpathTValue) -> (Subpath, Option<Subpath>) {
		let (segment_index, t) = self.t_value_to_parametric(t);
		let curve = self.get_segment(segment_index).unwrap();

		let [first_bezier, second_bezier] = curve.split(TValue::Parametric(t));

		let mut clone = self.manipulator_groups.clone();
		// Split the manipulator group list such that the split location is between the last and first elements of the two split halves
		// If the split is on an anchor point, include this anchor point in the first half of the split, except for the first manipulator group which we want in the second group
		let (mut first_split, mut second_split) = if !(t == 0. && segment_index == 0) {
			let clone2 = clone.split_off(self.len().min(segment_index + 1 + (t == 1.) as usize));
			(clone, clone2)
		} else {
			(vec![], clone)
		};

		// If the subpath is closed and the split point is the start or end of the Subpath
		if self.closed && ((t == 0. && segment_index == 0) || (t == 1. && segment_index == self.len_segments() - 1)) {
			// The entire vector of manipulator groups will be in the second_split
			// Add a new manipulator group with the same anchor as the first node to represent the end of the now opened subpath
			let last_curve = self.iter().last().unwrap();
			first_split.push(ManipulatorGroup {
				anchor: first_bezier.end(),
				in_handle: last_curve.handle_end(),
				out_handle: None,
			});
		} else {
			if !first_split.is_empty() {
				let num_elements = first_split.len();
				first_split[num_elements - 1].out_handle = first_bezier.handle_start();
			}

			if !second_split.is_empty() {
				second_split[0].in_handle = second_bezier.handle_end();
			}

			// Push new manipulator groups to represent the location of the split at the end of the first group and at the start of the second
			// If the split was at a manipulator group's anchor, add only one manipulator group
			// Add it to the first list when the split location is on the first manipulator group, otherwise add to the second list
			if (t % 1. != 0.) || segment_index == 0 {
				first_split.push(ManipulatorGroup {
					anchor: first_bezier.end(),
					in_handle: first_bezier.handle_end(),
					out_handle: None,
				});
			}

			if !(t == 0. && segment_index == 0) {
				second_split.insert(
					0,
					ManipulatorGroup {
						anchor: second_bezier.start(),
						in_handle: None,
						out_handle: second_bezier.handle_start(),
					},
				);
			}
		}

		if self.closed {
			// "Rotate" the manipulator groups list so that the split point becomes the start and end of the open subpath
			second_split.append(&mut first_split);
			(Subpath::new(second_split, false), None)
		} else {
			(Subpath::new(first_split, false), Some(Subpath::new(second_split, false)))
		}
	}
}

#[cfg(test)]
mod tests {
	use crate::utils::SubpathTValue;

	use super::*;
	use glam::DVec2;

	fn set_up_open_subpath() -> Subpath {
		let start = DVec2::new(20., 30.);
		let middle1 = DVec2::new(80., 90.);
		let middle2 = DVec2::new(100., 100.);
		let end = DVec2::new(60., 45.);

		let handle1 = DVec2::new(75., 85.);
		let handle2 = DVec2::new(40., 30.);
		let handle3 = DVec2::new(10., 10.);

		Subpath::new(
			vec![
				ManipulatorGroup {
					anchor: start,
					in_handle: None,
					out_handle: Some(handle1),
				},
				ManipulatorGroup {
					anchor: middle1,
					in_handle: None,
					out_handle: Some(handle2),
				},
				ManipulatorGroup {
					anchor: middle2,
					in_handle: None,
					out_handle: None,
				},
				ManipulatorGroup {
					anchor: end,
					in_handle: None,
					out_handle: Some(handle3),
				},
			],
			false,
		)
	}

	fn set_up_closed_subpath() -> Subpath {
		let mut subpath = set_up_open_subpath();
		subpath.closed = true;
		subpath
	}

	#[test]
	fn split_an_open_subpath() {
		let subpath = set_up_open_subpath();
		let location = subpath.evaluate(SubpathTValue::GlobalParametric(0.2));
		let split_pair = subpath.iter().next().unwrap().split(TValue::Parametric((0.2 * 3.) % 1.));
		let (first, second) = subpath.split(SubpathTValue::GlobalParametric(0.2));
		assert!(second.is_some());
		let second = second.unwrap();
		assert_eq!(first.manipulator_groups[1].anchor, location);
		assert_eq!(second.manipulator_groups[0].anchor, location);
		assert_eq!(split_pair[0], first.iter().last().unwrap());
		assert_eq!(split_pair[1], second.iter().next().unwrap());
	}

	#[test]
	fn split_at_start_of_an_open_subpath() {
		let subpath = set_up_open_subpath();
		let location = subpath.evaluate(SubpathTValue::GlobalParametric(0.));
		let split_pair = subpath.iter().next().unwrap().split(TValue::Parametric(0.));
		let (first, second) = subpath.split(SubpathTValue::GlobalParametric(0.));
		assert!(second.is_some());
		let second = second.unwrap();
		assert_eq!(
			first.manipulator_groups[0],
			ManipulatorGroup {
				anchor: location,
				in_handle: None,
				out_handle: None
			}
		);
		assert_eq!(first.manipulator_groups.len(), 1);
		assert_eq!(second.manipulator_groups[0].anchor, location);
		assert_eq!(split_pair[1], second.iter().next().unwrap());
	}

	#[test]
	fn split_at_end_of_an_open_subpath() {
		let subpath = set_up_open_subpath();
		let location = subpath.evaluate(SubpathTValue::GlobalParametric(1.));
		let split_pair = subpath.iter().last().unwrap().split(TValue::Parametric(1.));
		let (first, second) = subpath.split(SubpathTValue::GlobalParametric(1.));
		assert!(second.is_some());
		let second = second.unwrap();
		assert_eq!(first.manipulator_groups[3].anchor, location);
		assert_eq!(split_pair[0], first.iter().last().unwrap());
		assert_eq!(
			second.manipulator_groups[0],
			ManipulatorGroup {
				anchor: location,
				in_handle: None,
				out_handle: None
			}
		);
		assert_eq!(second.manipulator_groups.len(), 1);
	}

	#[test]
	fn split_a_closed_subpath() {
		let subpath = set_up_closed_subpath();
		let location = subpath.evaluate(SubpathTValue::GlobalParametric(0.2));
		let split_pair = subpath.iter().next().unwrap().split(TValue::Parametric((0.2 * 4.) % 1.));
		let (first, second) = subpath.split(SubpathTValue::GlobalParametric(0.2));
		assert!(second.is_none());
		assert_eq!(first.manipulator_groups[0].anchor, location);
		assert_eq!(first.manipulator_groups[5].anchor, location);
		assert_eq!(first.manipulator_groups.len(), 6);
		assert_eq!(split_pair[0], first.iter().last().unwrap());
		assert_eq!(split_pair[1], first.iter().next().unwrap());
	}

	#[test]
	fn split_at_start_of_a_closed_subpath() {
		let subpath = set_up_closed_subpath();
		let location = subpath.evaluate(SubpathTValue::GlobalParametric(0.));
		let (first, second) = subpath.split(SubpathTValue::GlobalParametric(0.));
		assert!(second.is_none());
		assert_eq!(first.manipulator_groups[0].anchor, location);
		assert_eq!(first.manipulator_groups[4].anchor, location);
		assert_eq!(subpath.manipulator_groups[0..], first.manipulator_groups[..4]);
		assert!(!first.closed);
		assert_eq!(first.iter().last().unwrap(), subpath.iter().last().unwrap());
		assert_eq!(first.iter().next().unwrap(), subpath.iter().next().unwrap());
	}

	#[test]
	fn split_at_end_of_a_closed_subpath() {
		let subpath = set_up_closed_subpath();
		let location = subpath.evaluate(SubpathTValue::GlobalParametric(1.));
		let (first, second) = subpath.split(SubpathTValue::GlobalParametric(1.));
		assert!(second.is_none());
		assert_eq!(first.manipulator_groups[0].anchor, location);
		assert_eq!(first.manipulator_groups[4].anchor, location);
		assert_eq!(subpath.manipulator_groups[0..], first.manipulator_groups[..4]);
		assert!(!first.closed);
		assert_eq!(first.iter().last().unwrap(), subpath.iter().last().unwrap());
		assert_eq!(first.iter().next().unwrap(), subpath.iter().next().unwrap());
	}
}

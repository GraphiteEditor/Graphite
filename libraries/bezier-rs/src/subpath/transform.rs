use super::*;
use crate::utils::SubpathTValue;
use crate::utils::TValue;

use glam::DAffine2;

/// Helper function to ensure the index and t value pair is mapped within a maximum index value.
/// Allows for the point to be fetched without needing to handle an additional edge case.
/// - Ex. Via `subpath.iter().nth(index).evaluate(t);`
fn map_index_within_range(index: usize, t: f64, max_size: usize) -> (usize, f64) {
	if max_size > 0 && index == max_size && t == 0. {
		(index - 1, 1.)
	} else {
		(index, t)
	}
}

/// Functionality that transforms Subpaths, such as split, reduce, offset, etc.
impl<ManipulatorGroupId: crate::Identifier> Subpath<ManipulatorGroupId> {
	/// Returns either one or two Subpaths that result from splitting the original Subpath at the point corresponding to `t`.
	/// If the original Subpath was closed, a single open Subpath will be returned.
	/// If the original Subpath was open, two open Subpaths will be returned.
	/// <iframe frameBorder="0" width="100%" height="400px" src="https://graphite.rs/bezier-rs-demos#subpath/split/solo" title="Split Demo"></iframe>
	pub fn split(&self, t: SubpathTValue) -> (Subpath<ManipulatorGroupId>, Option<Subpath<ManipulatorGroupId>>) {
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
				id: ManipulatorGroupId::new(),
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
					id: ManipulatorGroupId::new(),
				});
			}

			if !(t == 0. && segment_index == 0) {
				second_split.insert(
					0,
					ManipulatorGroup {
						anchor: second_bezier.start(),
						in_handle: None,
						out_handle: second_bezier.handle_start(),
						id: ManipulatorGroupId::new(),
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

	/// Returns [ManipulatorGroup]s with a reversed winding order.
	fn reverse_manipulator_groups(manipulator_groups: &[ManipulatorGroup<ManipulatorGroupId>]) -> Vec<ManipulatorGroup<ManipulatorGroupId>> {
		manipulator_groups
			.iter()
			.rev()
			.map(|group| ManipulatorGroup {
				anchor: group.anchor,
				in_handle: group.out_handle,
				out_handle: group.in_handle,
				id: ManipulatorGroupId::new(),
			})
			.collect::<Vec<ManipulatorGroup<ManipulatorGroupId>>>()
	}

	/// Returns a [Subpath] with a reversed winding order.
	pub fn reverse(&self) -> Subpath<ManipulatorGroupId> {
		Subpath {
			manipulator_groups: Subpath::reverse_manipulator_groups(&self.manipulator_groups),
			closed: self.closed,
		}
	}

	/// Returns an open [Subpath] that results from trimming the original Subpath between the points corresponding to `t1` and `t2`, maintaining the winding order of the original.
	/// If the original Subpath is closed, the order of arguments does matter.
	/// The resulting Subpath will wind from the given `t1` to `t2`.
	/// That means, if the value of `t1` > `t2`, it will cross the break between endpoints from `t1` to `t = 1 = 0` to `t2`.
	/// If a path winding in the reverse direction is desired, call `trim` on the `Subpath` returned from `Subpath::reverse`.
	/// <iframe frameBorder="0" width="100%" height="400px" src="https://graphite.rs/bezier-rs-demos#subpath/trim/solo" title="Trim Demo"></iframe>
	pub fn trim(&self, t1: SubpathTValue, t2: SubpathTValue) -> Subpath<ManipulatorGroupId> {
		// Return a clone of the Subpath if it is not long enough to be a valid Bezier
		if self.manipulator_groups.is_empty() {
			return Subpath {
				manipulator_groups: vec![],
				closed: self.closed,
			};
		}

		let (mut t1_curve_index, mut t1_curve_t) = self.t_value_to_parametric(t1);
		let (mut t2_curve_index, mut t2_curve_t) = self.t_value_to_parametric(t2);

		// The only case where t would be 1 is when the input parameter refers to the the very last point on the subpath.
		// We want these index and t pairs to always represent that point as the next curve index with t == 0.
		if t1_curve_t == 1. {
			t1_curve_index += 1;
			t1_curve_t = 0.;
		}
		if t2_curve_t == 1. {
			t2_curve_index += 1;
			t2_curve_t = 0.;
		}

		// Check if the trimmed result is in the reverse direction
		let are_arguments_reversed = t1_curve_index > t2_curve_index || (t1_curve_index == t2_curve_index && t1_curve_t > t2_curve_t);
		if !self.closed && are_arguments_reversed {
			(t1_curve_index, t2_curve_index) = (t2_curve_index, t1_curve_index);
			(t1_curve_t, t2_curve_t) = (t2_curve_t, t1_curve_t);
		}

		// Get a new list from the manipulator groups that will be trimmed at the ends to form the resulting subpath.
		// The list will contain enough manipulator groups such that the later code simply needs to trim the first and last bezier segments
		// and then update the values of the corresponding first and last manipulator groups accordingly.
		let mut cloned_manipulator_groups = self.manipulator_groups.clone();
		let mut new_manipulator_groups = if self.closed && are_arguments_reversed {
			// Need to rotate the cloned manipulator groups vector
			// Remove the elements starting from t1_curve_index to become the new beginning of the list
			let mut front = cloned_manipulator_groups.split_off(t1_curve_index);
			// Truncate middle elements that are not needed
			cloned_manipulator_groups.truncate(t2_curve_index + ((t2_curve_t != 0.) as usize) + 1);
			// Reconnect the two ends in the new order
			front.extend(cloned_manipulator_groups);
			if t1_curve_index == t2_curve_index % self.len_segments() {
				// If the start and end of the trim are in the same bezier segment, we want to add a duplicate of the first two manipulator groups.
				// This is to make sure the the closed loop is correctly represented and because this segment needs to be trimmed on both ends of the resulting subpath.
				front.push(front[0].clone());
				front.push(front[1].clone());
			}
			if t1_curve_index == t2_curve_index % self.len_segments() + 1 {
				// If the start and end of the trim are in adjacent bezier segments, we want to add a duplicate of the first manipulator group.
				// This is to make sure the the closed loop is correctly represented.
				front.push(front[0].clone());
			}
			front
		} else {
			// Determine the subsection of the subpath's manipulator groups that are needed
			if self.closed {
				// Add a duplicate of the first manipulator group to ensure the final closing segment is considered
				cloned_manipulator_groups.push(cloned_manipulator_groups[0].clone());
			}

			// Find the start and end of the new range and consider whether the indices are reversed
			let range_start = t1_curve_index.min(t2_curve_index);
			// Add 1 since the drain range is not inclusive
			// Add 1 again if the corresponding t is not 0 because we want to include the next manipulator group which forms the bezier that this t value is on
			let range_end = 1 + t2_curve_index + ((t2_curve_t != 0.) as usize);

			cloned_manipulator_groups
				.drain(range_start..range_end.min(cloned_manipulator_groups.len()))
				.collect::<Vec<ManipulatorGroup<ManipulatorGroupId>>>()
		};

		// Adjust curve indices to match the cloned list
		if self.closed && are_arguments_reversed {
			// If trimmed subpath required rotating the manipulator group, adjust the indices to match
			t2_curve_index = (t2_curve_index + self.len_segments() - t1_curve_index) % self.len_segments();
			if t2_curve_index == 0 {
				// If the case is where the start and end are in the same bezier,
				// change the index to point to the duplicate of this bezier that was pushed to the vector
				t2_curve_index += self.len_segments();
			}
			t1_curve_index = 0;
		} else {
			let min_index = t1_curve_index.min(t2_curve_index);
			t1_curve_index -= min_index;
			t2_curve_index -= min_index;
		}

		// Change the representation of the point corresponding to the end point of the subpath
		// So that we do not need an additional edges case in the later code to handle this point
		(t1_curve_index, t1_curve_t) = map_index_within_range(t1_curve_index, t1_curve_t, new_manipulator_groups.len() - 1);
		(t2_curve_index, t2_curve_t) = map_index_within_range(t2_curve_index, t2_curve_t, new_manipulator_groups.len() - 1);

		if new_manipulator_groups.len() == 1 {
			// This case will occur when `t1` and `t2` both represent one of the manipulator group anchors
			// Add a duplicate manipulator group so that the returned Subpath is still a valid Bezier
			let mut point = new_manipulator_groups[0].clone();
			point.in_handle = None;
			point.out_handle = None;
			return Subpath {
				manipulator_groups: vec![point],
				closed: false,
			};
		}

		let len_new_manip_groups = new_manipulator_groups.len();

		// Create Beziers from the first and last pairs of manipulator groups
		// These will be trimmed to form the start and end of the new subpath
		let curve1 = new_manipulator_groups[0].to_bezier(&new_manipulator_groups[1]);
		let curve2 = new_manipulator_groups[len_new_manip_groups - 2].to_bezier(&new_manipulator_groups[len_new_manip_groups - 1]);

		// If the target curve_indices are the same, then the trim must be happening within one bezier
		// This means curve1 == curve2 must be true, and we can simply call the Bezier trim.
		if t1_curve_index == t2_curve_index {
			return Subpath::from_bezier(curve1.trim(TValue::Parametric(t1_curve_t), TValue::Parametric(t2_curve_t)));
		}

		// Split the bezier's with the according t value and keep the correct half
		let [_, front_split] = curve1.split(TValue::Parametric(t1_curve_t));
		let [back_split, _] = curve2.split(TValue::Parametric(t2_curve_t));

		// Update the first two manipulator groups to match the front_split
		new_manipulator_groups[1].in_handle = front_split.handle_end();
		new_manipulator_groups[0] = ManipulatorGroup {
			anchor: front_split.start(),
			in_handle: None,
			out_handle: front_split.handle_start(),
			id: ManipulatorGroupId::new(),
		};

		// Update the last two manipulator groups to match the back_split
		new_manipulator_groups[len_new_manip_groups - 2].out_handle = back_split.handle_start();
		new_manipulator_groups[len_new_manip_groups - 1] = ManipulatorGroup {
			anchor: back_split.end(),
			in_handle: back_split.handle_end(),
			out_handle: None,
			id: ManipulatorGroupId::new(),
		};

		Subpath {
			manipulator_groups: new_manipulator_groups,
			closed: false,
		}
	}

	/// Apply a transformation to all of the [ManipulatorGroup]s in the [Subpath].
	pub fn apply_transform(&mut self, affine_transform: DAffine2) {
		for manipulator_group in &mut self.manipulator_groups {
			manipulator_group.apply_transform(affine_transform);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::{ManipulatorGroup, Subpath};
	use crate::compare::{compare_points, compare_subpaths, compare_vec_of_points};
	use crate::consts::MAX_ABSOLUTE_DIFFERENCE;
	use crate::utils::{SubpathTValue, TValue};
	use crate::EmptyId;
	use glam::DVec2;

	fn set_up_open_subpath() -> Subpath<EmptyId> {
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
					id: EmptyId,
				},
				ManipulatorGroup {
					anchor: middle1,
					in_handle: None,
					out_handle: Some(handle2),
					id: EmptyId,
				},
				ManipulatorGroup {
					anchor: middle2,
					in_handle: None,
					out_handle: None,
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
		)
	}

	fn set_up_closed_subpath() -> Subpath<EmptyId> {
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
				out_handle: None,
				id: EmptyId,
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
				out_handle: None,
				id: EmptyId,
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

	#[test]
	fn reverse_an_open_subpath() {
		let subpath = set_up_open_subpath();
		let temporary = subpath.reverse();
		let result = temporary.reverse();
		let end = result.len();

		assert_eq!(temporary.manipulator_groups[0].anchor, result.manipulator_groups[end - 1].anchor);
		assert_eq!(temporary.manipulator_groups[0].out_handle, result.manipulator_groups[end - 1].in_handle);
		assert_eq!(subpath, result);
	}

	#[test]
	fn reverse_a_closed_subpath() {
		let subpath = set_up_closed_subpath();
		let temporary = subpath.reverse();
		let result = temporary.reverse();
		let end = result.len();

		assert_eq!(temporary.manipulator_groups[0].anchor, result.manipulator_groups[end - 1].anchor);
		assert_eq!(temporary.manipulator_groups[0].in_handle, result.manipulator_groups[end - 1].out_handle);
		assert_eq!(temporary.manipulator_groups[0].out_handle, result.manipulator_groups[end - 1].in_handle);
		assert_eq!(subpath, result);
	}

	#[test]
	fn trim_an_open_subpath() {
		let subpath = set_up_open_subpath();
		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(0.2));
		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(0.8));
		let [_, trim_front] = subpath.iter().next().unwrap().split(TValue::Parametric((0.2 * 3.) % 1.));
		let [trim_back, _] = subpath.iter().last().unwrap().split(TValue::Parametric((0.8 * 3.) % 1.));
		let result = subpath.trim(SubpathTValue::GlobalParametric(0.2), SubpathTValue::GlobalParametric(0.8));

		assert_eq!(result.manipulator_groups[0].anchor, location_front);
		assert_eq!(result.manipulator_groups[3].anchor, location_back);
		assert_eq!(trim_front, result.iter().next().unwrap());
		assert_eq!(trim_back, result.iter().last().unwrap());
	}

	#[test]
	fn trim_within_a_bezier() {
		let subpath = set_up_open_subpath();
		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(0.1));
		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(0.2));
		let trimmed = subpath.iter().next().unwrap().trim(TValue::Parametric((0.1 * 3.) % 1.), TValue::Parametric((0.2 * 3.) % 1.));
		let result = subpath.trim(SubpathTValue::GlobalParametric(0.1), SubpathTValue::GlobalParametric(0.2));
		assert!(compare_points(result.manipulator_groups[0].anchor, location_front));
		assert!(compare_points(result.manipulator_groups[1].anchor, location_back));
		assert_eq!(trimmed, result.iter().next().unwrap());
		assert_eq!(result.len(), 2);
	}

	#[test]
	fn trim_first_segment_of_an_open_subpath() {
		let subpath = set_up_closed_subpath();
		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(0.));
		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(0.25));
		let trimmed = subpath.iter().next().unwrap().trim(TValue::Parametric(0.), TValue::Parametric(1.));
		let result = subpath.trim(SubpathTValue::GlobalParametric(0.), SubpathTValue::GlobalParametric(0.25));

		assert_eq!(result.manipulator_groups[0].anchor, location_front);
		assert_eq!(result.manipulator_groups[1].anchor, location_back);
		assert_eq!(trimmed, result.iter().next().unwrap());
	}

	#[test]
	fn trim_second_segment_of_an_open_subpath() {
		let subpath = set_up_closed_subpath();
		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(0.25));
		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(0.5));
		let trimmed = subpath.iter().nth(1).unwrap().trim(TValue::Parametric(0.), TValue::Parametric(1.));
		let result = subpath.trim(SubpathTValue::GlobalParametric(0.25), SubpathTValue::GlobalParametric(0.5));

		assert_eq!(result.manipulator_groups[0].anchor, location_front);
		assert_eq!(result.manipulator_groups[1].anchor, location_back);
		assert_eq!(trimmed, result.iter().next().unwrap());
	}

	#[test]
	fn trim_reverse_in_open_subpath() {
		let subpath = set_up_open_subpath();
		let result1 = subpath.trim(SubpathTValue::GlobalParametric(0.8), SubpathTValue::GlobalParametric(0.2));
		let result2 = subpath.trim(SubpathTValue::GlobalParametric(0.2), SubpathTValue::GlobalParametric(0.8));

		assert!(compare_subpaths(&result1, &result2));
	}

	#[test]
	fn trim_reverse_within_a_bezier() {
		let subpath = set_up_open_subpath();
		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(0.1));
		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(0.2));
		let trimmed = subpath.iter().next().unwrap().trim(TValue::Parametric((0.2 * 3.) % 1.), TValue::Parametric((0.1 * 3.) % 1.));
		let result = subpath.trim(SubpathTValue::GlobalParametric(0.2), SubpathTValue::GlobalParametric(0.1));

		assert!(compare_points(result.manipulator_groups[0].anchor, location_front));
		assert!(compare_points(result.manipulator_groups[1].anchor, location_back));
		assert!(compare_vec_of_points(
			trimmed.get_points().collect(),
			result.iter().next().unwrap().get_points().collect(),
			MAX_ABSOLUTE_DIFFERENCE
		));
		assert_eq!(result.len(), 2);
	}

	#[test]
	fn trim_a_duplicate_subpath() {
		let subpath = set_up_open_subpath();
		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(0.));
		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(1.));
		let result = subpath.trim(SubpathTValue::GlobalParametric(0.), SubpathTValue::GlobalParametric(1.));

		// Assume that resulting subpath would no longer have the any meaningless handles
		let mut expected_subpath = subpath.clone();
		expected_subpath[3].out_handle = None;

		assert_eq!(result.manipulator_groups[0].anchor, location_front);
		assert!(compare_points(result.manipulator_groups[3].anchor, location_back));
		assert_eq!(expected_subpath, result);
	}

	#[test]
	fn trim_a_reversed_duplicate_subpath() {
		let subpath = set_up_open_subpath();
		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(0.));
		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(1.));
		let result = subpath.trim(SubpathTValue::GlobalParametric(1.), SubpathTValue::GlobalParametric(0.));

		assert_eq!(result.manipulator_groups[0].anchor, location_front);
		assert_eq!(result.manipulator_groups[3].anchor, location_back);
		assert!(compare_subpaths(&subpath, &result));
	}

	#[test]
	fn trim_to_end_of_subpath() {
		let subpath = set_up_open_subpath();
		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(0.8));
		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(1.));
		let trimmed = subpath.iter().last().unwrap().trim(TValue::Parametric((0.8 * 3.) % 1.), TValue::Parametric(1.));
		let result = subpath.trim(SubpathTValue::GlobalParametric(0.8), SubpathTValue::GlobalParametric(1.));

		assert_eq!(result.manipulator_groups[0].anchor, location_front);
		assert!(compare_points(result.manipulator_groups[1].anchor, location_back));
		assert_eq!(trimmed, result.iter().next().unwrap());
	}

	#[test]
	fn trim_reversed_to_end_of_subpath() {
		let subpath = set_up_open_subpath();
		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(0.));
		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(0.2));
		let trimmed = subpath.iter().next().unwrap().trim(TValue::Parametric((0.2 * 3.) % 1.), TValue::Parametric(0.));
		let result = subpath.trim(SubpathTValue::GlobalParametric(0.2), SubpathTValue::GlobalParametric(0.));

		assert!(compare_points(result.manipulator_groups[0].anchor, location_front));
		assert!(compare_points(result.manipulator_groups[1].anchor, location_back));
		assert!(compare_vec_of_points(
			trimmed.get_points().collect(),
			result.iter().next().unwrap().get_points().collect(),
			MAX_ABSOLUTE_DIFFERENCE
		));
	}

	#[test]
	fn trim_start_point() {
		let subpath = set_up_open_subpath();
		let location = subpath.evaluate(SubpathTValue::GlobalParametric(0.));
		let result = subpath.trim(SubpathTValue::GlobalParametric(0.), SubpathTValue::GlobalParametric(0.));

		assert!(compare_points(result.manipulator_groups[0].anchor, location));
		assert!(result.manipulator_groups[0].in_handle.is_none());
		assert!(result.manipulator_groups[0].out_handle.is_none());
		assert_eq!(result.len(), 1);
	}

	#[test]
	fn trim_middle_point() {
		let subpath = set_up_closed_subpath();
		let location = subpath.evaluate(SubpathTValue::GlobalParametric(0.25));
		let result = subpath.trim(SubpathTValue::GlobalParametric(0.25), SubpathTValue::GlobalParametric(0.25));

		assert!(compare_points(result.manipulator_groups[0].anchor, location));
		assert!(result.manipulator_groups[0].in_handle.is_none());
		assert!(result.manipulator_groups[0].out_handle.is_none());
		assert_eq!(result.len(), 1);
	}

	#[test]
	fn trim_a_closed_subpath() {
		let subpath = set_up_closed_subpath();
		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(0.2));
		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(0.8));
		let [_, trim_front] = subpath.iter().next().unwrap().split(TValue::Parametric((0.2 * 4.) % 1.));
		let [trim_back, _] = subpath.iter().last().unwrap().split(TValue::Parametric((0.8 * 4.) % 1.));
		let result = subpath.trim(SubpathTValue::GlobalParametric(0.2), SubpathTValue::GlobalParametric(0.8));

		assert_eq!(result.manipulator_groups[0].anchor, location_front);
		assert_eq!(result.manipulator_groups[4].anchor, location_back);
		assert_eq!(trim_front, result.iter().next().unwrap());
		assert_eq!(trim_back, result.iter().last().unwrap());
	}

	#[test]
	fn trim_to_end_of_closed_subpath() {
		let subpath = set_up_closed_subpath();
		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(0.8));
		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(1.));
		let trimmed = subpath.iter().last().unwrap().trim(TValue::Parametric((0.8 * 4.) % 1.), TValue::Parametric(1.));
		let result = subpath.trim(SubpathTValue::GlobalParametric(0.8), SubpathTValue::GlobalParametric(1.));

		assert_eq!(result.manipulator_groups[0].anchor, location_front);
		assert!(compare_points(result.manipulator_groups[1].anchor, location_back));
		assert_eq!(trimmed, result.iter().next().unwrap());
	}

	#[test]
	fn trim_across_break_in_a_closed_subpath() {
		let subpath = set_up_closed_subpath();
		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(0.8));
		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(0.2));
		let [_, trim_front] = subpath.iter().last().unwrap().split(TValue::Parametric((0.8 * 4.) % 1.));
		let [trim_back, _] = subpath.iter().next().unwrap().split(TValue::Parametric((0.2 * 4.) % 1.));
		let result = subpath.trim(SubpathTValue::GlobalParametric(0.8), SubpathTValue::GlobalParametric(0.2));

		assert_eq!(result.manipulator_groups[0].anchor, location_front);
		assert_eq!(result.manipulator_groups[2].anchor, location_back);
		assert_eq!(trim_front, result.iter().next().unwrap());
		assert_eq!(trim_back, result.iter().last().unwrap());
	}

	#[test]
	fn trim_across_break_in_a_closed_subpath_where_result_is_multiple_segments() {
		let subpath = set_up_closed_subpath();
		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(0.6));
		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(0.4));
		let [_, trim_front] = subpath.iter().nth(2).unwrap().split(TValue::Parametric((0.6 * 4.) % 1.));
		let [trim_back, _] = subpath.iter().nth(1).unwrap().split(TValue::Parametric((0.4 * 4.) % 1.));
		let result = subpath.trim(SubpathTValue::GlobalParametric(0.6), SubpathTValue::GlobalParametric(0.4));

		assert_eq!(result.manipulator_groups[0].anchor, location_front);
		assert_eq!(result.manipulator_groups[4].anchor, location_back);
		assert_eq!(trim_front, result.iter().next().unwrap());
		assert_eq!(trim_back, result.iter().last().unwrap());
	}

	#[test]
	fn trim_across_break_in_a_closed_subpath_where_ends_are_in_same_segment() {
		let subpath = set_up_closed_subpath();
		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(0.45));
		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(0.4));
		let [_, trim_front] = subpath.iter().nth(1).unwrap().split(TValue::Parametric((0.45 * 4.) % 1.));
		let [trim_back, _] = subpath.iter().nth(1).unwrap().split(TValue::Parametric((0.4 * 4.) % 1.));
		let result = subpath.trim(SubpathTValue::GlobalParametric(0.45), SubpathTValue::GlobalParametric(0.4));

		assert_eq!(result.manipulator_groups[0].anchor, location_front);
		assert_eq!(result.manipulator_groups[5].anchor, location_back);
		assert_eq!(trim_front, result.iter().next().unwrap());
		assert_eq!(trim_back, result.iter().last().unwrap());
	}

	#[test]
	fn trim_at_break_in_closed_subpath_where_end_is_0() {
		let subpath = set_up_closed_subpath();
		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(0.8));
		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(0.));
		let trimmed = subpath.iter().last().unwrap().trim(TValue::Parametric((0.8 * 4.) % 1.), TValue::Parametric(1.));
		let result = subpath.trim(SubpathTValue::GlobalParametric(0.8), SubpathTValue::GlobalParametric(0.));

		assert_eq!(result.manipulator_groups[0].anchor, location_front);
		assert_eq!(result.manipulator_groups[1].anchor, location_back);
		assert_eq!(trimmed, result.iter().next().unwrap());
	}

	#[test]
	fn trim_at_break_in_closed_subpath_where_start_is_1() {
		let subpath = set_up_closed_subpath();
		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(1.));
		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(0.2));
		let trimmed = subpath.iter().next().unwrap().trim(TValue::Parametric(0.), TValue::Parametric((0.2 * 4.) % 1.));
		let result = subpath.trim(SubpathTValue::GlobalParametric(1.), SubpathTValue::GlobalParametric(0.2));

		assert_eq!(result.manipulator_groups[0].anchor, location_front);
		assert_eq!(result.manipulator_groups[1].anchor, location_back);
		assert_eq!(trimmed, result.iter().next().unwrap());
	}

	#[test]
	fn trim_at_break_in_closed_subpath_from_1_to_0() {
		let subpath = set_up_closed_subpath();
		let location = subpath.evaluate(SubpathTValue::GlobalParametric(0.));
		let result = subpath.trim(SubpathTValue::GlobalParametric(1.), SubpathTValue::GlobalParametric(0.));

		assert_eq!(result.manipulator_groups[0].anchor, location);
		assert!(result.manipulator_groups[0].in_handle.is_none());
		assert!(result.manipulator_groups[0].out_handle.is_none());
		assert_eq!(result.manipulator_groups.len(), 1);
	}

	fn transform_subpath() {
		let mut subpath = set_up_open_subpath();
		subpath.apply_transform(glam::DAffine2::IDENTITY);
		assert_eq!(subpath, set_up_open_subpath());
	}
}

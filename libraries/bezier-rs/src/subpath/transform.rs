use std::vec;

use super::*;
use crate::consts::MAX_ABSOLUTE_DIFFERENCE;
use crate::utils::{Cap, Join, SubpathTValue, TValue};

use glam::{DAffine2, DVec2};

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
	/// <iframe frameBorder="0" width="100%" height="350px" src="https://graphite.rs/libraries/bezier-rs#subpath/split/solo" title="Split Demo"></iframe>
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
	/// Note that a reversed closed subpath will start on the same manipulator group and simply wind the other direction
	pub fn reverse(&self) -> Subpath<ManipulatorGroupId> {
		let mut reversed = Subpath::reverse_manipulator_groups(self.manipulator_groups());
		if self.closed {
			reversed.rotate_right(1);
		};
		Subpath {
			manipulator_groups: reversed,
			closed: self.closed,
		}
	}

	/// Returns an open [Subpath] that results from trimming the original Subpath between the points corresponding to `t1` and `t2`, maintaining the winding order of the original.
	/// If the original Subpath is closed, the order of arguments does matter.
	/// The resulting Subpath will wind from the given `t1` to `t2`.
	/// That means, if the value of `t1` > `t2`, it will cross the break between endpoints from `t1` to `t = 1 = 0` to `t2`.
	/// If a path winding in the reverse direction is desired, call `trim` on the `Subpath` returned from `Subpath::reverse`.
	/// <iframe frameBorder="0" width="100%" height="400px" src="https://graphite.rs/libraries/bezier-rs#subpath/trim/solo" title="Trim Demo"></iframe>
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
			return Subpath::from_bezier(&curve1.trim(TValue::Parametric(t1_curve_t), TValue::Parametric(t2_curve_t)));
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

	/// Smooths a Subpath up to the first derivative, using a weighted averaged based on segment length.
	/// The Subpath must be open, and contain no quadratic segments.
	pub(crate) fn smooth_open_subpath(&mut self) {
		if self.len() < 2 {
			return;
		}
		for i in 1..self.len() - 1 {
			let first_bezier = self.manipulator_groups[i - 1].to_bezier(&self.manipulator_groups[i]);
			let second_bezier = self.manipulator_groups[i].to_bezier(&self.manipulator_groups[i + 1]);
			if first_bezier.handle_end().is_none() || second_bezier.handle_end().is_none() {
				continue;
			}
			let end_tangent = first_bezier.non_normalized_tangent(1.);
			let start_tangent = second_bezier.non_normalized_tangent(0.);

			// Compute an average unit vector, weighing the segments by a rough estimation of their relative size.
			let segment1_len = first_bezier.length(Some(5));
			let segment2_len = second_bezier.length(Some(5));
			let average_unit_tangent = (end_tangent.normalize() * segment1_len + start_tangent.normalize() * segment2_len) / (segment1_len + segment2_len);

			// Adjust start and end handles to fit the average tangent
			let end_point = first_bezier.end();
			self.manipulator_groups[i].in_handle = Some((average_unit_tangent / 3. * -1.) * end_tangent.length() + end_point);

			let start_point = second_bezier.start();
			self.manipulator_groups[i].out_handle = Some((average_unit_tangent / 3.) * start_tangent.length() + start_point);
		}
	}

	// TODO: If a segment curls back on itself tightly enough it could intersect again at the portion that should be trimmed. This could cause the Subpaths to be clipped
	// at the incorrect location. This can be avoided by first trimming the two Subpaths at any extrema, effectively ignoring loopbacks.
	/// Helper function to clip overlap of two intersecting open Subpaths. Returns an optional, as intersections may not exist for certain arrangements and distances.
	/// Assumes that the Subpaths represents simple Bezier segments, and clips the Subpaths at the last intersection of the first Subpath, and first intersection of the last Subpath.
	fn clip_simple_subpaths(subpath1: &Subpath<ManipulatorGroupId>, subpath2: &Subpath<ManipulatorGroupId>) -> Option<(Subpath<ManipulatorGroupId>, Subpath<ManipulatorGroupId>)> {
		// Split the first subpath at its last intersection
		let intersections1 = subpath1.subpath_intersections(subpath2, None, None);
		if intersections1.is_empty() {
			return None;
		}
		let (segment_index, t) = *intersections1.last().unwrap();
		let (clipped_subpath1, _) = subpath1.split(SubpathTValue::Parametric { segment_index, t });

		// Split the second subpath at its first intersection
		let intersections2 = subpath2.subpath_intersections(subpath1, None, None);
		if intersections2.is_empty() {
			return None;
		}
		let (segment_index, t) = intersections2[0];
		let (_, clipped_subpath2) = subpath2.split(SubpathTValue::Parametric { segment_index, t });

		Some((clipped_subpath1, clipped_subpath2.unwrap()))
	}

	/// Returns a subpath that results from rotating this subpath around the origin by the given angle (in radians).
	/// <iframe frameBorder="0" width="100%" height="325px" src="https://graphite.rs/libraries/bezier-rs#subpath/rotate/solo" title="Rotate Demo"></iframe>
	pub fn rotate(&self, angle: f64) -> Subpath<ManipulatorGroupId> {
		let mut rotated_subpath = self.clone();

		let affine_transform: DAffine2 = DAffine2::from_angle(angle);
		rotated_subpath.apply_transform(affine_transform);

		rotated_subpath
	}

	/// Returns a subpath that results from rotating this subpath around the provided point by the given angle (in radians).
	pub fn rotate_about_point(&self, angle: f64, pivot: DVec2) -> Subpath<ManipulatorGroupId> {
		// Translate before and after the rotation to account for the pivot
		let translate: DAffine2 = DAffine2::from_translation(pivot);
		let rotate: DAffine2 = DAffine2::from_angle(angle);
		let translate_inverse = translate.inverse();

		let mut rotated_subpath = self.clone();
		rotated_subpath.apply_transform(translate * rotate * translate_inverse);
		rotated_subpath
	}

	/// Reduces the segments of the subpath into simple subcurves, then scales each subcurve a set `distance` away.
	/// The intersections of segments of the subpath are joined using the method specified by the `join` argument.
	/// <iframe frameBorder="0" width="100%" height="400px" src="https://graphite.rs/libraries/bezier-rs#subpath/offset/solo" title="Offset Demo"></iframe>
	pub fn offset(&self, distance: f64, join: Join) -> Subpath<ManipulatorGroupId> {
		assert!(self.len_segments() > 1, "Cannot offset an empty Subpath.");

		// An offset at a distance 0 from the curve is simply the same curve
		// An offset of a single point is not defined
		if distance == 0. || self.len() == 1 {
			return self.clone();
		}

		let mut subpaths = self
			.iter()
			.filter(|bezier| !bezier.is_point())
			.map(|bezier| bezier.offset(distance))
			.collect::<Vec<Subpath<ManipulatorGroupId>>>();
		let mut drop_common_point = vec![true; self.len()];

		// Clip or join consecutive Subpaths
		for i in 0..subpaths.len() - 1 {
			let j = i + 1;
			let subpath1 = &subpaths[i];
			let subpath2 = &subpaths[j];

			let last_segment = subpath1.get_segment(subpath1.len_segments() - 1).unwrap();
			let first_segment = subpath2.get_segment(0).unwrap();

			// If the anchors are approximately equal, there is no need to clip / join the segments
			if last_segment.end().abs_diff_eq(first_segment.start(), MAX_ABSOLUTE_DIFFERENCE) {
				continue;
			}

			// Calculate the angle formed between two consecutive Subpaths
			let out_tangent = self.get_segment(i).unwrap().tangent(TValue::Parametric(1.));
			let in_tangent = self.get_segment(j).unwrap().tangent(TValue::Parametric(0.));
			let angle = out_tangent.angle_between(in_tangent);

			// The angle is concave. The Subpath overlap and must be clipped
			let mut apply_join = true;
			if (angle > 0. && distance > 0.) || (angle < 0. && distance < 0.) {
				// If the distance is large enough, there may still be no intersections. Also, if the angle is close enough to zero,
				// subpath intersections may find no intersections. In this case, the points are likely close enough that we can approximate
				// the points as being on top of one another.
				if let Some((clipped_subpath1, clipped_subpath2)) = Subpath::clip_simple_subpaths(subpath1, subpath2) {
					subpaths[i] = clipped_subpath1;
					subpaths[j] = clipped_subpath2;
					apply_join = false;
				}
			}
			// The angle is convex. The Subpath must be joined using the specified join type
			if apply_join {
				drop_common_point[j] = false;
				match join {
					Join::Bevel => {}
					Join::Miter(miter_limit) => {
						let miter_manipulator_group = subpaths[i].miter_line_join(&subpaths[j], miter_limit);
						if let Some(miter_manipulator_group) = miter_manipulator_group {
							subpaths[i].manipulator_groups.push(miter_manipulator_group);
						}
					}
					Join::Round => {
						let (out_handle, round_point, in_handle) = subpaths[i].round_line_join(&subpaths[j], self.manipulator_groups[j].anchor);
						let last_index = subpaths[i].manipulator_groups.len() - 1;
						subpaths[i].manipulator_groups[last_index].out_handle = Some(out_handle);
						subpaths[i].manipulator_groups.push(round_point.clone());
						subpaths[j].manipulator_groups[0].in_handle = Some(in_handle);
					}
				}
			}
		}

		// Clip any overlap in the last segment
		if self.closed {
			let out_tangent = self.get_segment(self.len_segments() - 1).unwrap().tangent(TValue::Parametric(1.));
			let in_tangent = self.get_segment(0).unwrap().tangent(TValue::Parametric(0.));
			let angle = out_tangent.angle_between(in_tangent);

			let mut apply_join = true;
			if (angle > 0. && distance > 0.) || (angle < 0. && distance < 0.) {
				if let Some((clipped_subpath1, clipped_subpath2)) = Subpath::clip_simple_subpaths(&subpaths[subpaths.len() - 1], &subpaths[0]) {
					// Merge the clipped subpaths
					let last_index = subpaths.len() - 1;
					subpaths[last_index] = clipped_subpath1;
					subpaths[0] = clipped_subpath2;
					apply_join = false;
				}
			}
			if apply_join {
				drop_common_point[0] = false;
				match join {
					Join::Bevel => {}
					Join::Miter(miter_limit) => {
						let last_subpath_index = subpaths.len() - 1;
						let miter_manipulator_group = subpaths[last_subpath_index].miter_line_join(&subpaths[0], miter_limit);
						if let Some(miter_manipulator_group) = miter_manipulator_group {
							subpaths[last_subpath_index].manipulator_groups.push(miter_manipulator_group);
						}
					}
					Join::Round => {
						let last_subpath_index = subpaths.len() - 1;
						let (out_handle, round_point, in_handle) = subpaths[last_subpath_index].round_line_join(&subpaths[0], self.manipulator_groups[0].anchor);
						let last_index = subpaths[last_subpath_index].manipulator_groups.len() - 1;
						subpaths[last_subpath_index].manipulator_groups[last_index].out_handle = Some(out_handle);
						subpaths[last_subpath_index].manipulator_groups.push(round_point);
						subpaths[0].manipulator_groups[0].in_handle = Some(in_handle);
					}
				}
			}
		}

		// Merge the subpaths. Drop points which overlap with one another.
		let mut manipulator_groups = subpaths[0].manipulator_groups.clone();
		for i in 1..subpaths.len() {
			if drop_common_point[i] {
				let last_group = manipulator_groups.pop().unwrap();
				let mut manipulators_copy = subpaths[i].manipulator_groups.clone();
				manipulators_copy[0].in_handle = last_group.in_handle;

				manipulator_groups.append(&mut manipulators_copy);
			} else {
				manipulator_groups.append(&mut subpaths[i].manipulator_groups.clone());
			}
		}
		if self.closed && drop_common_point[0] {
			let last_group = manipulator_groups.pop().unwrap();
			manipulator_groups[0].in_handle = last_group.in_handle;
		}

		Subpath::new(manipulator_groups, self.closed)
	}

	/// Helper function to combine the two offsets that make up an outline.
	pub(crate) fn combine_outline(&self, other: &Subpath<ManipulatorGroupId>, cap: Cap) -> Subpath<ManipulatorGroupId> {
		let mut result_manipulator_groups: Vec<ManipulatorGroup<ManipulatorGroupId>> = vec![];
		result_manipulator_groups.extend_from_slice(self.manipulator_groups());
		match cap {
			Cap::Butt => {
				result_manipulator_groups.extend_from_slice(other.manipulator_groups());
			}
			Cap::Round => {
				let last_index = result_manipulator_groups.len() - 1;
				let (out_handle, round_point, in_handle) = self.round_cap(other);
				result_manipulator_groups[last_index].out_handle = Some(out_handle);
				result_manipulator_groups.push(round_point);
				result_manipulator_groups.extend_from_slice(&other.manipulator_groups);
				result_manipulator_groups[last_index + 2].in_handle = Some(in_handle);

				let last_index = result_manipulator_groups.len() - 1;
				let (out_handle, round_point, in_handle) = other.round_cap(self);
				result_manipulator_groups[last_index].out_handle = Some(out_handle);
				result_manipulator_groups.push(round_point);
				result_manipulator_groups[0].in_handle = Some(in_handle);
			}
			Cap::Square => {
				let square_points = self.square_cap(other);
				result_manipulator_groups.extend_from_slice(&square_points);
				result_manipulator_groups.extend_from_slice(other.manipulator_groups());
				let square_points = other.square_cap(self);
				result_manipulator_groups.extend_from_slice(&square_points);
			}
		}
		Subpath::new(result_manipulator_groups, true)
	}

	// TODO: Replace this return type with `Path`, once the `Path` data type has been created.
	/// Outline returns a single closed subpath (if the original subpath was open) or two closed subpaths (if the original subpath was closed) that forms
	/// an approximate outline around the subpath at a specified distance from the curve. Outline takes the following parameters:
	/// - `distance` - The outline's distance from the curve.
	/// - `join` - The join type used to cap the endpoints of open bezier curves, and join successive subpath segments.
	/// <iframe frameBorder="0" width="100%" height="425px" src="https://graphite.rs/libraries/bezier-rs#subpath/outline/solo" title="Outline Demo"></iframe>
	pub fn outline(&self, distance: f64, join: Join, cap: Cap) -> (Subpath<ManipulatorGroupId>, Option<Subpath<ManipulatorGroupId>>) {
		let is_point = self.is_point();
		let (pos_offset, neg_offset) = if is_point {
			let point = self.manipulator_groups[0].anchor;
			(
				Subpath::new(vec![ManipulatorGroup::new_anchor(point + DVec2::NEG_Y * distance)], false),
				Subpath::new(vec![ManipulatorGroup::new_anchor(point + DVec2::Y * distance)], false),
			)
		} else {
			(self.offset(distance, join), self.reverse().offset(distance, join))
		};

		if self.closed && !is_point {
			return (pos_offset, Some(neg_offset));
		}

		(pos_offset.combine_outline(&neg_offset, cap), None)
	}
}

#[cfg(test)]
mod tests {
	use super::{Cap, Join, ManipulatorGroup, Subpath};
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
	fn outline_with_single_point_segment() {
		let subpath = Subpath::new(
			vec![
				ManipulatorGroup {
					anchor: DVec2::new(20., 20.),
					out_handle: Some(DVec2::new(10., 90.)),
					in_handle: None,
					id: EmptyId,
				},
				ManipulatorGroup {
					anchor: DVec2::new(150., 40.),
					out_handle: None,
					in_handle: Some(DVec2::new(60., 40.)),
					id: EmptyId,
				},
				ManipulatorGroup {
					anchor: DVec2::new(150., 40.),
					out_handle: Some(DVec2::new(40., 120.)),
					in_handle: None,
					id: EmptyId,
				},
				ManipulatorGroup {
					anchor: DVec2::new(100., 100.),
					out_handle: None,
					in_handle: None,
					id: EmptyId,
				},
			],
			false,
		);

		let outline = subpath.outline(10., crate::Join::Round, crate::Cap::Round).0;
		assert!(outline.manipulator_groups.windows(2).all(|pair| !pair[0].anchor.abs_diff_eq(pair[1].anchor, MAX_ABSOLUTE_DIFFERENCE)));
		assert!(outline.closed());
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

		// Second manipulator group on the temporary subpath should be the reflected version of the last in the result
		assert_eq!(temporary.manipulator_groups[1].anchor, result.manipulator_groups[end - 1].anchor);
		assert_eq!(temporary.manipulator_groups[1].in_handle, result.manipulator_groups[end - 1].out_handle);
		assert_eq!(temporary.manipulator_groups[1].out_handle, result.manipulator_groups[end - 1].in_handle);

		// The first manipulator group in both should be the reflected versions of each other
		assert_eq!(temporary.manipulator_groups[0].anchor, result.manipulator_groups[0].anchor);
		assert_eq!(temporary.manipulator_groups[0].in_handle, result.manipulator_groups[0].out_handle);
		assert_eq!(temporary.manipulator_groups[0].out_handle, result.manipulator_groups[0].in_handle);
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

		assert!(compare_subpaths::<EmptyId>(&result1, &result2));
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
		let mut expected_subpath = subpath;
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
		assert!(compare_subpaths::<EmptyId>(&subpath, &result));
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

	#[test]
	fn outline_single_point_circle() {
		let ellipse: Subpath<EmptyId> = Subpath::new_ellipse(DVec2::new(0., 0.), DVec2::new(50., 50.)).reverse();
		let p = DVec2::new(25., 25.);

		let subpath: Subpath<EmptyId> = Subpath::from_anchors([p, p, p], false);
		let outline_open = subpath.outline(25., Join::Bevel, Cap::Round);
		assert_eq!(outline_open.0, ellipse);
		assert_eq!(outline_open.1, None);

		let subpath_closed: Subpath<EmptyId> = Subpath::from_anchors([p, p, p], true);
		let outline_closed = subpath_closed.outline(25., Join::Bevel, Cap::Round);
		assert_eq!(outline_closed.0, ellipse);
		assert_eq!(outline_closed.1, None);
	}

	#[test]
	fn outline_single_point_square() {
		let square: Subpath<EmptyId> = Subpath::from_anchors(
			[
				DVec2::new(25., 0.),
				DVec2::new(0., 0.),
				DVec2::new(0., 50.),
				DVec2::new(25., 50.),
				DVec2::new(50., 50.),
				DVec2::new(50., 0.),
			],
			true,
		);
		let p = DVec2::new(25., 25.);

		let subpath: Subpath<EmptyId> = Subpath::from_anchors([p, p, p], false);
		let outline_open = subpath.outline(25., Join::Bevel, Cap::Square);
		assert_eq!(outline_open.0, square);
		assert_eq!(outline_open.1, None);

		let subpath_closed: Subpath<EmptyId> = Subpath::from_anchors([p, p, p], true);
		let outline_closed = subpath_closed.outline(25., Join::Bevel, Cap::Square);
		assert_eq!(outline_closed.0, square);
		assert_eq!(outline_closed.1, None);
	}
}

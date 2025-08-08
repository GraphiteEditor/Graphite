use super::structs::Identifier;
use super::*;
use glam::{DAffine2, DVec2};

/// Helper function to ensure the index and t value pair is mapped within a maximum index value.
/// Allows for the point to be fetched without needing to handle an additional edge case.
/// - Ex. Via `subpath.iter().nth(index).evaluate(t);`
fn map_index_within_range(index: usize, t: f64, max_size: usize) -> (usize, f64) {
	if max_size > 0 && index == max_size && t == 0. { (index - 1, 1.) } else { (index, t) }
}

/// Functionality that transforms Subpaths, such as split, reduce, offset, etc.
impl<PointId: Identifier> Subpath<PointId> {
	/// Returns [ManipulatorGroup]s with a reversed winding order.
	fn reverse_manipulator_groups(manipulator_groups: &[ManipulatorGroup<PointId>]) -> Vec<ManipulatorGroup<PointId>> {
		manipulator_groups
			.iter()
			.rev()
			.map(|group| ManipulatorGroup {
				anchor: group.anchor,
				in_handle: group.out_handle,
				out_handle: group.in_handle,
				id: PointId::new(),
			})
			.collect::<Vec<ManipulatorGroup<PointId>>>()
	}

	/// Returns a [Subpath] with a reversed winding order.
	/// Note that a reversed closed subpath will start on the same manipulator group and simply wind the other direction
	pub fn reverse(&self) -> Subpath<PointId> {
		let mut reversed = Subpath::reverse_manipulator_groups(self.manipulator_groups());
		if self.closed {
			reversed.rotate_right(1);
		};
		Subpath {
			manipulator_groups: reversed,
			closed: self.closed,
		}
	}

	/// Apply a transformation to all of the [ManipulatorGroup]s in the [Subpath].
	pub fn apply_transform(&mut self, affine_transform: DAffine2) {
		for manipulator_group in &mut self.manipulator_groups {
			manipulator_group.apply_transform(affine_transform);
		}
	}

	/// Returns a subpath that results from rotating this subpath around the origin by the given angle (in radians).
	/// <iframe frameBorder="0" width="100%" height="325px" src="https://graphite.rs/libraries/bezier-rs#subpath/rotate/solo" title="Rotate Demo"></iframe>
	pub fn rotate(&self, angle: f64) -> Subpath<PointId> {
		let mut rotated_subpath = self.clone();

		let affine_transform: DAffine2 = DAffine2::from_angle(angle);
		rotated_subpath.apply_transform(affine_transform);

		rotated_subpath
	}

	/// Returns a subpath that results from rotating this subpath around the provided point by the given angle (in radians).
	pub fn rotate_about_point(&self, angle: f64, pivot: DVec2) -> Subpath<PointId> {
		// Translate before and after the rotation to account for the pivot
		let translate: DAffine2 = DAffine2::from_translation(pivot);
		let rotate: DAffine2 = DAffine2::from_angle(angle);
		let translate_inverse = translate.inverse();

		let mut rotated_subpath = self.clone();
		rotated_subpath.apply_transform(translate * rotate * translate_inverse);
		rotated_subpath
	}
}

// #[cfg(test)]
// mod tests {
// 	use super::{Cap, Join, ManipulatorGroup, Subpath};
// 	use crate::EmptyId;
// 	use crate::compare::{compare_points, compare_subpaths, compare_vec_of_points};
// 	use crate::consts::MAX_ABSOLUTE_DIFFERENCE;
// 	use crate::utils::{SubpathTValue, TValue};
// 	use glam::DVec2;

// 	fn set_up_open_subpath() -> Subpath<EmptyId> {
// 		let start = DVec2::new(20., 30.);
// 		let middle1 = DVec2::new(80., 90.);
// 		let middle2 = DVec2::new(100., 100.);
// 		let end = DVec2::new(60., 45.);

// 		let handle1 = DVec2::new(75., 85.);
// 		let handle2 = DVec2::new(40., 30.);
// 		let handle3 = DVec2::new(10., 10.);

// 		Subpath::new(
// 			vec![
// 				ManipulatorGroup {
// 					anchor: start,
// 					in_handle: None,
// 					out_handle: Some(handle1),
// 					id: EmptyId,
// 				},
// 				ManipulatorGroup {
// 					anchor: middle1,
// 					in_handle: None,
// 					out_handle: Some(handle2),
// 					id: EmptyId,
// 				},
// 				ManipulatorGroup {
// 					anchor: middle2,
// 					in_handle: None,
// 					out_handle: None,
// 					id: EmptyId,
// 				},
// 				ManipulatorGroup {
// 					anchor: end,
// 					in_handle: None,
// 					out_handle: Some(handle3),
// 					id: EmptyId,
// 				},
// 			],
// 			false,
// 		)
// 	}

// 	fn set_up_closed_subpath() -> Subpath<EmptyId> {
// 		let mut subpath = set_up_open_subpath();
// 		subpath.closed = true;
// 		subpath
// 	}

// 	#[test]
// 	fn outline_with_single_point_segment() {
// 		let subpath = Subpath::new(
// 			vec![
// 				ManipulatorGroup {
// 					anchor: DVec2::new(20., 20.),
// 					out_handle: Some(DVec2::new(10., 90.)),
// 					in_handle: None,
// 					id: EmptyId,
// 				},
// 				ManipulatorGroup {
// 					anchor: DVec2::new(150., 40.),
// 					out_handle: None,
// 					in_handle: Some(DVec2::new(60., 40.)),
// 					id: EmptyId,
// 				},
// 				ManipulatorGroup {
// 					anchor: DVec2::new(150., 40.),
// 					out_handle: Some(DVec2::new(40., 120.)),
// 					in_handle: None,
// 					id: EmptyId,
// 				},
// 				ManipulatorGroup {
// 					anchor: DVec2::new(100., 100.),
// 					out_handle: None,
// 					in_handle: None,
// 					id: EmptyId,
// 				},
// 			],
// 			false,
// 		);

// 		let outline = subpath.outline(10., crate::Join::Round, crate::Cap::Round).0;
// 		assert!(outline.manipulator_groups.windows(2).all(|pair| !pair[0].anchor.abs_diff_eq(pair[1].anchor, MAX_ABSOLUTE_DIFFERENCE)));
// 		assert!(outline.closed());
// 	}

// 	#[test]
// 	/// Even though the bézier here is not marked as a point, the offset and scaled version is.
// 	fn outline_with_point_offset() {
// 		let subpath = Subpath::new(
// 			vec![
// 				ManipulatorGroup {
// 					anchor: DVec2::new(1122.6253015182049, 610.9441551227939),
// 					out_handle: Some(DVec2::new(1122.6253015182049, 610.9445412168651)),
// 					in_handle: None,
// 					id: EmptyId,
// 				},
// 				ManipulatorGroup {
// 					anchor: DVec2::new(1122.6258671405062, 610.9453107605276),
// 					out_handle: None,
// 					in_handle: Some(DVec2::new(1122.6254904904154, 610.9449255479497)),
// 					id: EmptyId,
// 				},
// 				ManipulatorGroup {
// 					anchor: DVec2::new(0., 0.),
// 					out_handle: None,
// 					in_handle: None,
// 					id: EmptyId,
// 				},
// 			],
// 			false,
// 		);
// 		let outline = subpath.outline(4.4, crate::Join::Round, crate::Cap::Round).0;
// 		assert!(outline.manipulator_groups.windows(2).all(|pair| !pair[0].anchor.abs_diff_eq(pair[1].anchor, MAX_ABSOLUTE_DIFFERENCE)));
// 		assert!(outline.closed());
// 	}

// 	#[test]
// 	fn split_an_open_subpath() {
// 		let subpath = set_up_open_subpath();
// 		let location = subpath.evaluate(SubpathTValue::GlobalParametric(0.2));
// 		let split_pair = subpath.iter().next().unwrap().split(TValue::Parametric((0.2 * 3.) % 1.));
// 		let (first, second) = subpath.split(SubpathTValue::GlobalParametric(0.2));
// 		assert!(second.is_some());
// 		let second = second.unwrap();
// 		assert_eq!(first.manipulator_groups[1].anchor, location);
// 		assert_eq!(second.manipulator_groups[0].anchor, location);
// 		assert_eq!(split_pair[0], first.iter().last().unwrap());
// 		assert_eq!(split_pair[1], second.iter().next().unwrap());
// 	}

// 	#[test]
// 	fn split_at_start_of_an_open_subpath() {
// 		let subpath = set_up_open_subpath();
// 		let location = subpath.evaluate(SubpathTValue::GlobalParametric(0.));
// 		let split_pair = subpath.iter().next().unwrap().split(TValue::Parametric(0.));
// 		let (first, second) = subpath.split(SubpathTValue::GlobalParametric(0.));
// 		assert!(second.is_some());
// 		let second = second.unwrap();
// 		assert_eq!(
// 			first.manipulator_groups[0],
// 			ManipulatorGroup {
// 				anchor: location,
// 				in_handle: None,
// 				out_handle: None,
// 				id: EmptyId,
// 			}
// 		);
// 		assert_eq!(first.manipulator_groups.len(), 1);
// 		assert_eq!(second.manipulator_groups[0].anchor, location);
// 		assert_eq!(split_pair[1], second.iter().next().unwrap());
// 	}

// 	#[test]
// 	fn split_at_end_of_an_open_subpath() {
// 		let subpath = set_up_open_subpath();
// 		let location = subpath.evaluate(SubpathTValue::GlobalParametric(1.));
// 		let split_pair = subpath.iter().last().unwrap().split(TValue::Parametric(1.));
// 		let (first, second) = subpath.split(SubpathTValue::GlobalParametric(1.));
// 		assert!(second.is_some());
// 		let second = second.unwrap();
// 		assert_eq!(first.manipulator_groups[3].anchor, location);
// 		assert_eq!(split_pair[0], first.iter().last().unwrap());
// 		assert_eq!(
// 			second.manipulator_groups[0],
// 			ManipulatorGroup {
// 				anchor: location,
// 				in_handle: None,
// 				out_handle: None,
// 				id: EmptyId,
// 			}
// 		);
// 		assert_eq!(second.manipulator_groups.len(), 1);
// 	}

// 	#[test]
// 	fn split_a_closed_subpath() {
// 		let subpath = set_up_closed_subpath();
// 		let location = subpath.evaluate(SubpathTValue::GlobalParametric(0.2));
// 		let split_pair = subpath.iter().next().unwrap().split(TValue::Parametric((0.2 * 4.) % 1.));
// 		let (first, second) = subpath.split(SubpathTValue::GlobalParametric(0.2));
// 		assert!(second.is_none());
// 		assert_eq!(first.manipulator_groups[0].anchor, location);
// 		assert_eq!(first.manipulator_groups[5].anchor, location);
// 		assert_eq!(first.manipulator_groups.len(), 6);
// 		assert_eq!(split_pair[0], first.iter().last().unwrap());
// 		assert_eq!(split_pair[1], first.iter().next().unwrap());
// 	}

// 	#[test]
// 	fn split_at_start_of_a_closed_subpath() {
// 		let subpath = set_up_closed_subpath();
// 		let location = subpath.evaluate(SubpathTValue::GlobalParametric(0.));
// 		let (first, second) = subpath.split(SubpathTValue::GlobalParametric(0.));
// 		assert!(second.is_none());
// 		assert_eq!(first.manipulator_groups[0].anchor, location);
// 		assert_eq!(first.manipulator_groups[4].anchor, location);
// 		assert_eq!(subpath.manipulator_groups[0..], first.manipulator_groups[..4]);
// 		assert!(!first.closed);
// 		assert_eq!(first.iter().last().unwrap(), subpath.iter().last().unwrap());
// 		assert_eq!(first.iter().next().unwrap(), subpath.iter().next().unwrap());
// 	}

// 	#[test]
// 	fn split_at_end_of_a_closed_subpath() {
// 		let subpath = set_up_closed_subpath();
// 		let location = subpath.evaluate(SubpathTValue::GlobalParametric(1.));
// 		let (first, second) = subpath.split(SubpathTValue::GlobalParametric(1.));
// 		assert!(second.is_none());
// 		assert_eq!(first.manipulator_groups[0].anchor, location);
// 		assert_eq!(first.manipulator_groups[4].anchor, location);
// 		assert_eq!(subpath.manipulator_groups[0..], first.manipulator_groups[..4]);
// 		assert!(!first.closed);
// 		assert_eq!(first.iter().last().unwrap(), subpath.iter().last().unwrap());
// 		assert_eq!(first.iter().next().unwrap(), subpath.iter().next().unwrap());
// 	}

// 	#[test]
// 	fn reverse_an_open_subpath() {
// 		let subpath = set_up_open_subpath();
// 		let temporary = subpath.reverse();
// 		let result = temporary.reverse();
// 		let end = result.len();

// 		assert_eq!(temporary.manipulator_groups[0].anchor, result.manipulator_groups[end - 1].anchor);
// 		assert_eq!(temporary.manipulator_groups[0].out_handle, result.manipulator_groups[end - 1].in_handle);
// 		assert_eq!(subpath, result);
// 	}

// 	#[test]
// 	fn reverse_a_closed_subpath() {
// 		let subpath = set_up_closed_subpath();
// 		let temporary = subpath.reverse();
// 		let result = temporary.reverse();
// 		let end = result.len();

// 		// Second manipulator group on the temporary subpath should be the reflected version of the last in the result
// 		assert_eq!(temporary.manipulator_groups[1].anchor, result.manipulator_groups[end - 1].anchor);
// 		assert_eq!(temporary.manipulator_groups[1].in_handle, result.manipulator_groups[end - 1].out_handle);
// 		assert_eq!(temporary.manipulator_groups[1].out_handle, result.manipulator_groups[end - 1].in_handle);

// 		// The first manipulator group in both should be the reflected versions of each other
// 		assert_eq!(temporary.manipulator_groups[0].anchor, result.manipulator_groups[0].anchor);
// 		assert_eq!(temporary.manipulator_groups[0].in_handle, result.manipulator_groups[0].out_handle);
// 		assert_eq!(temporary.manipulator_groups[0].out_handle, result.manipulator_groups[0].in_handle);
// 		assert_eq!(subpath, result);
// 	}

// 	#[test]
// 	fn trim_an_open_subpath() {
// 		let subpath = set_up_open_subpath();
// 		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(0.2));
// 		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(0.8));
// 		let [_, trim_front] = subpath.iter().next().unwrap().split(TValue::Parametric((0.2 * 3.) % 1.));
// 		let [trim_back, _] = subpath.iter().last().unwrap().split(TValue::Parametric((0.8 * 3.) % 1.));
// 		let result = subpath.trim(SubpathTValue::GlobalParametric(0.2), SubpathTValue::GlobalParametric(0.8));

// 		assert_eq!(result.manipulator_groups[0].anchor, location_front);
// 		assert_eq!(result.manipulator_groups[3].anchor, location_back);
// 		assert_eq!(trim_front, result.iter().next().unwrap());
// 		assert_eq!(trim_back, result.iter().last().unwrap());
// 	}

// 	#[test]
// 	fn trim_within_a_bezier() {
// 		let subpath = set_up_open_subpath();
// 		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(0.1));
// 		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(0.2));
// 		let trimmed = subpath.iter().next().unwrap().trim(TValue::Parametric((0.1 * 3.) % 1.), TValue::Parametric((0.2 * 3.) % 1.));
// 		let result = subpath.trim(SubpathTValue::GlobalParametric(0.1), SubpathTValue::GlobalParametric(0.2));
// 		assert!(compare_points(result.manipulator_groups[0].anchor, location_front));
// 		assert!(compare_points(result.manipulator_groups[1].anchor, location_back));
// 		assert_eq!(trimmed, result.iter().next().unwrap());
// 		assert_eq!(result.len(), 2);
// 	}

// 	#[test]
// 	fn trim_first_segment_of_an_open_subpath() {
// 		let subpath = set_up_closed_subpath();
// 		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(0.));
// 		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(0.25));
// 		let trimmed = subpath.iter().next().unwrap().trim(TValue::Parametric(0.), TValue::Parametric(1.));
// 		let result = subpath.trim(SubpathTValue::GlobalParametric(0.), SubpathTValue::GlobalParametric(0.25));

// 		assert_eq!(result.manipulator_groups[0].anchor, location_front);
// 		assert_eq!(result.manipulator_groups[1].anchor, location_back);
// 		assert_eq!(trimmed, result.iter().next().unwrap());
// 	}

// 	#[test]
// 	fn trim_second_segment_of_an_open_subpath() {
// 		let subpath = set_up_closed_subpath();
// 		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(0.25));
// 		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(0.5));
// 		let trimmed = subpath.iter().nth(1).unwrap().trim(TValue::Parametric(0.), TValue::Parametric(1.));
// 		let result = subpath.trim(SubpathTValue::GlobalParametric(0.25), SubpathTValue::GlobalParametric(0.5));

// 		assert_eq!(result.manipulator_groups[0].anchor, location_front);
// 		assert_eq!(result.manipulator_groups[1].anchor, location_back);
// 		assert_eq!(trimmed, result.iter().next().unwrap());
// 	}

// 	#[test]
// 	fn trim_reverse_in_open_subpath() {
// 		let subpath = set_up_open_subpath();
// 		let result1 = subpath.trim(SubpathTValue::GlobalParametric(0.8), SubpathTValue::GlobalParametric(0.2));
// 		let result2 = subpath.trim(SubpathTValue::GlobalParametric(0.2), SubpathTValue::GlobalParametric(0.8));

// 		assert!(compare_subpaths::<EmptyId>(&result1, &result2));
// 	}

// 	#[test]
// 	fn trim_reverse_within_a_bezier() {
// 		let subpath = set_up_open_subpath();
// 		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(0.1));
// 		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(0.2));
// 		let trimmed = subpath.iter().next().unwrap().trim(TValue::Parametric((0.2 * 3.) % 1.), TValue::Parametric((0.1 * 3.) % 1.));
// 		let result = subpath.trim(SubpathTValue::GlobalParametric(0.2), SubpathTValue::GlobalParametric(0.1));

// 		assert!(compare_points(result.manipulator_groups[0].anchor, location_front));
// 		assert!(compare_points(result.manipulator_groups[1].anchor, location_back));
// 		assert!(compare_vec_of_points(
// 			trimmed.get_points().collect(),
// 			result.iter().next().unwrap().get_points().collect(),
// 			MAX_ABSOLUTE_DIFFERENCE
// 		));
// 		assert_eq!(result.len(), 2);
// 	}

// 	#[test]
// 	fn trim_a_duplicate_subpath() {
// 		let subpath = set_up_open_subpath();
// 		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(0.));
// 		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(1.));
// 		let result = subpath.trim(SubpathTValue::GlobalParametric(0.), SubpathTValue::GlobalParametric(1.));

// 		// Assume that resulting subpath would no longer have the any meaningless handles
// 		let mut expected_subpath = subpath;
// 		expected_subpath[3].out_handle = None;

// 		assert_eq!(result.manipulator_groups[0].anchor, location_front);
// 		assert!(compare_points(result.manipulator_groups[3].anchor, location_back));
// 		assert_eq!(expected_subpath, result);
// 	}

// 	#[test]
// 	fn trim_a_reversed_duplicate_subpath() {
// 		let subpath = set_up_open_subpath();
// 		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(0.));
// 		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(1.));
// 		let result = subpath.trim(SubpathTValue::GlobalParametric(1.), SubpathTValue::GlobalParametric(0.));

// 		assert_eq!(result.manipulator_groups[0].anchor, location_front);
// 		assert_eq!(result.manipulator_groups[3].anchor, location_back);
// 		assert!(compare_subpaths::<EmptyId>(&subpath, &result));
// 	}

// 	#[test]
// 	fn trim_to_end_of_subpath() {
// 		let subpath = set_up_open_subpath();
// 		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(0.8));
// 		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(1.));
// 		let trimmed = subpath.iter().last().unwrap().trim(TValue::Parametric((0.8 * 3.) % 1.), TValue::Parametric(1.));
// 		let result = subpath.trim(SubpathTValue::GlobalParametric(0.8), SubpathTValue::GlobalParametric(1.));

// 		assert_eq!(result.manipulator_groups[0].anchor, location_front);
// 		assert!(compare_points(result.manipulator_groups[1].anchor, location_back));
// 		assert_eq!(trimmed, result.iter().next().unwrap());
// 	}

// 	#[test]
// 	fn trim_reversed_to_end_of_subpath() {
// 		let subpath = set_up_open_subpath();
// 		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(0.));
// 		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(0.2));
// 		let trimmed = subpath.iter().next().unwrap().trim(TValue::Parametric((0.2 * 3.) % 1.), TValue::Parametric(0.));
// 		let result = subpath.trim(SubpathTValue::GlobalParametric(0.2), SubpathTValue::GlobalParametric(0.));

// 		assert!(compare_points(result.manipulator_groups[0].anchor, location_front));
// 		assert!(compare_points(result.manipulator_groups[1].anchor, location_back));
// 		assert!(compare_vec_of_points(
// 			trimmed.get_points().collect(),
// 			result.iter().next().unwrap().get_points().collect(),
// 			MAX_ABSOLUTE_DIFFERENCE
// 		));
// 	}

// 	#[test]
// 	fn trim_start_point() {
// 		let subpath = set_up_open_subpath();
// 		let location = subpath.evaluate(SubpathTValue::GlobalParametric(0.));
// 		let result = subpath.trim(SubpathTValue::GlobalParametric(0.), SubpathTValue::GlobalParametric(0.));

// 		assert!(compare_points(result.manipulator_groups[0].anchor, location));
// 		assert!(result.manipulator_groups[0].in_handle.is_none());
// 		assert!(result.manipulator_groups[0].out_handle.is_none());
// 		assert_eq!(result.len(), 1);
// 	}

// 	#[test]
// 	fn trim_middle_point() {
// 		let subpath = set_up_closed_subpath();
// 		let location = subpath.evaluate(SubpathTValue::GlobalParametric(0.25));
// 		let result = subpath.trim(SubpathTValue::GlobalParametric(0.25), SubpathTValue::GlobalParametric(0.25));

// 		assert!(compare_points(result.manipulator_groups[0].anchor, location));
// 		assert!(result.manipulator_groups[0].in_handle.is_none());
// 		assert!(result.manipulator_groups[0].out_handle.is_none());
// 		assert_eq!(result.len(), 1);
// 	}

// 	#[test]
// 	fn trim_a_closed_subpath() {
// 		let subpath = set_up_closed_subpath();
// 		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(0.2));
// 		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(0.8));
// 		let [_, trim_front] = subpath.iter().next().unwrap().split(TValue::Parametric((0.2 * 4.) % 1.));
// 		let [trim_back, _] = subpath.iter().last().unwrap().split(TValue::Parametric((0.8 * 4.) % 1.));
// 		let result = subpath.trim(SubpathTValue::GlobalParametric(0.2), SubpathTValue::GlobalParametric(0.8));

// 		assert_eq!(result.manipulator_groups[0].anchor, location_front);
// 		assert_eq!(result.manipulator_groups[4].anchor, location_back);
// 		assert_eq!(trim_front, result.iter().next().unwrap());
// 		assert_eq!(trim_back, result.iter().last().unwrap());
// 	}

// 	#[test]
// 	fn trim_to_end_of_closed_subpath() {
// 		let subpath = set_up_closed_subpath();
// 		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(0.8));
// 		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(1.));
// 		let trimmed = subpath.iter().last().unwrap().trim(TValue::Parametric((0.8 * 4.) % 1.), TValue::Parametric(1.));
// 		let result = subpath.trim(SubpathTValue::GlobalParametric(0.8), SubpathTValue::GlobalParametric(1.));

// 		assert_eq!(result.manipulator_groups[0].anchor, location_front);
// 		assert!(compare_points(result.manipulator_groups[1].anchor, location_back));
// 		assert_eq!(trimmed, result.iter().next().unwrap());
// 	}

// 	#[test]
// 	fn trim_across_break_in_a_closed_subpath() {
// 		let subpath = set_up_closed_subpath();
// 		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(0.8));
// 		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(0.2));
// 		let [_, trim_front] = subpath.iter().last().unwrap().split(TValue::Parametric((0.8 * 4.) % 1.));
// 		let [trim_back, _] = subpath.iter().next().unwrap().split(TValue::Parametric((0.2 * 4.) % 1.));
// 		let result = subpath.trim(SubpathTValue::GlobalParametric(0.8), SubpathTValue::GlobalParametric(0.2));

// 		assert_eq!(result.manipulator_groups[0].anchor, location_front);
// 		assert_eq!(result.manipulator_groups[2].anchor, location_back);
// 		assert_eq!(trim_front, result.iter().next().unwrap());
// 		assert_eq!(trim_back, result.iter().last().unwrap());
// 	}

// 	#[test]
// 	fn trim_across_break_in_a_closed_subpath_where_result_is_multiple_segments() {
// 		let subpath = set_up_closed_subpath();
// 		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(0.6));
// 		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(0.4));
// 		let [_, trim_front] = subpath.iter().nth(2).unwrap().split(TValue::Parametric((0.6 * 4.) % 1.));
// 		let [trim_back, _] = subpath.iter().nth(1).unwrap().split(TValue::Parametric((0.4 * 4.) % 1.));
// 		let result = subpath.trim(SubpathTValue::GlobalParametric(0.6), SubpathTValue::GlobalParametric(0.4));

// 		assert_eq!(result.manipulator_groups[0].anchor, location_front);
// 		assert_eq!(result.manipulator_groups[4].anchor, location_back);
// 		assert_eq!(trim_front, result.iter().next().unwrap());
// 		assert_eq!(trim_back, result.iter().last().unwrap());
// 	}

// 	#[test]
// 	fn trim_across_break_in_a_closed_subpath_where_ends_are_in_same_segment() {
// 		let subpath = set_up_closed_subpath();
// 		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(0.45));
// 		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(0.4));
// 		let [_, trim_front] = subpath.iter().nth(1).unwrap().split(TValue::Parametric((0.45 * 4.) % 1.));
// 		let [trim_back, _] = subpath.iter().nth(1).unwrap().split(TValue::Parametric((0.4 * 4.) % 1.));
// 		let result = subpath.trim(SubpathTValue::GlobalParametric(0.45), SubpathTValue::GlobalParametric(0.4));

// 		assert_eq!(result.manipulator_groups[0].anchor, location_front);
// 		assert_eq!(result.manipulator_groups[5].anchor, location_back);
// 		assert_eq!(trim_front, result.iter().next().unwrap());
// 		assert_eq!(trim_back, result.iter().last().unwrap());
// 	}

// 	#[test]
// 	fn trim_at_break_in_closed_subpath_where_end_is_0() {
// 		let subpath = set_up_closed_subpath();
// 		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(0.8));
// 		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(0.));
// 		let trimmed = subpath.iter().last().unwrap().trim(TValue::Parametric((0.8 * 4.) % 1.), TValue::Parametric(1.));
// 		let result = subpath.trim(SubpathTValue::GlobalParametric(0.8), SubpathTValue::GlobalParametric(0.));

// 		assert_eq!(result.manipulator_groups[0].anchor, location_front);
// 		assert_eq!(result.manipulator_groups[1].anchor, location_back);
// 		assert_eq!(trimmed, result.iter().next().unwrap());
// 	}

// 	#[test]
// 	fn trim_at_break_in_closed_subpath_where_start_is_1() {
// 		let subpath = set_up_closed_subpath();
// 		let location_front = subpath.evaluate(SubpathTValue::GlobalParametric(1.));
// 		let location_back = subpath.evaluate(SubpathTValue::GlobalParametric(0.2));
// 		let trimmed = subpath.iter().next().unwrap().trim(TValue::Parametric(0.), TValue::Parametric((0.2 * 4.) % 1.));
// 		let result = subpath.trim(SubpathTValue::GlobalParametric(1.), SubpathTValue::GlobalParametric(0.2));

// 		assert_eq!(result.manipulator_groups[0].anchor, location_front);
// 		assert_eq!(result.manipulator_groups[1].anchor, location_back);
// 		assert_eq!(trimmed, result.iter().next().unwrap());
// 	}

// 	#[test]
// 	fn trim_at_break_in_closed_subpath_from_1_to_0() {
// 		let subpath = set_up_closed_subpath();
// 		let location = subpath.evaluate(SubpathTValue::GlobalParametric(0.));
// 		let result = subpath.trim(SubpathTValue::GlobalParametric(1.), SubpathTValue::GlobalParametric(0.));

// 		assert_eq!(result.manipulator_groups[0].anchor, location);
// 		assert!(result.manipulator_groups[0].in_handle.is_none());
// 		assert!(result.manipulator_groups[0].out_handle.is_none());
// 		assert_eq!(result.manipulator_groups.len(), 1);
// 	}

// 	#[test]
// 	fn outline_single_point_circle() {
// 		let ellipse: Subpath<EmptyId> = Subpath::new_ellipse(DVec2::new(0., 0.), DVec2::new(50., 50.)).reverse();
// 		let p = DVec2::new(25., 25.);

// 		let subpath: Subpath<EmptyId> = Subpath::from_anchors([p, p, p], false);
// 		let outline_open = subpath.outline(25., Join::Bevel, Cap::Round);
// 		assert_eq!(outline_open.0, ellipse);
// 		assert_eq!(outline_open.1, None);

// 		let subpath_closed: Subpath<EmptyId> = Subpath::from_anchors([p, p, p], true);
// 		let outline_closed = subpath_closed.outline(25., Join::Bevel, Cap::Round);
// 		assert_eq!(outline_closed.0, ellipse);
// 		assert_eq!(outline_closed.1, None);
// 	}

// 	#[test]
// 	fn outline_single_point_square() {
// 		let square: Subpath<EmptyId> = Subpath::from_anchors(
// 			[
// 				DVec2::new(25., 0.),
// 				DVec2::new(0., 0.),
// 				DVec2::new(0., 50.),
// 				DVec2::new(25., 50.),
// 				DVec2::new(50., 50.),
// 				DVec2::new(50., 0.),
// 			],
// 			true,
// 		);
// 		let p = DVec2::new(25., 25.);

// 		let subpath: Subpath<EmptyId> = Subpath::from_anchors([p, p, p], false);
// 		let outline_open = subpath.outline(25., Join::Bevel, Cap::Square);
// 		assert_eq!(outline_open.0, square);
// 		assert_eq!(outline_open.1, None);

// 		let subpath_closed: Subpath<EmptyId> = Subpath::from_anchors([p, p, p], true);
// 		let outline_closed = subpath_closed.outline(25., Join::Bevel, Cap::Square);
// 		assert_eq!(outline_closed.0, square);
// 		assert_eq!(outline_closed.1, None);
// 	}
// }

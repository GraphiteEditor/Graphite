use super::*;
use crate::consts::MAX_ABSOLUTE_DIFFERENCE;
use crate::utils::f64_compare;
use crate::{SubpathTValue, TValue};

impl<ManipulatorGroupId: crate::Identifier> Subpath<ManipulatorGroupId> {
	/// Inserts a `ManipulatorGroup` at a certain point along the subpath based on the parametric `t`-value provided.
	/// Expects `t` to be within the inclusive range `[0, 1]`.
	pub fn insert(&mut self, t: SubpathTValue) {
		let (segment_index, t) = self.t_value_to_parametric(t);

		if f64_compare(t, 0., MAX_ABSOLUTE_DIFFERENCE) || f64_compare(t, 1., MAX_ABSOLUTE_DIFFERENCE) {
			return;
		}

		// The only case where `curve` would be `None` is if the provided argument was 1
		// But the above if case would catch that, since `target_curve_t` would be 0.
		let curve = self.iter().nth(segment_index).unwrap();

		let [first, second] = curve.split(TValue::Parametric(t));
		let new_group = ManipulatorGroup {
			anchor: first.end(),
			in_handle: first.handle_end(),
			out_handle: second.handle_start(),
			id: ManipulatorGroupId::new(),
		};
		let number_of_groups = self.manipulator_groups.len() + 1;
		self.manipulator_groups.insert((segment_index) + 1, new_group);
		self.manipulator_groups[segment_index % number_of_groups].out_handle = first.handle_start();
		self.manipulator_groups[(segment_index + 2) % number_of_groups].in_handle = second.handle_end();
	}
}

#[cfg(test)]
mod tests {
	use crate::utils::SubpathTValue;

	use super::*;
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
	fn insert_in_first_segment_of_open_subpath() {
		let mut subpath = set_up_open_subpath();
		let location = subpath.evaluate(SubpathTValue::GlobalParametric(0.2));
		let split_pair = subpath.iter().next().unwrap().split(TValue::Parametric((0.2 * 3.) % 1.));
		subpath.insert(SubpathTValue::GlobalParametric(0.2));
		assert_eq!(subpath.manipulator_groups[1].anchor, location);
		assert_eq!(split_pair[0], subpath.iter().next().unwrap());
		assert_eq!(split_pair[1], subpath.iter().nth(1).unwrap());
	}

	#[test]
	fn insert_in_last_segment_of_open_subpath() {
		let mut subpath = set_up_open_subpath();
		let location = subpath.evaluate(SubpathTValue::GlobalParametric(0.9));
		let split_pair = subpath.iter().nth(2).unwrap().split(TValue::Parametric((0.9 * 3.) % 1.));
		subpath.insert(SubpathTValue::GlobalParametric(0.9));
		assert_eq!(subpath.manipulator_groups[3].anchor, location);
		assert_eq!(split_pair[0], subpath.iter().nth(2).unwrap());
		assert_eq!(split_pair[1], subpath.iter().nth(3).unwrap());
	}

	#[test]
	fn insert_at_exisiting_manipulator_group_of_open_subpath() {
		// This will do nothing to the subpath
		let mut subpath = set_up_open_subpath();
		let location = subpath.evaluate(SubpathTValue::GlobalParametric(0.75));
		subpath.insert(SubpathTValue::GlobalParametric(0.75));
		assert_eq!(subpath.manipulator_groups[3].anchor, location);
		assert_eq!(subpath.manipulator_groups.len(), 5);
		assert_eq!(subpath.len_segments(), 4);
	}

	#[test]
	fn insert_at_last_segment_of_closed_subpath() {
		let mut subpath = set_up_closed_subpath();
		let location = subpath.evaluate(SubpathTValue::GlobalParametric(0.9));
		let split_pair = subpath.iter().nth(3).unwrap().split(TValue::Parametric((0.9 * 4.) % 1.));
		subpath.insert(SubpathTValue::GlobalParametric(0.9));
		assert_eq!(subpath.manipulator_groups[4].anchor, location);
		assert_eq!(split_pair[0], subpath.iter().nth(3).unwrap());
		assert_eq!(split_pair[1], subpath.iter().nth(4).unwrap());
		assert!(subpath.closed);
	}

	#[test]
	fn insert_at_last_manipulator_group_of_closed_subpath() {
		// This will do nothing to the subpath
		let mut subpath = set_up_closed_subpath();
		let location = subpath.evaluate(SubpathTValue::GlobalParametric(1.));
		subpath.insert(SubpathTValue::GlobalParametric(1.));
		assert_eq!(subpath.manipulator_groups[0].anchor, location);
		assert_eq!(subpath.manipulator_groups.len(), 4);
		assert!(subpath.closed);
	}
}

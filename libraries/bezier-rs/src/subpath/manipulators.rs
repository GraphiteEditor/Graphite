use super::*;
use crate::consts::MAX_ABSOLUTE_DIFFERENCE;
use crate::utils::f64_compare;
use crate::{SubpathTValue, TValue};

impl<ManipulatorGroupId: crate::Identifier> Subpath<ManipulatorGroupId> {
	/// Get whether the subpath is closed.
	pub fn closed(&self) -> bool {
		self.closed
	}

	/// Set whether the subpath is closed.
	pub fn set_closed(&mut self, new_closed: bool) {
		self.closed = new_closed;
	}

	/// Access a [ManipulatorGroup] from a ManipulatorGroupId.
	pub fn manipulator_from_id(&self, id: ManipulatorGroupId) -> Option<&ManipulatorGroup<ManipulatorGroupId>> {
		self.manipulator_groups.iter().find(|manipulator_group| manipulator_group.id == id)
	}

	/// Access a mutable [ManipulatorGroup] from a ManipulatorGroupId.
	pub fn manipulator_mut_from_id(&mut self, id: ManipulatorGroupId) -> Option<&mut ManipulatorGroup<ManipulatorGroupId>> {
		self.manipulator_groups.iter_mut().find(|manipulator_group| manipulator_group.id == id)
	}

	/// Access the index of a [ManipulatorGroup] from a ManipulatorGroupId.
	pub fn manipulator_index_from_id(&self, id: ManipulatorGroupId) -> Option<usize> {
		self.manipulator_groups.iter().position(|manipulator_group| manipulator_group.id == id)
	}

	/// Insert a manipulator group at an index.
	pub fn insert_manipulator_group(&mut self, index: usize, group: ManipulatorGroup<ManipulatorGroupId>) {
		assert!(group.is_finite(), "Inserting non finite manipulator group");
		self.manipulator_groups.insert(index, group)
	}

	/// Push a manipulator group to the end.
	pub fn push_manipulator_group(&mut self, group: ManipulatorGroup<ManipulatorGroupId>) {
		assert!(group.is_finite(), "Pushing non finite manipulator group");
		self.manipulator_groups.push(group)
	}

	/// Get a mutable reference to the last manipulator
	pub fn last_manipulator_group_mut(&mut self) -> Option<&mut ManipulatorGroup<ManipulatorGroupId>> {
		self.manipulator_groups.last_mut()
	}

	/// Remove a manipulator group at an index.
	pub fn remove_manipulator_group(&mut self, index: usize) -> ManipulatorGroup<ManipulatorGroupId> {
		self.manipulator_groups.remove(index)
	}

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

	/// Append a [Bezier] to the end of a subpath from a vector of [Bezier].
	/// The `append_type` parameter determines how the function behaves when the subpath's last anchor is not equal to the Bezier's start point.
	/// - `IgnoreStart`: drops the bezier's start point in favor of the subpath's last anchor
	/// - `SmoothJoin(f64)`: joins the subpath's endpoint with the bezier's start with a another Bezier segment that is continuous up to the second derivative
	///   if the difference between the subpath's end point and Bezier's start point exceeds the wrapped integer value.
	/// This function assumes that the position of the [Bezier]'s starting point is equal to that of the Subpath's last manipulator group.
	pub fn append_bezier(&mut self, bezier: &Bezier, append_type: AppendType) {
		if self.manipulator_groups.is_empty() {
			self.manipulator_groups = vec![ManipulatorGroup {
				anchor: bezier.start(),
				in_handle: None,
				out_handle: None,
				id: ManipulatorGroupId::new(),
			}];
		}
		let mut last_index = self.manipulator_groups.len() - 1;
		let last_anchor = self.manipulator_groups[last_index].anchor;

		if let AppendType::SmoothJoin(max_absolute_difference) = append_type {
			// If the provided Bezier does not start at a location similar to the end of the Subpath,
			// add an additional manipulator group to represent a smooth join with a new bezier in between
			if !last_anchor.abs_diff_eq(bezier.start(), max_absolute_difference) {
				let last_bezier = if self.manipulator_groups.len() > 1 {
					self.manipulator_groups[last_index - 1].to_bezier(&self.manipulator_groups[last_index])
				} else {
					Bezier::from_linear_dvec2(last_anchor, last_anchor)
				};
				let join_bezier = last_bezier.join(bezier);
				self.append_bezier(&join_bezier, AppendType::IgnoreStart);
				last_index = self.manipulator_groups.len() - 1;
			}
		}
		self.manipulator_groups[last_index].out_handle = bezier.handle_start();
		self.manipulator_groups.push(ManipulatorGroup {
			anchor: bezier.end(),
			in_handle: bezier.handle_end(),
			out_handle: None,
			id: ManipulatorGroupId::new(),
		});
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

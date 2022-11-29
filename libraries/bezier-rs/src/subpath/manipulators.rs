use super::*;
use crate::ComputeType;

impl Subpath {
	/// Calculate the point on the subpath based on the parametric `t`-value provided.
	/// Expects `t` to be within the inclusive range `[0, 1]`.
	pub fn add_manipulator_group(&mut self, t: ComputeType) {
		match t {
			ComputeType::Parametric(t) => {
				assert!((0.0..=1.).contains(&t));

				let number_of_curves = self.len_segments() as f64;
				let scaled_t = t * number_of_curves;

				let target_curve_index = scaled_t.floor() as i32;
				let target_curve_t = scaled_t % 1.;

				if target_curve_t == 0. || target_curve_t == 1. {
					return;
				}

				// The only case where `curve` would be `None` is if the provided argument was 1
				// But the above if case would catch that, since `target_curve_t` would be 0.
				let curve = self.iter().nth(target_curve_index as usize).unwrap();

				let [first, second] = curve.split(target_curve_t);
				let new_group = ManipulatorGroup {
					anchor: first.end(),
					in_handle: first.handle_end(),
					out_handle: second.handle_start(),
				};
				let number_of_groups = self.manipulator_groups.len() + 1;
				self.manipulator_groups.insert((target_curve_index as usize) + 1, new_group);
				self.manipulator_groups[(target_curve_index as usize) % number_of_groups].out_handle = first.handle_start();
				self.manipulator_groups[((target_curve_index as usize) + 2) % number_of_groups].in_handle = second.handle_end();
			}
			// TODO: change this implementation to Euclidean compute
			ComputeType::Euclidean(_t) => {}
			ComputeType::EuclideanWithinError { t: _, epsilon: _ } => todo!(),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use glam::DVec2;

	#[test]
	fn add_manipulator_group() {
		let start = DVec2::new(20., 30.);
		let middle = DVec2::new(80., 90.);
		let end = DVec2::new(60., 45.);
		let handle1 = DVec2::new(75., 85.);
		let handle2 = DVec2::new(40., 30.);
		let handle3 = DVec2::new(10., 10.);

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

		let location = subpath.evaluate(ComputeType::Parametric(0.2));
		let split_pair = subpath.iter().next().unwrap().split((0.2 * 2.) % 1.);
		subpath.add_manipulator_group(ComputeType::Parametric(0.2));
		assert_eq!(subpath.manipulator_groups[1].anchor, location);
		assert_eq!(split_pair[0], subpath.iter().next().unwrap());
		assert_eq!(split_pair[1], subpath.iter().nth(1).unwrap());

		let location2 = subpath.evaluate(ComputeType::Parametric(0.9));
		let split_pair2 = subpath.iter().nth(2).unwrap().split((0.9 * 3.) % 1.);
		subpath.add_manipulator_group(ComputeType::Parametric(0.9));
		assert_eq!(subpath.manipulator_groups[3].anchor, location2);
		assert_eq!(split_pair2[0], subpath.iter().nth(2).unwrap());
		assert_eq!(split_pair2[1], subpath.iter().nth(3).unwrap());

		let location3 = subpath.evaluate(ComputeType::Parametric(0.75));
		subpath.add_manipulator_group(ComputeType::Parametric(0.75));
		assert_eq!(subpath.manipulator_groups[3].anchor, location3);
		assert_eq!(subpath.manipulator_groups.len(), 5);
		assert_eq!(subpath.len_segments(), 4);

		subpath.closed = true;

		let location4 = subpath.evaluate(ComputeType::Parametric(0.9));
		let split_pair4 = subpath.iter().nth(4).unwrap().split((0.9 * 5.) % 1.);
		subpath.add_manipulator_group(ComputeType::Parametric(0.9));
		assert_eq!(subpath.manipulator_groups[5].anchor, location4);
		assert_eq!(split_pair4[0], subpath.iter().nth(4).unwrap());
		assert_eq!(split_pair4[1], subpath.iter().nth(5).unwrap());

		let location5 = subpath.evaluate(ComputeType::Parametric(1.));
		let prev_len = subpath.manipulator_groups.len();
		subpath.add_manipulator_group(ComputeType::Parametric(1.));
		assert_eq!(subpath.manipulator_groups[0].anchor, location5);
		assert_eq!(subpath.manipulator_groups.len(), prev_len);
	}
}

use super::*;
use crate::ComputeType;

impl Subpath {
	/// Calculate the point on the subpath based on the parametric `t`-value provided.
	/// Expects `t` to be within the inclusive range `[0, 1]`.
	pub fn add_manipulator_group(&self, t: ComputeType) {
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
				let Some(curve) = self.iter().nth(target_curve_index as usize);

				let [first, second] = curve.split(target_curve_t);
				let new_group = ManipulatorGroup {
					anchor: first.end,
					in_handle: first.handle_end,
					out_handle: second.handle_start,
				};
				self.manipulator_groups.insert(target_curve_index + 1, new_group);
			}
			// TODO: change this implementation to Euclidean compute
			ComputeType::Euclidean(_t) => self.iter().next().unwrap().evaluate(ComputeType::Parametric(0.)),
			ComputeType::EuclideanWithinError { t: _, epsilon: _ } => todo!(),
		}
	}
}

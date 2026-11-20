use glam::DVec2;

/// An origin and a set of axis directions in a work plane, from which every directional behavior takes its directions.
///
/// A flat drawing has two axes at right angles, and an isometric one has three.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
	pub origin: DVec2,
	axes: Vec<DVec2>,
}

impl Default for Frame {
	fn default() -> Self {
		Self {
			origin: DVec2::ZERO,
			axes: vec![DVec2::X, DVec2::Y],
		}
	}
}

impl Frame {
	/// Builds a frame from its axis directions, which are normalized. Returns `None` unless there are at least two axes and no two are parallel.
	pub fn new(origin: DVec2, axes: impl IntoIterator<Item = DVec2>) -> Option<Self> {
		let axes = axes.into_iter().map(|axis| axis.try_normalize()).collect::<Option<Vec<_>>>()?;

		let distinct = axes.iter().enumerate().all(|(index, axis)| axes[..index].iter().all(|earlier| axis.perp_dot(*earlier).abs() > 1e-9));
		(axes.len() >= 2 && distinct).then_some(Self { origin, axes })
	}

	/// The unit directions of the axes.
	pub fn axes(&self) -> &[DVec2] {
		&self.axes
	}

	/// The index of the axis most nearly parallel to a direction, pointing either way along it.
	pub fn nearest_axis(&self, direction: DVec2) -> usize {
		let alignment = |axis: &DVec2| axis.dot(direction).abs();
		(0..self.axes.len()).max_by(|&a, &b| alignment(&self.axes[a]).total_cmp(&alignment(&self.axes[b]))).unwrap_or_default()
	}

	/// The direction closest to `direction` among the multiples of `step` radians measured either way from the first axis.
	/// A step that is not positive and finite leaves the direction as it is.
	pub fn nearest_angle_step(&self, direction: DVec2, step: f64) -> DVec2 {
		if !step.is_finite() || step <= 0. {
			return direction.normalize_or(self.axes[0]);
		}

		let first_axis = self.axes[0];
		let angle_from_first_axis = first_axis.perp_dot(direction).atan2(first_axis.dot(direction));
		let steps = (angle_from_first_axis / step).round();
		DVec2::from_angle(first_axis.to_angle() + steps * step)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::f64::consts::PI;

	#[test]
	fn parallel_or_missing_axes_are_rejected() {
		assert!(Frame::new(DVec2::ZERO, [DVec2::X]).is_none());
		assert!(Frame::new(DVec2::ZERO, [DVec2::X, DVec2::NEG_X]).is_none());
		assert!(Frame::new(DVec2::ZERO, [DVec2::X, DVec2::ZERO]).is_none());
		assert!(Frame::new(DVec2::ZERO, [DVec2::X * 5., DVec2::Y]).is_some_and(|frame| frame.axes()[0] == DVec2::X));
	}

	#[test]
	fn the_nearest_axis_ignores_which_way_the_direction_points() {
		let isometric = Frame::new(DVec2::ZERO, [DVec2::from_angle(PI / 6.), DVec2::from_angle(PI / 2.), DVec2::from_angle(5. * PI / 6.)]).unwrap();

		assert_eq!(isometric.nearest_axis(DVec2::new(1., 0.4)), 0);
		assert_eq!(isometric.nearest_axis(DVec2::new(-1., -0.4)), 0);
		assert_eq!(isometric.nearest_axis(DVec2::new(0.1, -1.)), 1);
		assert_eq!(isometric.nearest_axis(DVec2::new(-1., 0.7)), 2);
	}

	#[test]
	fn angle_steps_are_measured_from_the_first_axis() {
		let step = PI / 12.;

		let flat = Frame::default();
		assert!(flat.nearest_angle_step(DVec2::from_angle(0.3), step).abs_diff_eq(DVec2::from_angle(PI / 12.), 1e-12));

		let tilted = Frame::new(DVec2::ZERO, [DVec2::from_angle(0.1), DVec2::from_angle(0.1 + PI / 2.)]).unwrap();
		assert!(tilted.nearest_angle_step(DVec2::from_angle(0.3), step).abs_diff_eq(DVec2::from_angle(0.1 + PI / 12.), 1e-12));

		// A step that doesn't divide a full turn is still counted from the first axis across the opposite direction
		let uneven_step = 25_f64.to_radians();
		let just_past_opposite = DVec2::from_angle(180.1_f64.to_radians());
		assert!(tilted.nearest_angle_step(just_past_opposite, uneven_step).abs_diff_eq(DVec2::from_angle(0.1 + 7. * uneven_step), 1e-12));

		// A step that divides nothing leaves the direction alone
		for unusable in [0., -1., f64::NAN, f64::INFINITY] {
			assert!(flat.nearest_angle_step(DVec2::from_angle(0.3) * 5., unusable).abs_diff_eq(DVec2::from_angle(0.3), 1e-12));
		}
	}
}

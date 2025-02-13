use glam::{DAffine2, DVec2};

use crate::consts::{COMPASS_ROSE_ANGLE_WIDTH, COMPASS_ROSE_HOVER_RING_DIAMETER, COMPASS_ROSE_RING_INNER_DIAMETER};
use std::f64::consts::FRAC_PI_2;

#[derive(Clone, Debug)]
pub struct CompassRose {
	/// Transform to get from normalized pivot to viewspace
	transform_from_normalized: DAffine2,
}

impl CompassRose {
	pub fn get_compass_position(&self) -> DVec2 {
		self.transform_from_normalized.transform_point2(DVec2::splat(0.5))
	}

	/// Answers if the pointer is currently positioned over the pivot.
	pub fn compass_rose_state(&self, mouse: DVec2, angle: f64) -> CompassRoseState {
		let compass_center = self.get_compass_position();

		let compass_distance_squared = mouse.distance_squared(compass_center);

		if (COMPASS_ROSE_RING_INNER_DIAMETER / 2.).powi(2) < compass_distance_squared && compass_distance_squared < (COMPASS_ROSE_HOVER_RING_DIAMETER / 2.).powi(2) {
			let angle = (mouse - compass_center).angle_to(DVec2::from_angle(angle)).abs();
			let resolved_angle = (FRAC_PI_2 - angle).abs();
			let width = COMPASS_ROSE_ANGLE_WIDTH.to_radians();

			if resolved_angle < width {
				CompassRoseState::AxisY
			} else if resolved_angle > (FRAC_PI_2 - width) {
				CompassRoseState::AxisX
			} else {
				CompassRoseState::Ring
			}
		} else {
			CompassRoseState::None
		}
	}
}

#[derive(Clone, Debug, PartialEq)]
pub enum CompassRoseState {
	Ring,
	AxisX,
	AxisY,
	None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Axis {
	#[default]
	None,
	X,
	Y,
}

impl Axis {
	pub fn is_constraint(&self) -> bool {
		matches!(self, Self::X | Self::Y)
	}
}

impl CompassRoseState {
	pub fn can_grab(&self) -> bool {
		matches!(self, Self::Ring | Self::AxisX | Self::AxisY)
	}

	pub fn is_ring(&self) -> bool {
		matches!(self, Self::Ring)
	}

	pub fn axis_type(&self) -> Option<Axis> {
		match self {
			CompassRoseState::AxisX => Some(Axis::X),
			CompassRoseState::AxisY => Some(Axis::Y),
			CompassRoseState::Ring => Some(Axis::None),
			_ => None,
		}
	}
}

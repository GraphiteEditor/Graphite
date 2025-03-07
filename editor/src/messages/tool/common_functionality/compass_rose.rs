use crate::consts::{COMPASS_ROSE_ARROW_CLICK_TARGET_ANGLE, COMPASS_ROSE_HOVER_RING_DIAMETER, COMPASS_ROSE_RING_INNER_DIAMETER};
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::DocumentMessageHandler;

use glam::{DAffine2, DVec2};
use std::f64::consts::FRAC_PI_2;

#[derive(Clone, Default, Debug)]
pub struct CompassRose {
	compass_center: DVec2,
}

impl CompassRose {
	fn get_layer_pivot_transform(layer: LayerNodeIdentifier, document: &DocumentMessageHandler) -> DAffine2 {
		let [min, max] = document.metadata().nonzero_bounding_box(layer);

		let bounds_transform = DAffine2::from_translation(min) * DAffine2::from_scale(max - min);
		let layer_transform = document.metadata().transform_to_viewport(layer);
		layer_transform * bounds_transform
	}
	pub fn refresh_position(&mut self, document: &DocumentMessageHandler) {
		let selected_nodes = document.network_interface.selected_nodes();
		let mut layers = selected_nodes.selected_visible_and_unlocked_layers(&document.network_interface);

		let Some(first) = layers.next() else { return };
		let count = layers.count() + 1;
		let transform = if count == 1 {
			Self::get_layer_pivot_transform(first, document)
		} else {
			let [min, max] = document.selected_visible_and_unlock_layers_bounding_box_viewport().unwrap_or([DVec2::ZERO, DVec2::ONE]);
			DAffine2::from_translation(min) * DAffine2::from_scale(max - min)
		};

		self.compass_center = transform.transform_point2(DVec2::splat(0.5));
	}

	pub fn compass_rose_position(&self) -> DVec2 {
		self.compass_center
	}

	pub fn compass_rose_state(&self, mouse: DVec2, angle: f64) -> CompassRoseState {
		const COMPASS_ROSE_RING_INNER_RADIUS_SQUARED: f64 = (COMPASS_ROSE_RING_INNER_DIAMETER / 2.) * (COMPASS_ROSE_RING_INNER_DIAMETER / 2.);
		const COMPASS_ROSE_HOVER_RING_RADIUS_SQUARED: f64 = (COMPASS_ROSE_HOVER_RING_DIAMETER / 2.) * (COMPASS_ROSE_HOVER_RING_DIAMETER / 2.);

		let compass_distance_squared = mouse.distance_squared(self.compass_center);

		if !(COMPASS_ROSE_RING_INNER_RADIUS_SQUARED..COMPASS_ROSE_HOVER_RING_RADIUS_SQUARED).contains(&compass_distance_squared) {
			return CompassRoseState::None;
		}

		let angle = (mouse - self.compass_center).angle_to(DVec2::from_angle(angle)).abs();
		let resolved_angle = (FRAC_PI_2 - angle).abs();
		let angular_width = COMPASS_ROSE_ARROW_CLICK_TARGET_ANGLE.to_radians();

		if resolved_angle < angular_width {
			CompassRoseState::AxisY
		} else if resolved_angle > (FRAC_PI_2 - angular_width) {
			CompassRoseState::AxisX
		} else {
			CompassRoseState::Ring
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

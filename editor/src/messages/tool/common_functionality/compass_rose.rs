use crate::consts::{COMPASS_ROSE_ARROW_CLICK_TARGET_ANGLE, COMPASS_ROSE_HOVER_RING_DIAMETER, COMPASS_ROSE_RING_INNER_DIAMETER};
use crate::messages::prelude::DocumentMessageHandler;
use glam::{DAffine2, DVec2};
use std::f64::consts::FRAC_PI_2;

#[derive(Clone, Default, Debug)]
pub struct CompassRose {
	compass_center: DVec2,
}

impl CompassRose {
	pub fn refresh_position(&mut self, document: &DocumentMessageHandler) {
		let selected = document.network_interface.selected_nodes();

		if !selected.has_selected_nodes() {
			return;
		}

		let transform = selected
			.selected_visible_and_unlocked_layers(&document.network_interface)
			.find(|layer| !document.network_interface.is_artboard(&layer.to_node(), &[]))
			.map(|layer| document.metadata().transform_to_viewport_with_first_transform_node_if_group(layer, &document.network_interface))
			.unwrap_or_default();

		let bounds = document
			.network_interface
			.selected_nodes()
			.selected_visible_and_unlocked_layers(&document.network_interface)
			.filter_map(|layer| {
				document
					.metadata()
					.bounding_box_with_transform(layer, transform.inverse() * document.metadata().transform_to_viewport(layer))
			})
			.reduce(graphene_std::renderer::Quad::combine_bounds);

		let [min, max] = bounds.unwrap_or([DVec2::ZERO, DVec2::ONE]);
		let transform = transform * DAffine2::from_translation(min) * DAffine2::from_scale(max - min);

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

impl From<Axis> for DVec2 {
	fn from(value: Axis) -> Self {
		match value {
			Axis::X => DVec2::X,
			Axis::Y => DVec2::Y,
			Axis::None => DVec2::ZERO,
		}
	}
}

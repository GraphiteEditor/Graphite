//! Handler for the pivot overlay visible on the selected layer(s) whilst using the Select tool which controls the center of rotation/scale and origin of the layer.

use super::graph_modification_utils;
use crate::consts::{COMPASS_ROSE_ARROW_SIZE, COMPASS_ROSE_HOVER_RING_DIAMETER, COMPASS_ROSE_MAIN_RING_DIAMETER, COMPASS_ROSE_PIVOT_DIAMETER, COMPASS_ROSE_RING_INNER_DIAMETER};
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::*;

use glam::{DAffine2, DVec2};
use std::collections::VecDeque;
use std::f64::consts::FRAC_PI_2;

#[derive(Clone, Debug, PartialEq)]
pub enum CompassRoseState {
	Pivot,
	HoverRing,
	MainRing,
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
		matches!(self, Self::HoverRing | Self::AxisX | Self::AxisY)
	}

	pub fn is_pivot(&self) -> bool {
		matches!(self, Self::Pivot)
	}

	pub fn is_ring(&self) -> bool {
		matches!(self, Self::HoverRing | Self::MainRing)
	}

	pub fn axis_type(&self) -> Option<Axis> {
		match self {
			CompassRoseState::AxisX => Some(Axis::X),
			CompassRoseState::AxisY => Some(Axis::Y),
			CompassRoseState::HoverRing => Some(Axis::None),
			_ => None,
		}
	}
}

#[derive(Clone, Debug)]
pub struct Pivot {
	/// Pivot between (0,0) and (1,1)
	normalized_pivot: DVec2,
	/// Transform to get from normalized pivot to viewspace
	transform_from_normalized: DAffine2,
	/// The viewspace pivot position (if applicable)
	pivot: Option<DVec2>,
	/// The old pivot position in the GUI, used to reduce refreshes of the document bar
	old_pivot_position: PivotPosition,
}

impl Default for Pivot {
	fn default() -> Self {
		Self {
			normalized_pivot: DVec2::splat(0.5),
			transform_from_normalized: Default::default(),
			pivot: Default::default(),
			old_pivot_position: PivotPosition::Center,
		}
	}
}

impl Pivot {
	/// Calculates the transform that gets from normalized pivot to viewspace.
	fn get_layer_pivot_transform(layer: LayerNodeIdentifier, document: &DocumentMessageHandler) -> DAffine2 {
		let [min, max] = document.metadata().nonzero_bounding_box(layer);

		let bounds_transform = DAffine2::from_translation(min) * DAffine2::from_scale(max - min);
		let layer_transform = document.metadata().transform_to_viewport(layer);
		layer_transform * bounds_transform
	}

	pub fn get_position(&self) -> Option<DVec2> {
		self.pivot
	}

	/// Recomputes the pivot position and transform.
	fn recalculate_pivot(&mut self, document: &DocumentMessageHandler) {
		let selected_nodes = document.network_interface.selected_nodes(&[]).unwrap();
		let mut layers = selected_nodes.selected_visible_and_unlocked_layers(&document.network_interface);
		let Some(first) = layers.next() else {
			// If no layers are selected then we revert things back to default
			self.normalized_pivot = DVec2::splat(0.5);
			self.pivot = None;
			return;
		};

		// Add one because the first item is consumed above.
		let selected_layers_count = layers.count() + 1;

		// If just one layer is selected we can use its inner transform (as it accounts for rotation)
		if selected_layers_count == 1 {
			let normalized_pivot = graph_modification_utils::get_pivot(first, &document.network_interface).unwrap_or(DVec2::splat(0.5));
			self.normalized_pivot = normalized_pivot;
			self.transform_from_normalized = Self::get_layer_pivot_transform(first, document);
			self.pivot = Some(self.transform_from_normalized.transform_point2(normalized_pivot));
		} else {
			// If more than one layer is selected we use the AABB with the mean of the pivots
			let xy_summation = document
				.network_interface
				.selected_nodes(&[])
				.unwrap()
				.selected_visible_and_unlocked_layers(&document.network_interface)
				.map(|layer| graph_modification_utils::get_viewport_pivot(layer, &document.network_interface))
				.reduce(|a, b| a + b)
				.unwrap_or_default();

			let pivot = xy_summation / selected_layers_count as f64;
			self.pivot = Some(pivot);
			let [min, max] = document.selected_visible_and_unlock_layers_bounding_box_viewport().unwrap_or([DVec2::ZERO, DVec2::ONE]);
			self.normalized_pivot = (pivot - min) / (max - min);

			self.transform_from_normalized = DAffine2::from_translation(min) * DAffine2::from_scale(max - min);
		}
	}

	pub fn update_pivot(&mut self, document: &DocumentMessageHandler, overlay_context: &mut OverlayContext, angle: f64, show_hover_ring: bool) {
		self.recalculate_pivot(document);
		if let Some(pivot) = self.pivot {
			overlay_context.pivot(pivot, angle, show_hover_ring);
		}
	}

	/// Answers if the pivot widget has changed (so we should refresh the tool bar at the top of the canvas).
	pub fn should_refresh_pivot_position(&mut self) -> bool {
		let new = self.to_pivot_position();
		let should_refresh = new != self.old_pivot_position;
		self.old_pivot_position = new;
		should_refresh
	}

	pub fn to_pivot_position(&self) -> PivotPosition {
		self.normalized_pivot.into()
	}

	/// Sets the viewport position of the pivot for all selected layers.
	pub fn set_viewport_position(&self, position: DVec2, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		for layer in document
			.network_interface
			.selected_nodes(&[])
			.unwrap()
			.selected_visible_and_unlocked_layers(&document.network_interface)
		{
			let transform = Self::get_layer_pivot_transform(layer, document);
			let pivot = transform.inverse().transform_point2(position);
			// Only update the pivot when computed position is finite. Infinite can happen when scale is 0.
			if pivot.is_finite() {
				responses.add(GraphOperationMessage::TransformSetPivot { layer, pivot });
			}
		}
	}

	/// Set the pivot using the normalized transform that is set above.
	pub fn set_normalized_position(&self, position: DVec2, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		self.set_viewport_position(self.transform_from_normalized.transform_point2(position), document, responses);
	}

	/// Answers if the pointer is currently positioned over the pivot.
	pub fn compass_rose_state(&self, mouse: DVec2, angle: f64) -> CompassRoseState {
		match self.pivot {
			None => CompassRoseState::None,
			Some(pivot) => {
				let distance_squared = mouse.distance_squared(pivot);
				let ring_radius = (COMPASS_ROSE_MAIN_RING_DIAMETER + 1.) / 2.;

				for i in 0..4 {
					let base_angle = i as f64 * FRAC_PI_2 + angle;
					let direction = DVec2::from_angle(base_angle);

					let arrow_base = pivot + direction * ring_radius;
					let arrow_tip = arrow_base + direction * COMPASS_ROSE_ARROW_SIZE;

					let perp = direction.perp() * COMPASS_ROSE_ARROW_SIZE / 2.;
					let side1 = arrow_base + perp;
					let side2 = arrow_base - perp;

					if is_point_in_triangle(mouse, arrow_tip, side1, side2) {
						return if i % 2 == 0 { CompassRoseState::AxisX } else { CompassRoseState::AxisY };
					}
				}
				if distance_squared < (COMPASS_ROSE_PIVOT_DIAMETER / 2.).powi(2) {
					CompassRoseState::Pivot
				} else if (COMPASS_ROSE_RING_INNER_DIAMETER / 2.).powi(2) < distance_squared && distance_squared < (COMPASS_ROSE_MAIN_RING_DIAMETER / 2.).powi(2) {
					CompassRoseState::MainRing
				} else if (COMPASS_ROSE_MAIN_RING_DIAMETER / 2.).powi(2) < distance_squared && distance_squared < (COMPASS_ROSE_HOVER_RING_DIAMETER / 2.).powi(2) {
					CompassRoseState::HoverRing
				} else {
					CompassRoseState::None
				}
			}
		}
	}
}
fn is_point_in_triangle(p: DVec2, a: DVec2, b: DVec2, c: DVec2) -> bool {
	// Calculate barycentric coordinates
	let v0 = c - a;
	let v1 = b - a;
	let v2 = p - a;

	let dot00 = v0.dot(v0);
	let dot01 = v0.dot(v1);
	let dot02 = v0.dot(v2);
	let dot11 = v1.dot(v1);
	let dot12 = v1.dot(v2);

	let inv_denom = 1. / (dot00 * dot11 - dot01 * dot01);
	let u = (dot11 * dot02 - dot01 * dot12) * inv_denom;
	let v = (dot00 * dot12 - dot01 * dot02) * inv_denom;

	// Check if point is inside triangle
	u >= 0. && v >= 0. && u + v <= 1.
}

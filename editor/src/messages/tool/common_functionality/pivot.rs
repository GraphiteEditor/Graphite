//! Handler for the pivot overlay visible on the selected layer(s) whilst using the Select tool which controls the center of rotation/scale.

use crate::consts::PIVOT_DIAMETER;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::prelude::*;
use glam::{DAffine2, DVec2};
use graphene_std::transform::ReferencePoint;

#[derive(Clone, Debug)]
pub struct Pivot {
	/// Pivot between (0,0) and (1,1)
	normalized_pivot: DVec2,
	/// Transform to get from normalized pivot to viewspace
	transform_from_normalized: DAffine2,
	/// The viewspace pivot position
	pivot: Option<DVec2>,
	/// The old pivot position in the GUI, used to reduce refreshes of the document bar
	pub old_pivot_position: ReferencePoint,
	/// Used to enable and disable the pivot
	active: bool,
}

impl Default for Pivot {
	fn default() -> Self {
		Self {
			normalized_pivot: DVec2::splat(0.5),
			transform_from_normalized: Default::default(),
			pivot: Default::default(),
			old_pivot_position: ReferencePoint::Center,
			active: true,
		}
	}
}

impl Pivot {
	/// Recomputes the pivot position and transform.
	fn recalculate_pivot(&mut self, document: &DocumentMessageHandler) {
		if !self.active {
			return;
		}

		if !document.network_interface.selected_nodes().has_selected_nodes() {
			self.normalized_pivot = DVec2::ZERO;
			self.pivot = None;
			return;
		};

		let transform = document
			.network_interface
			.selected_nodes()
			.selected_visible_and_unlocked_layers(&document.network_interface)
			.find(|layer| !document.network_interface.is_artboard(&layer.to_node(), &[]))
			.map(|layer| document.metadata().transform_to_viewport_with_first_transform_node_if_group(layer, &document.network_interface))
			.unwrap_or_default();

		let [min, max] = document.selected_visible_and_unlock_layers_bounding_box_document().unwrap_or([DVec2::ZERO, DVec2::ONE]);
		self.transform_from_normalized = transform * DAffine2::from_translation(min) * DAffine2::from_scale(max - min);
		self.pivot = Some(self.transform_from_normalized.transform_point2(self.normalized_pivot));
	}

	pub fn update(&mut self, document: &DocumentMessageHandler, overlay_context: &mut OverlayContext, draw_data: Option<(f64,)>, draw: bool) {
		if !overlay_context.visibility_settings.pivot() {
			self.active = false;
			return;
		} else {
			self.active = true;
		}

		// self.recalculate_transform(document);
		self.recalculate_pivot(document);
		if !draw {
			return;
		};
		if let (Some(pivot), Some(data)) = (self.pivot, draw_data) {
			overlay_context.pivot(pivot, data.0);
		}
	}

	/// Answers if the pivot widget has changed (so we should refresh the tool bar at the top of the canvas).
	pub fn should_refresh_pivot_position(&mut self) -> bool {
		if !self.active {
			return false;
		}

		let new = self.to_pivot_position();
		let should_refresh = new != self.old_pivot_position;
		self.old_pivot_position = new;
		should_refresh
	}

	pub fn to_pivot_position(&self) -> ReferencePoint {
		self.normalized_pivot.into()
	}

	pub fn position(&self) -> Option<DVec2> {
		self.pivot
	}

	/// Sets the viewport position of the pivot.
	pub fn set_viewport_position(&mut self, position: DVec2) {
		if !self.active {
			return;
		}

		if self.transform_from_normalized.matrix2.determinant().abs() <= f64::EPSILON {
			return;
		};

		self.normalized_pivot = self.transform_from_normalized.inverse().transform_point2(position);
		self.pivot = Some(position);
	}

	/// Set the pivot using a normalized position.
	pub fn set_normalized_position(&mut self, position: DVec2) {
		if !self.active {
			return;
		}
		self.normalized_pivot = position;
		self.pivot = Some(self.transform_from_normalized.transform_point2(position));
	}

	/// Answers if the pointer is currently positioned over the pivot.
	pub fn is_over(&self, mouse: DVec2) -> bool {
		if !self.active {
			return false;
		}
		self.pivot.filter(|&pivot| mouse.distance_squared(pivot) < (PIVOT_DIAMETER / 2.).powi(2)).is_some()
	}
}

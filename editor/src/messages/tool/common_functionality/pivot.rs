//! Handler for the pivot overlay visible on the selected layer(s) whilst using the Select tool which controls the center of rotation/scale.

use crate::consts::PIVOT_DIAMETER;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
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
	// fn recalculate_transform(&mut self, document: &DocumentMessageHandler) {
	// 	if !self.active {
	// 		self.pivot = None;
	// 		return;
	// 	}
	//
	// 	if !document.network_interface.selected_nodes().has_selected_nodes() {
	// 		self.pivot = None;
	// 		return;
	// 	}
	//
	// 	let Some([min, max]) = document.selected_visible_and_unlock_layers_bounding_box_viewport() else {
	// 		self.pivot = None;
	// 		return;
	// 	};
	//
	// 	self.transform_from_normalized = DAffine2::from_translation(min) * DAffine2::from_scale(max - min);
	// 	self.pivot = Some(self.transform_from_normalized.transform_point2(self.normalized_pivot));
	// }

	fn recalculate_pivot(&mut self, document: &DocumentMessageHandler) {
		if !self.active {
			return;
		}

		let selected_nodes = document.network_interface.selected_nodes();
		let mut layers = selected_nodes.selected_visible_and_unlocked_layers(&document.network_interface);
		let Some(first) = layers.next() else {
			// If no layers are selected then we revert things back to default
			self.normalized_pivot = DVec2::ZERO;
			self.pivot = None;
			return;
		};

		// If just one layer is selected we can use its inner transform (as it accounts for rotation)
		self.transform_from_normalized = Self::get_layer_origin_transform(first, document);
		self.pivot = Some(self.transform_from_normalized.transform_point2(self.normalized_pivot));
	}

	fn get_layer_origin_transform(layer: LayerNodeIdentifier, document: &DocumentMessageHandler) -> DAffine2 {
		let [min, max] = document.metadata().nonzero_bounding_box(layer);

		let bounds_transform = DAffine2::from_translation(min) * DAffine2::from_scale(max - min);
		let layer_transform = document.metadata().transform_to_viewport(layer);
		layer_transform * bounds_transform
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

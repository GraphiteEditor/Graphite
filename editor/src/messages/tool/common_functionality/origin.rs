//! Handler for the origin overlay visible on the selected layer(s) whilst using the Select tool which controls the center of rotation/scale and origin of the layer.

use super::graph_modification_utils;
use crate::consts::DOWEL_PIN_RADIUS;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::*;
use glam::{DAffine2, DVec2};
use graphene_std::transform::ReferencePoint;
use std::collections::VecDeque;

#[derive(Clone, Debug)]
pub struct Origin {
	/// Origin between (0,0) and (1,1)
	normalized_origin: DVec2,
	/// Transform to get from normalized origin to viewspace
	transform_from_normalized: DAffine2,
	/// The viewspace origin position (if applicable)
	origin: Option<DVec2>,
	/// The old origin position in the GUI, used to reduce refreshes of the document bar
	old_origin_position: ReferencePoint,
	/// Used to enable and disable the origin
	active: bool,
}

impl Default for Origin {
	fn default() -> Self {
		Self {
			normalized_origin: DVec2::ZERO,
			transform_from_normalized: Default::default(),
			origin: Default::default(),
			old_origin_position: ReferencePoint::Center,
			active: true,
		}
	}
}

impl Origin {
	/// Calculates the transform that gets from normalized origin to viewspace.
	fn get_layer_origin_transform(layer: LayerNodeIdentifier, document: &DocumentMessageHandler) -> DAffine2 {
		let [min, max] = document.metadata().nonzero_bounding_box(layer);

		let bounds_transform = DAffine2::from_translation(min) * DAffine2::from_scale(max - min);
		let layer_transform = document.metadata().transform_to_viewport(layer);
		layer_transform * bounds_transform
	}

	/// Recomputes the origin position and transform.
	fn recalculate_origin(&mut self, document: &DocumentMessageHandler) {
		if !self.active {
			return;
		}

		let selected_nodes = document.network_interface.selected_nodes();
		let mut layers = selected_nodes.selected_visible_and_unlocked_layers(&document.network_interface);
		let Some(first) = layers.next() else {
			// If no layers are selected then we revert things back to default
			self.normalized_origin = DVec2::ZERO;
			self.origin = None;
			return;
		};

		// Add one because the first item is consumed above.
		let selected_layers_count = layers.count() + 1;

		// If just one layer is selected we can use its inner transform (as it accounts for rotation)
		if selected_layers_count == 1 {
			let normalized_origin = graph_modification_utils::get_origin(first, &document.network_interface).unwrap_or(DVec2::ZERO);
			self.normalized_origin = normalized_origin;
			self.transform_from_normalized = Self::get_layer_origin_transform(first, document);
			self.origin = Some(self.transform_from_normalized.transform_point2(normalized_origin));
		} else {
			// If more than one layer is selected we use the AABB with the mean of the origins
			let xy_summation = document
				.network_interface
				.selected_nodes()
				.selected_visible_and_unlocked_layers(&document.network_interface)
				.map(|layer| graph_modification_utils::get_viewport_origin(layer, &document.network_interface))
				.reduce(|a, b| a + b)
				.unwrap_or_default();

			let origin = xy_summation / selected_layers_count as f64;
			self.origin = Some(origin);
			let [min, max] = document.selected_visible_and_unlock_layers_bounding_box_viewport().unwrap_or([DVec2::ZERO, DVec2::ONE]);
			self.normalized_origin = (origin - min) / (max - min);

			self.transform_from_normalized = DAffine2::from_translation(min) * DAffine2::from_scale(max - min);
		}
	}

	pub fn update(&mut self, document: &DocumentMessageHandler, overlay_context: &mut OverlayContext) {
		if !overlay_context.visibility_settings.pivot() {
			self.active = false;
			return;
		} else {
			self.active = true;
		}

		self.recalculate_origin(document);
		if let Some(origin) = self.origin {
			overlay_context.draw_origin(origin);
		}
	}

	pub fn position(&self) -> Option<DVec2> {
		self.origin
	}

	/// Answers if the origin widget has changed (so we should refresh the tool bar at the top of the canvas).
	pub fn should_refresh_origin_position(&mut self) -> bool {
		if !self.active {
			return false;
		}

		let new = self.to_origin_position();
		let should_refresh = new != self.old_origin_position;
		self.old_origin_position = new;
		should_refresh
	}

	pub fn to_origin_position(&self) -> ReferencePoint {
		self.normalized_origin.into()
	}

	/// Sets the viewport position of the origin for all selected layers.
	pub fn set_viewport_position(&self, position: DVec2, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		if !self.active {
			return;
		}

		for layer in document.network_interface.selected_nodes().selected_visible_and_unlocked_layers(&document.network_interface) {
			let transform = Self::get_layer_origin_transform(layer, document);
			// Only update the origin when computed position is finite.
			if transform.matrix2.determinant().abs() <= f64::EPSILON {
				return;
			};
			let origin = transform.inverse().transform_point2(position);
			responses.add(GraphOperationMessage::TransformSetOrigin { layer, origin });
		}
	}

	/// Set the origin using the normalized transform that is set above.
	pub fn set_normalized_position(&self, position: DVec2, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		if !self.active {
			return;
		}

		self.set_viewport_position(self.transform_from_normalized.transform_point2(position), document, responses);
	}

	/// Answers if the pointer is currently positioned over the origin.
	pub fn is_over(&self, mouse: DVec2) -> bool {
		if !self.active {
			return false;
		}
		self.origin.filter(|&origin| mouse.distance_squared(origin) < (DOWEL_PIN_RADIUS).powi(2)).is_some()
	}
}

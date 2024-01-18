//! Handler for the pivot overlay visible on the selected layer(s) whilst using the Select tool which controls the center of rotation/scale and origin of the layer.

use super::graph_modification_utils;
use crate::consts::PIVOT_DIAMETER;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::*;

use glam::{DAffine2, DVec2};
use std::collections::VecDeque;

#[derive(Clone, Debug)]
pub struct Pivot {
	/// The viewspace pivot position (if applicable)
	pivot: Option<DVec2>,
	viewport_pos: DVec2,
}

impl Default for Pivot {
	fn default() -> Self {
		Self {
			pivot: None,
			viewport_pos: DVec2::ZERO,
		}
	}
}

impl Pivot {
	/// Calculates the transform that gets from normalized pivot to viewspace.
	fn get_layer_pivot_transform(layer: LayerNodeIdentifier, document: &DocumentMessageHandler) -> DAffine2 {
		document.metadata().transform_to_viewport(layer)
	}

	pub fn update_pivot(&mut self, document: &DocumentMessageHandler, overlay_context: &mut OverlayContext) {
		log::debug!("adding_overlay for pivot at ");
		if let Some(pivot) = self.pivot {
			let viewport_transform = document.metadata().document_to_viewport;
			let pivot = viewport_transform.transform_point2(pivot);
			self.viewport_pos = pivot;
			log::debug!("adding_overlay for pivot at {:?}", pivot);
			overlay_context.pivot(pivot);
		}
	}

	/// Answers if the pivot widget has changed (so we should refresh the tool bar at the top of the canvas).
	pub fn should_refresh_pivot_position(&mut self) -> bool {
		false
	}

	/// Sets the viewport position of the pivot for all selected layers.
	pub fn set_viewport_position(&mut self, position: DVec2, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		let viewport_transform = document.metadata().document_to_viewport.inverse();
		self.pivot = Some(viewport_transform.transform_point2(position));
		for layer in document.selected_nodes.selected_visible_layers(document.network(), document.metadata()) {
			let transform = Self::get_layer_pivot_transform(layer, document);
			let pivot = transform.inverse().transform_point2(position);
			// Only update the pivot when computed position is finite. Infinite can happen when scale is 0.
			if pivot.is_finite() {
				responses.add(GraphOperationMessage::TransformSetPivot { layer, pivot });
			}
		}
	}

	/// Answers if the pointer is currently positioned over the pivot.
	pub fn is_over(&self, mouse: DVec2) -> bool {
		self.pivot.filter(|&pivot| mouse.distance_squared(self.viewport_pos) < (PIVOT_DIAMETER / 2.).powi(2)).is_some()
	}
}

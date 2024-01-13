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

	/// Recomputes the pivot position and transform.
	fn recalculate_pivot(&mut self, document: &DocumentMessageHandler) {
		let mut layers = document.selected_nodes.selected_visible_layers(document.network(), document.metadata());
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
			let normalized_pivot = graph_modification_utils::get_pivot(first, &document.network).unwrap_or(DVec2::splat(0.5));
			self.normalized_pivot = normalized_pivot;
			self.transform_from_normalized = Self::get_layer_pivot_transform(first, document);
			self.pivot = Some(self.transform_from_normalized.transform_point2(normalized_pivot));
		} else {
			// If more than one layer is selected we use the AABB with the mean of the pivots
			let xy_summation = document
				.selected_nodes
				.selected_visible_layers(document.network(), document.metadata())
				.map(|layer| graph_modification_utils::get_viewport_pivot(layer, &document.network, &document.metadata))
				.reduce(|a, b| a + b)
				.unwrap_or_default();

			let pivot = xy_summation / selected_layers_count as f64;
			self.pivot = Some(pivot);
			let [min, max] = document.selected_visible_layers_bounding_box_viewport().unwrap_or([DVec2::ZERO, DVec2::ONE]);
			self.normalized_pivot = (pivot - min) / (max - min);

			self.transform_from_normalized = DAffine2::from_translation(min) * DAffine2::from_scale(max - min);
		}
	}

	pub fn update_pivot(&mut self, document: &DocumentMessageHandler, overlay_context: &mut OverlayContext) {
		self.recalculate_pivot(document);
		if let Some(pivot) = self.pivot {
			overlay_context.pivot(pivot);
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
		for layer in document.selected_nodes.selected_visible_layers(document.network(), document.metadata()) {
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
	pub fn is_over(&self, mouse: DVec2) -> bool {
		self.pivot.filter(|&pivot| mouse.distance_squared(pivot) < (PIVOT_DIAMETER / 2.).powi(2)).is_some()
	}
}

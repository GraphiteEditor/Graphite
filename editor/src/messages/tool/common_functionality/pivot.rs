//! Handler for the pivot overlay visible on the selected layer(s) whilst using the Select tool which controls the center of rotation/scale and origin of the layer.

use crate::application::generate_uuid;
use crate::consts::{COLOR_ACCENT, PIVOT_INNER, PIVOT_OUTER, PIVOT_OUTER_OUTLINE_THICKNESS};
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

use document_legacy::document_metadata::LayerNodeIdentifier;
use document_legacy::layers::style;
use document_legacy::{LayerId, Operation};

use glam::{DAffine2, DVec2};
use std::collections::VecDeque;

use super::graph_modification_utils;

#[derive(Clone, Debug)]
pub struct Pivot {
	/// Pivot between (0,0) and (1,1)
	normalized_pivot: DVec2,
	/// Transform to get from normalized pivot to viewspace
	transform_from_normalized: DAffine2,
	/// The viewspace pivot position (if applicable)
	pivot: Option<DVec2>,
	/// A reference to the previous overlays so we can destroy them
	pivot_overlay_circles: Option<[Vec<LayerId>; 2]>,
	/// The old pivot position in the GUI, used to reduce refreshes of the document bar
	old_pivot_position: PivotPosition,
}

impl Default for Pivot {
	fn default() -> Self {
		Self {
			normalized_pivot: DVec2::splat(0.5),
			transform_from_normalized: Default::default(),
			pivot: Default::default(),
			pivot_overlay_circles: Default::default(),
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
		let mut layers = document.document_legacy.selected_visible_layers();
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
			if let Some(normalized_pivot) = graph_modification_utils::get_pivot(first, &document.document_legacy) {
				self.normalized_pivot = normalized_pivot;
				self.transform_from_normalized = Self::get_layer_pivot_transform(first, document);
				self.pivot = Some(self.transform_from_normalized.transform_point2(normalized_pivot));
			}
		} else {
			// If more than one layer is selected we use the AABB with the mean of the pivots
			let xy_summation = document
				.document_legacy
				.selected_visible_layers()
				.filter_map(|layer| graph_modification_utils::get_viewport_pivot(layer, &document.document_legacy))
				.reduce(|a, b| a + b)
				.unwrap_or_default();

			let pivot = xy_summation / selected_layers_count as f64;
			self.pivot = Some(pivot);
			let [min, max] = document.document_legacy.selected_visible_layers_bounding_box_viewport().unwrap_or([DVec2::ZERO, DVec2::ONE]);
			self.normalized_pivot = (pivot - min) / (max - min);

			self.transform_from_normalized = DAffine2::from_translation(min) * DAffine2::from_scale(max - min);
		}
	}

	pub fn clear_overlays(&mut self, responses: &mut VecDeque<Message>) {
		if let Some(overlays) = self.pivot_overlay_circles.take() {
			for path in overlays {
				responses.add(DocumentMessage::Overlays(Operation::DeleteLayer { path }.into()));
			}
		}
	}

	fn redraw_pivot(&mut self, responses: &mut VecDeque<Message>) {
		self.clear_overlays(responses);

		let pivot = match self.pivot {
			Some(pivot) => pivot,
			None => return,
		};

		let layer_paths = [vec![generate_uuid()], vec![generate_uuid()]];
		responses.add(DocumentMessage::Overlays(
			Operation::AddEllipse {
				path: layer_paths[0].clone(),
				transform: DAffine2::IDENTITY.to_cols_array(),
				style: style::PathStyle::new(
					Some(style::Stroke::new(Some(COLOR_ACCENT), PIVOT_OUTER_OUTLINE_THICKNESS)),
					style::Fill::Solid(graphene_core::raster::color::Color::WHITE),
				),
				insert_index: -1,
			}
			.into(),
		));
		responses.add(DocumentMessage::Overlays(
			Operation::AddEllipse {
				path: layer_paths[1].clone(),
				transform: DAffine2::IDENTITY.to_cols_array(),
				style: style::PathStyle::new(None, style::Fill::Solid(COLOR_ACCENT)),
				insert_index: -1,
			}
			.into(),
		));

		self.pivot_overlay_circles = Some(layer_paths.clone());
		let [outer, inner] = layer_paths;

		let pivot_diameter_without_outline = PIVOT_OUTER - PIVOT_OUTER_OUTLINE_THICKNESS;
		let transform = DAffine2::from_scale_angle_translation(DVec2::splat(pivot_diameter_without_outline), 0., pivot - DVec2::splat(pivot_diameter_without_outline / 2.)).to_cols_array();
		responses.add(DocumentMessage::Overlays(Operation::TransformLayerInViewport { path: outer, transform }.into()));

		let transform = DAffine2::from_scale_angle_translation(DVec2::splat(PIVOT_INNER), 0., pivot - DVec2::splat(PIVOT_INNER / 2.)).to_cols_array();
		responses.add(DocumentMessage::Overlays(Operation::TransformLayerInViewport { path: inner, transform }.into()));
	}

	pub fn update_pivot(&mut self, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		self.recalculate_pivot(document);
		self.redraw_pivot(responses);
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
		for layer in document.document_legacy.selected_visible_layers() {
			let transform = Self::get_layer_pivot_transform(layer, document);
			let pivot = transform.inverse().transform_point2(position);
			// Only update the pivot when computed position is finite. Infinite can happen when scale is 0.
			if pivot.is_finite() {
				let layer = layer.to_path();
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
		self.pivot.filter(|&pivot| mouse.distance_squared(pivot) < (PIVOT_OUTER / 2.).powi(2)).is_some()
	}
}

//! Handler for the pivot overlay visible on the selected layer(s) whilst using the Select tool which controls the center of rotation/scale and origin of the layer.

use crate::application::generate_uuid;
use crate::consts::{COLOR_ACCENT, PIVOT_INNER, PIVOT_OUTER, PIVOT_OUTER_OUTLINE_THICKNESS};
use crate::messages::layout::utility_types::widgets::assist_widgets::PivotPosition;
use crate::messages::prelude::*;

use graphene::layers::style;
use graphene::layers::text_layer::FontCache;
use graphene::{LayerId, Operation};

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
	fn get_layer_pivot_transform(layer_path: &[LayerId], layer: &graphene::layers::layer_info::Layer, document: &DocumentMessageHandler, font_cache: &FontCache) -> DAffine2 {
		let [min, max] = layer.aabb_for_transform(DAffine2::IDENTITY, font_cache).unwrap_or([DVec2::ZERO, DVec2::ONE]);
		let bounds_transform = DAffine2::from_translation(min) * DAffine2::from_scale(max - min);
		let layer_transform = document.graphene_document.multiply_transforms(layer_path).unwrap_or(DAffine2::IDENTITY);
		layer_transform * bounds_transform
	}

	/// Recomputes the pivot position and transform.
	fn recalculate_pivot(&mut self, document: &DocumentMessageHandler, font_cache: &FontCache) {
		let mut layers = document.selected_visible_layers();
		if let Some(first) = layers.next() {
			// Add one because the first item is consumed above.
			let selected_layers_count = layers.count() + 1;

			// If just one layer is selected we can use its inner transform
			if selected_layers_count == 1 {
				if let Ok(layer) = document.graphene_document.layer(first) {
					self.normalized_pivot = layer.pivot;
					self.transform_from_normalized = Self::get_layer_pivot_transform(first, layer, document, font_cache);
					self.pivot = Some(self.transform_from_normalized.transform_point2(layer.pivot));
				}
			} else {
				// If more than one layer is selected we use the AABB with the mean of the pivots
				let xy_summation = document
					.selected_visible_layers()
					.filter_map(|path| document.graphene_document.pivot(path, font_cache))
					.reduce(|a, b| a + b)
					.unwrap_or_default();

				let pivot = xy_summation / selected_layers_count as f64;
				self.pivot = Some(pivot);
				let [min, max] = document.selected_visible_layers_bounding_box(font_cache).unwrap_or([DVec2::ZERO, DVec2::ONE]);
				self.normalized_pivot = (pivot - min) / (max - min);

				self.transform_from_normalized = DAffine2::from_translation(min) * DAffine2::from_scale(max - min);
			}
		} else {
			// If no layers are selected then we revert things back to default
			self.normalized_pivot = DVec2::splat(0.5);
			self.pivot = None;
		}
	}

	pub fn clear_overlays(&mut self, responses: &mut VecDeque<Message>) {
		if let Some(overlays) = self.pivot_overlay_circles.take() {
			for path in overlays {
				responses.push_back(DocumentMessage::Overlays(Operation::DeleteLayer { path }.into()).into());
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
		responses.push_back(
			DocumentMessage::Overlays(
				Operation::AddEllipse {
					path: layer_paths[0].clone(),
					transform: DAffine2::IDENTITY.to_cols_array(),
					style: style::PathStyle::new(Some(style::Stroke::new(COLOR_ACCENT, PIVOT_OUTER_OUTLINE_THICKNESS)), style::Fill::Solid(graphene::color::Color::WHITE)),
					insert_index: -1,
				}
				.into(),
			)
			.into(),
		);
		responses.push_back(
			DocumentMessage::Overlays(
				Operation::AddEllipse {
					path: layer_paths[1].clone(),
					transform: DAffine2::IDENTITY.to_cols_array(),
					style: style::PathStyle::new(None, style::Fill::Solid(COLOR_ACCENT)),
					insert_index: -1,
				}
				.into(),
			)
			.into(),
		);

		self.pivot_overlay_circles = Some(layer_paths.clone());
		let [outer, inner] = layer_paths;

		let pivot_diameter_without_outline = PIVOT_OUTER - PIVOT_OUTER_OUTLINE_THICKNESS;
		let transform = DAffine2::from_scale_angle_translation(DVec2::splat(pivot_diameter_without_outline), 0., pivot - DVec2::splat(pivot_diameter_without_outline / 2.)).to_cols_array();
		responses.push_back(DocumentMessage::Overlays(Operation::TransformLayerInViewport { path: outer, transform }.into()).into());

		let transform = DAffine2::from_scale_angle_translation(DVec2::splat(PIVOT_INNER), 0., pivot - DVec2::splat(PIVOT_INNER / 2.)).to_cols_array();
		responses.push_back(DocumentMessage::Overlays(Operation::TransformLayerInViewport { path: inner, transform }.into()).into());
	}

	pub fn update_pivot(&mut self, document: &DocumentMessageHandler, font_cache: &FontCache, responses: &mut VecDeque<Message>) {
		self.recalculate_pivot(document, font_cache);
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
	pub fn set_viewport_position(&self, position: DVec2, document: &DocumentMessageHandler, font_cache: &FontCache, responses: &mut VecDeque<Message>) {
		for layer_path in document.selected_visible_layers() {
			if let Ok(layer) = document.graphene_document.layer(layer_path) {
				let transform = Self::get_layer_pivot_transform(layer_path, layer, document, font_cache);
				let pivot = transform.inverse().transform_point2(position);
				// Only update the pivot when computed position is finite. Infinite can happen when scale is 0.
				if pivot.is_finite() {
					let layer_path = layer_path.to_owned();
					responses.push_back(Operation::SetPivot { layer_path, pivot: pivot.into() }.into());
				}
			}
		}
	}

	/// Set the pivot using the normalized transform that is set above.
	pub fn set_normalized_position(&self, position: DVec2, document: &DocumentMessageHandler, font_cache: &FontCache, responses: &mut VecDeque<Message>) {
		self.set_viewport_position(self.transform_from_normalized.transform_point2(position), document, font_cache, responses);
	}

	/// Answers if the pointer is currently positioned over the pivot.
	pub fn is_over(&self, mouse: DVec2) -> bool {
		self.pivot.filter(|&pivot| mouse.distance_squared(pivot) < (PIVOT_OUTER / 2.).powi(2)).is_some()
	}
}

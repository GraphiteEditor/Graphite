use crate::document::DocumentMessageHandler;

use graphene::layers::layer_info::LayerDataType;
use graphene::layers::style::{self, Fill, Stroke};
use graphene::{LayerId, Operation};

use glam::DAffine2;
use kurbo::{BezPath, Shape};
use std::collections::VecDeque;

use crate::consts::{COLOR_ACCENT, SELECTION_HOVER_WEIGHT};
use crate::message_prelude::*;

/// Manages the overlay used by the select tool when hovering over a non selected shape.
#[derive(Clone, Debug, Default)]
pub struct HoverOutline {
	layer_path: Option<Vec<LayerId>>,
	overlay_path: Option<Vec<LayerId>>,
}
impl HoverOutline {
	/// Removes the overlay and deletes path references
	pub fn clear(&mut self, responses: &mut VecDeque<Message>) {
		if let Some(path) = self.overlay_path.take() {
			let operation = Operation::DeleteLayer { path };
			responses.push_back(DocumentMessage::Overlays(operation.into()).into());
		}
		self.layer_path = None;
	}

	/// Updates the overlay, generating a new one if necessary
	pub fn update(&mut self, new_layer_path: Vec<LayerId>, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		// Check if we are hovering over a different layer than before
		if self.layer_path.as_ref().map_or(true, |old| &new_layer_path != old) {
			// Get layer data
			if let Ok(layer) = document.graphene_document.layer(&new_layer_path) {
				// Get the bezpath from the shape or text
				let path = match &layer.data {
					LayerDataType::Shape(shape) => Some(shape.path.clone()),
					LayerDataType::Text(text) => Some(text.to_bez_path_nonmut(&document.graphene_document.font_cache)),
					_ => layer
						.aabounding_box_for_transform(DAffine2::IDENTITY, &document.graphene_document.font_cache)
						.map(|bounds| kurbo::Rect::new(bounds[0].x, bounds[0].y, bounds[1].x, bounds[1].y).to_path(0.)),
				};

				// Check that we have the bezpath
				if let Some(path) = path {
					// Generate a new overlay layer if necessary
					let overlay = match &self.overlay_path {
						Some(path) => path.clone(),
						None => {
							let overlay_path = vec![generate_uuid()];
							let operation = Operation::AddOverlayShape {
								path: overlay_path.clone(),
								bez_path: BezPath::new(),
								style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, SELECTION_HOVER_WEIGHT)), Fill::None),
								closed: false,
							};

							responses.push_back(DocumentMessage::Overlays(operation.into()).into());

							self.overlay_path = Some(overlay_path.clone());
							overlay_path
						}
					};

					// Update the shape bezpath
					let operation = Operation::SetShapePath {
						path: overlay.clone(),
						bez_path: path,
					};
					responses.push_back(DocumentMessage::Overlays(operation.into()).into());

					// Update the transform to match the document
					let operation = Operation::SetLayerTransform {
						path: overlay,
						transform: document.graphene_document.multiply_transforms(&new_layer_path).unwrap().to_cols_array(),
					};
					responses.push_back(DocumentMessage::Overlays(operation.into()).into());
				} else {
					self.clear(responses);
				}
			} else {
				self.clear(responses);
			}
		}
		self.layer_path = Some(new_layer_path);
	}
}

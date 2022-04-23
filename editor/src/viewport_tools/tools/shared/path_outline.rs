use crate::consts::{COLOR_ACCENT, PATH_OUTLINE_WEIGHT};
use crate::document::DocumentMessageHandler;
use crate::message_prelude::*;

use graphene::layers::layer_info::LayerDataType;
use graphene::layers::style::{self, Fill, Stroke};
use graphene::{LayerId, Operation};

use glam::DAffine2;
use kurbo::{BezPath, Shape};
use std::collections::VecDeque;

/// Manages the overlay used by the select tool for outlining selected shapes and when hovering over a non selected shape.
#[derive(Clone, Debug, Default)]
pub struct PathOutline {
	hovered_layer_path: Option<Vec<LayerId>>,
	hovered_overlay_path: Option<Vec<LayerId>>,
	selected_overlay_paths: Vec<Vec<LayerId>>,
}

impl PathOutline {
	/// Creates an outline of a layer either with a pre-existing overlay or by generating a new one
	fn create_outline(document_layer_path: Vec<LayerId>, overlay_path: Option<Vec<LayerId>>, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) -> Option<Vec<LayerId>> {
		// Get layer data
		let document_layer = document.graphene_document.layer(&document_layer_path).ok()?;

		// Get the bezpath from the shape or text
		let path = match &document_layer.data {
			LayerDataType::Shape(shape) => Some(shape.path.clone()),
			LayerDataType::Text(text) => Some(text.to_bez_path_nonmut(&document.graphene_document.font_cache)),
			_ => document_layer
				.aabounding_box_for_transform(DAffine2::IDENTITY, &document.graphene_document.font_cache)
				.map(|bounds| kurbo::Rect::new(bounds[0].x, bounds[0].y, bounds[1].x, bounds[1].y).to_path(0.)),
		}?;

		// Generate a new overlay layer if necessary
		let overlay = match overlay_path {
			Some(path) => path,
			None => {
				let overlay_path = vec![generate_uuid()];
				let operation = Operation::AddOverlayShape {
					path: overlay_path.clone(),
					bez_path: BezPath::new(),
					style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, PATH_OUTLINE_WEIGHT)), Fill::None),
					closed: false,
				};

				responses.push_back(DocumentMessage::Overlays(operation.into()).into());

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
			path: overlay.clone(),
			transform: document.graphene_document.multiply_transforms(&document_layer_path).unwrap().to_cols_array(),
		};
		responses.push_back(DocumentMessage::Overlays(operation.into()).into());

		Some(overlay)
	}

	/// Removes the hovered overlay and deletes path references
	pub fn clear_hovered(&mut self, responses: &mut VecDeque<Message>) {
		if let Some(path) = self.hovered_overlay_path.take() {
			let operation = Operation::DeleteLayer { path };
			responses.push_back(DocumentMessage::Overlays(operation.into()).into());
		}
		self.hovered_layer_path = None;
	}

	/// Updates the overlay, generating a new one if necessary
	pub fn update_hovered(&mut self, new_layer_path: Vec<LayerId>, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		// Check if we are hovering over a different layer than before
		if self.hovered_layer_path.as_ref().map_or(true, |old| &new_layer_path != old) {
			self.hovered_overlay_path = Self::create_outline(new_layer_path.clone(), self.hovered_overlay_path.take(), document, responses);
			if self.hovered_overlay_path.is_none() {
				self.clear_hovered(responses);
			}
		}
		self.hovered_layer_path = Some(new_layer_path);
	}

	/// Clears overlays for the seleted paths and removes references
	pub fn clear_selected(&mut self, responses: &mut VecDeque<Message>) {
		if let Some(path) = self.selected_overlay_paths.pop() {
			let operation = Operation::DeleteLayer { path };
			responses.push_back(DocumentMessage::Overlays(operation.into()).into());
		}
	}

	/// Updates the selected overlays, generating or removing overlays if necessary
	pub fn update_selected<'a>(&mut self, selected: impl Iterator<Item = &'a [LayerId]>, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		let mut old_overlay_paths = std::mem::take(&mut self.selected_overlay_paths);

		for document_layer_path in selected {
			if let Some(overlay_path) = Self::create_outline(document_layer_path.to_vec(), old_overlay_paths.pop(), document, responses) {
				self.selected_overlay_paths.push(overlay_path);
			}
		}
		for path in old_overlay_paths {
			let operation = Operation::DeleteLayer { path };
			responses.push_back(DocumentMessage::Overlays(operation.into()).into());
		}
	}
}

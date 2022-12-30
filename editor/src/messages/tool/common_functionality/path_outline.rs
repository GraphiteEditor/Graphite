use crate::application::generate_uuid;
use crate::consts::{COLOR_ACCENT, PATH_OUTLINE_WEIGHT, SELECTION_TOLERANCE};
use crate::messages::prelude::*;

use document_legacy::intersection::Quad;
use document_legacy::layers::layer_info::LayerDataType;
use document_legacy::layers::style::{self, Fill, Stroke};
use document_legacy::layers::text_layer::FontCache;
use document_legacy::{LayerId, Operation};
use graphene_std::vector::subpath::Subpath;

use glam::{DAffine2, DVec2};

/// Manages the overlay used by the select tool for outlining selected shapes and when hovering over a non selected shape.
#[derive(Clone, Debug, Default)]
pub struct PathOutline {
	hovered_layer_path: Option<Vec<LayerId>>,
	hovered_overlay_path: Option<Vec<LayerId>>,
	selected_overlay_paths: Vec<Vec<LayerId>>,
}

impl PathOutline {
	/// Creates an outline of a layer either with a pre-existing overlay or by generating a new one
	fn try_create_outline(
		document_layer_path: Vec<LayerId>,
		overlay_path: Option<Vec<LayerId>>,
		document: &DocumentMessageHandler,
		responses: &mut VecDeque<Message>,
		font_cache: &FontCache,
	) -> Option<Vec<LayerId>> {
		// Get layer data
		let document_layer = document.document_legacy.layer(&document_layer_path).ok()?;

		// TODO Purge this area of BezPath and Kurbo
		// Get the bezpath from the shape or text
		let subpath = match &document_layer.data {
			LayerDataType::Shape(layer_shape) => Some(layer_shape.shape.clone()),
			LayerDataType::Text(text) => Some(text.to_subpath_nonmut(font_cache)),
			_ => document_layer.aabb_for_transform(DAffine2::IDENTITY, font_cache).map(|[p1, p2]| Subpath::new_rect(p1, p2)),
		}?;

		// Generate a new overlay layer if necessary
		let overlay = match overlay_path {
			Some(path) => path,
			None => {
				let overlay_path = vec![generate_uuid()];
				let operation = Operation::AddShape {
					path: overlay_path.clone(),
					subpath: Default::default(),
					style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, PATH_OUTLINE_WEIGHT)), Fill::None),
					insert_index: -1,
					transform: DAffine2::IDENTITY.to_cols_array(),
				};

				responses.push_back(DocumentMessage::Overlays(operation.into()).into());

				overlay_path
			}
		};

		// Update the shape bezpath
		let operation = Operation::SetShapePath { path: overlay.clone(), subpath };
		responses.push_back(DocumentMessage::Overlays(operation.into()).into());

		// Update the transform to match the document
		let operation = Operation::SetLayerTransform {
			path: overlay.clone(),
			transform: document.document_legacy.multiply_transforms(&document_layer_path).unwrap().to_cols_array(),
		};
		responses.push_back(DocumentMessage::Overlays(operation.into()).into());

		Some(overlay)
	}

	/// Creates an outline of a layer either with a pre-existing overlay or by generating a new one
	///
	/// Creates an outline, discarding the overlay on failiure
	fn create_outline(
		document_layer_path: Vec<LayerId>,
		overlay_path: Option<Vec<LayerId>>,
		document: &DocumentMessageHandler,
		responses: &mut VecDeque<Message>,
		font_cache: &FontCache,
	) -> Option<Vec<LayerId>> {
		let copied_overlay_path = overlay_path.clone();
		let result = Self::try_create_outline(document_layer_path, overlay_path, document, responses, font_cache);
		if result.is_none() {
			// Discard the overlay layer if it exists
			if let Some(overlay_path) = copied_overlay_path {
				let operation = Operation::DeleteLayer { path: overlay_path };
				responses.push_back(DocumentMessage::Overlays(operation.into()).into());
			}
		}
		result
	}

	/// Removes the hovered overlay and deletes path references
	pub fn clear_hovered(&mut self, responses: &mut VecDeque<Message>) {
		if let Some(path) = self.hovered_overlay_path.take() {
			let operation = Operation::DeleteLayer { path };
			responses.push_back(DocumentMessage::Overlays(operation.into()).into());
		}
		self.hovered_layer_path = None;
	}

	/// Performs an intersect test and generates a hovered overlay if necessary
	pub fn intersect_test_hovered(&mut self, input: &InputPreprocessorMessageHandler, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>, font_cache: &FontCache) {
		// Get the layer the user is hovering over
		let tolerance = DVec2::splat(SELECTION_TOLERANCE);
		let quad = Quad::from_box([input.mouse.position - tolerance, input.mouse.position + tolerance]);
		let mut intersection = document.document_legacy.intersects_quad_root(quad, font_cache);

		// If the user is hovering over a layer they have not already selected, then update outline
		if let Some(path) = intersection.pop() {
			if !document.selected_visible_layers().any(|visible| visible == path.as_slice()) {
				// Updates the overlay, generating a new one if necessary
				self.hovered_overlay_path = Self::create_outline(path.clone(), self.hovered_overlay_path.take(), document, responses, font_cache);
				if self.hovered_overlay_path.is_none() {
					self.clear_hovered(responses);
				}

				self.hovered_layer_path = Some(path);
			} else {
				self.clear_hovered(responses);
			}
		} else {
			self.clear_hovered(responses);
		}
	}

	/// Clears overlays for the selected paths and removes references
	pub fn clear_selected(&mut self, responses: &mut VecDeque<Message>) {
		while let Some(path) = self.selected_overlay_paths.pop() {
			let operation = Operation::DeleteLayer { path };
			responses.push_back(DocumentMessage::Overlays(operation.into()).into());
		}
	}

	/// Updates the selected overlays, generating or removing overlays if necessary
	pub fn update_selected<'a>(&mut self, selected: impl Iterator<Item = &'a [LayerId]>, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>, font_cache: &FontCache) {
		let mut old_overlay_paths = std::mem::take(&mut self.selected_overlay_paths);

		for document_layer_path in selected {
			if let Some(overlay_path) = Self::create_outline(document_layer_path.to_vec(), old_overlay_paths.pop(), document, responses, font_cache) {
				self.selected_overlay_paths.push(overlay_path);
			}
		}
		for path in old_overlay_paths {
			let operation = Operation::DeleteLayer { path };
			responses.push_back(DocumentMessage::Overlays(operation.into()).into());
		}
	}
}

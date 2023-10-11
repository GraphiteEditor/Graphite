use crate::application::generate_uuid;
use crate::consts::{COLOR_ACCENT, PATH_OUTLINE_WEIGHT};
use crate::messages::prelude::*;

use document_legacy::document_metadata::LayerNodeIdentifier;
use document_legacy::layers::style::{self, Fill, RenderData, Stroke};
use document_legacy::{LayerId, Operation};

use glam::DAffine2;

/// Manages the overlay used by the select tool for outlining selected shapes and when hovering over a non selected shape.
#[derive(Clone, Debug, Default)]
pub struct PathOutline {
	hovered_layer_path: Option<LayerNodeIdentifier>,
	hovered_overlay_path: Option<Vec<LayerId>>,
	selected_overlay_paths: Vec<Vec<LayerId>>,
}

impl PathOutline {
	/// Creates an outline of a layer either with a pre-existing overlay or by generating a new one
	fn try_create_outline(
		layer: LayerNodeIdentifier,
		overlay_path: Option<Vec<LayerId>>,
		document: &DocumentMessageHandler,
		responses: &mut VecDeque<Message>,
		render_data: &RenderData,
	) -> Option<Vec<LayerId>> {
		let subpath = document.metadata().layer_outline(layer);
		let transform = document.metadata().transform_to_viewport(layer);

		// Generate a new overlay layer if necessary
		let overlay = overlay_path.unwrap_or_else(|| {
			let overlay_path = vec![generate_uuid()];

			responses.add(DocumentMessage::Overlays(
				(Operation::AddShape {
					path: overlay_path.clone(),
					subpath: Default::default(),
					style: style::PathStyle::new(Some(Stroke::new(Some(COLOR_ACCENT), PATH_OUTLINE_WEIGHT)), Fill::None),
					insert_index: -1,
					transform: DAffine2::IDENTITY.to_cols_array(),
				})
				.into(),
			));

			overlay_path
		});

		// Update the shape subpath
		responses.add(DocumentMessage::Overlays((Operation::SetShapePath { path: overlay.clone(), subpath }).into()));

		// Update the transform to match the document
		responses.add(DocumentMessage::Overlays(
			(Operation::SetLayerTransform {
				path: overlay.clone(),
				transform: transform.to_cols_array(),
			})
			.into(),
		));

		Some(overlay)
	}

	/// Creates an outline of a layer either with a pre-existing overlay or by generating a new one.
	///
	/// Creates an outline, discarding the overlay on failure.
	fn create_outline(
		layer: LayerNodeIdentifier,
		overlay_path: Option<Vec<LayerId>>,
		document: &DocumentMessageHandler,
		responses: &mut VecDeque<Message>,
		render_data: &RenderData,
	) -> Option<Vec<LayerId>> {
		let copied_overlay_path = overlay_path.clone();
		let result = Self::try_create_outline(layer, overlay_path, document, responses, render_data);
		if result.is_none() {
			// Discard the overlay layer if it exists
			if let Some(overlay_path) = copied_overlay_path {
				let operation = Operation::DeleteLayer { path: overlay_path };
				responses.add(DocumentMessage::Overlays(operation.into()));
			}
		}
		result
	}

	/// Removes the hovered overlay and deletes path references
	pub fn clear_hovered(&mut self, responses: &mut VecDeque<Message>) {
		if let Some(path) = self.hovered_overlay_path.take() {
			let operation = Operation::DeleteLayer { path };
			responses.add(DocumentMessage::Overlays(operation.into()));
		}
		self.hovered_layer_path = None;
	}

	/// Performs an intersect test and generates a hovered overlay if necessary
	pub fn intersect_test_hovered(&mut self, input: &InputPreprocessorMessageHandler, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>, render_data: &RenderData) {
		// Get the layer the user is hovering over
		let intersection = document.metadata().click(input.mouse.position, &document.document_legacy.document_network);

		let Some(hovered_layer) = intersection else {
			self.clear_hovered(responses);
			return;
		};

		if document.metadata().selected_layers_contains(hovered_layer) {
			self.clear_hovered(responses);
			return;
		}

		// Updates the overlay, generating a new one if necessary
		self.hovered_overlay_path = Self::create_outline(hovered_layer, self.hovered_overlay_path.take(), document, responses, render_data);
		if self.hovered_overlay_path.is_none() {
			self.clear_hovered(responses);
		}

		self.hovered_layer_path = Some(hovered_layer);
	}

	/// Clears overlays for the selected paths and removes references
	pub fn clear_selected(&mut self, responses: &mut VecDeque<Message>) {
		while let Some(path) = self.selected_overlay_paths.pop() {
			let operation = Operation::DeleteLayer { path };
			responses.add(DocumentMessage::Overlays(operation.into()));
		}
	}

	/// Updates the selected overlays, generating or removing overlays if necessary
	pub fn update_selected<'a>(&mut self, selected: impl Iterator<Item = LayerNodeIdentifier>, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>, render_data: &RenderData) {
		let mut old_overlay_paths = std::mem::take(&mut self.selected_overlay_paths);

		for layer_identifier in selected {
			if let Some(overlay_path) = Self::create_outline(layer_identifier, old_overlay_paths.pop(), document, responses, render_data) {
				self.selected_overlay_paths.push(overlay_path);
			}
		}
		for path in old_overlay_paths {
			let operation = Operation::DeleteLayer { path };
			responses.add(DocumentMessage::Overlays(operation.into()));
		}
	}
}

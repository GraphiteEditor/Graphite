use crate::consts::{DRAG_DIRECTION_MODE_DETERMINATION_THRESHOLD, SELECTION_TOLERANCE};
use crate::messages::{portfolio::document::utility_types::document_metadata::DocumentMetadata, preferences::SelectionMode, tool::tool_messages::tool_prelude::*};
/// Represents the current drag in progress
#[derive(Clone, Debug, Default)]
pub struct DragState {
	pub start_document: DVec2,
	pub current_document: DVec2,
	/// Selection mode is set when the drag exceeds a certain distance. Once resolved, the selection mode cannot change.
	resolved_selection_mode: Option<SelectionMode>,
}

impl DragState {
	pub fn new(input: &InputPreprocessorMessageHandler, metadata: &DocumentMetadata) -> Self {
		let document_mouse = metadata.document_to_viewport.inverse().transform_point2(input.mouse.position);
		Self {
			start_document: document_mouse,
			current_document: document_mouse,
			resolved_selection_mode: None,
		}
	}
	pub fn set_current(&mut self, input: &InputPreprocessorMessageHandler, metadata: &DocumentMetadata) {
		self.current_document = metadata.document_to_viewport.inverse().transform_point2(input.mouse.position);
	}

	pub fn offset_viewport(&mut self, offset: DVec2, metadata: &DocumentMetadata) {
		self.current_document = self.current_document + metadata.document_to_viewport.inverse().transform_vector2(offset);
	}

	pub fn start_viewport(&self, metadata: &DocumentMetadata) -> DVec2 {
		metadata.document_to_viewport.transform_point2(self.start_document)
	}

	pub fn current_viewport(&self, metadata: &DocumentMetadata) -> DVec2 {
		metadata.document_to_viewport.transform_point2(self.current_document)
	}

	pub fn start_current_viewport(&self, metadata: &DocumentMetadata) -> [DVec2; 2] {
		[self.start_viewport(metadata), self.current_viewport(metadata)]
	}

	pub fn total_drag_delta_document(&self) -> DVec2 {
		self.current_document - self.start_document
	}

	pub fn total_drag_delta_viewport(&self, metadata: &DocumentMetadata) -> DVec2 {
		metadata.document_to_viewport.transform_vector2(self.total_drag_delta_document())
	}

	pub fn inverse_drag_delta_viewport(&self, metadata: &DocumentMetadata) -> DVec2 {
		-self.total_drag_delta_viewport(metadata)
	}

	pub fn update_selection_mode(&mut self, metadata: &DocumentMetadata, preferences: &PreferencesMessageHandler) -> SelectionMode {
		if let Some(resolved_selection_mode) = self.resolved_selection_mode {
			return resolved_selection_mode;
		}
		if preferences.get_selection_mode() != SelectionMode::Directional {
			self.resolved_selection_mode = Some(preferences.get_selection_mode());
			return preferences.get_selection_mode();
		}

		let [start, current] = self.start_current_viewport(metadata);

		// Drag direction cannot be resolved TODO: why not consider only X distance?
		if start.distance_squared(current) >= DRAG_DIRECTION_MODE_DETERMINATION_THRESHOLD.powi(2) {
			let selection_mode = if current.x < start.x { SelectionMode::Touched } else { SelectionMode::Enclosed };
			self.resolved_selection_mode = Some(selection_mode);
			return selection_mode;
		}

		SelectionMode::default()
	}

	/// A viewport quad representing the drag bounds. Expanded if the start == end
	pub fn expanded_selection_box_viewport(&self, metadata: &DocumentMetadata) -> [DVec2; 2] {
		let [start, current] = self.start_current_viewport(metadata);
		if start == current {
			let tolerance = DVec2::splat(SELECTION_TOLERANCE);
			[current - tolerance, current + tolerance]
		} else {
			[start, current]
		}
	}
}

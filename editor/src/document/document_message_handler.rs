use super::clipboards::Clipboard;
use super::layer_panel::{layer_panel_entry, LayerMetadata, LayerPanelEntry, RawBuffer};
use super::utility_types::{AlignAggregate, AlignAxis, DocumentSave, FlipAxis, VectorManipulatorAnchor, VectorManipulatorPoint, VectorManipulatorSegment, VectorManipulatorShape};
use super::vectorize_layer_metadata;
use super::{ArtboardMessageHandler, MovementMessageHandler, OverlaysMessageHandler, TransformLayerMessageHandler};
use crate::consts::{
	ASYMPTOTIC_EFFECT, DEFAULT_DOCUMENT_NAME, FILE_EXPORT_SUFFIX, FILE_SAVE_SUFFIX, GRAPHITE_DOCUMENT_VERSION, SCALE_EFFECT, SCROLLBAR_SPACING, VIEWPORT_ZOOM_TO_FIT_PADDING_SCALE_FACTOR,
};
use crate::input::InputPreprocessorMessageHandler;
use crate::message_prelude::*;
use crate::EditorError;

use graphene::document::Document as GrapheneDocument;
use graphene::layers::folder::Folder;
use graphene::layers::layer_info::LayerDataType;
use graphene::layers::style::ViewMode;
use graphene::{DocumentError, DocumentResponse, LayerId, Operation as DocumentOperation};

use glam::{DAffine2, DVec2};
use kurbo::PathSeg;
use log::warn;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::VecDeque;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentMessageHandler {
	pub graphene_document: GrapheneDocument,
	#[serde(skip)]
	pub document_undo_history: Vec<DocumentSave>,
	#[serde(skip)]
	pub document_redo_history: Vec<DocumentSave>,
	pub saved_document_identifier: u64,
	pub name: String,
	#[serde(with = "vectorize_layer_metadata")]
	pub layer_metadata: HashMap<Vec<LayerId>, LayerMetadata>,
	layer_range_selection_reference: Vec<LayerId>,
	movement_handler: MovementMessageHandler,
	#[serde(skip)]
	overlays_message_handler: OverlaysMessageHandler,
	artboard_message_handler: ArtboardMessageHandler,
	#[serde(skip)]
	transform_layer_handler: TransformLayerMessageHandler,
	pub overlays_visible: bool,
	pub snapping_enabled: bool,
	pub view_mode: ViewMode,
	pub version: String,
}

impl Default for DocumentMessageHandler {
	fn default() -> Self {
		Self {
			graphene_document: GrapheneDocument::default(),
			document_undo_history: Vec::new(),
			document_redo_history: Vec::new(),
			saved_document_identifier: 0,
			name: String::from("Untitled Document"),
			layer_metadata: vec![(vec![], LayerMetadata::new(true))].into_iter().collect(),
			layer_range_selection_reference: Vec::new(),
			movement_handler: MovementMessageHandler::default(),
			overlays_message_handler: OverlaysMessageHandler::default(),
			artboard_message_handler: ArtboardMessageHandler::default(),
			transform_layer_handler: TransformLayerMessageHandler::default(),
			snapping_enabled: true,
			overlays_visible: true,
			view_mode: ViewMode::default(),
			version: GRAPHITE_DOCUMENT_VERSION.to_string(),
		}
	}
}

impl DocumentMessageHandler {
	pub fn serialize_document(&self) -> String {
		let val = serde_json::to_string(self);
		// We fully expect the serialization to succeed
		val.unwrap()
	}

	pub fn deserialize_document(serialized_content: &str) -> Result<Self, DocumentError> {
		let deserialized_result: Result<Self, DocumentError> = serde_json::from_str(serialized_content).map_err(|e| DocumentError::InvalidFile(e.to_string()));
		match deserialized_result {
			Ok(document) => {
				if document.version == GRAPHITE_DOCUMENT_VERSION {
					Ok(document)
				} else {
					Err(DocumentError::InvalidFile("Graphite document version mismatch".to_string()))
				}
			}
			Err(e) => Err(e),
		}
	}

	pub fn with_name(name: String, ipp: &InputPreprocessorMessageHandler) -> Self {
		let mut document = Self { name, ..Self::default() };
		let starting_root_transform = document.movement_handler.calculate_offset_transform(ipp.viewport_bounds.size() / 2.);
		document.graphene_document.root.transform = starting_root_transform;
		document.artboard_message_handler.artboards_graphene_document.root.transform = starting_root_transform;
		document
	}

	pub fn with_name_and_content(name: String, serialized_content: String) -> Result<Self, EditorError> {
		match Self::deserialize_document(&serialized_content) {
			Ok(mut document) => {
				document.name = name;
				Ok(document)
			}
			Err(DocumentError::InvalidFile(msg)) => Err(EditorError::Document(msg)),
			_ => Err(EditorError::Document(String::from("Failed to open file"))),
		}
	}

	pub fn is_unmodified_default(&self) -> bool {
		self.serialize_root().len() == Self::default().serialize_root().len()
			&& self.document_undo_history.is_empty()
			&& self.document_redo_history.is_empty()
			&& self.name.starts_with(DEFAULT_DOCUMENT_NAME)
	}

	fn select_layer(&mut self, path: &[LayerId]) -> Option<Message> {
		println!("Select_layer fail: {:?}", self.all_layers_sorted());

		if let Some(layer) = self.layer_metadata.get_mut(path) {
			layer.selected = true;
			let data = self.layer_panel_entry(path.to_vec()).ok()?;
			(!path.is_empty()).then(|| FrontendMessage::UpdateDocumentLayer { data }.into())
		} else {
			log::warn!("Tried to select non existing layer {:?}", path);
			None
		}
	}

	pub fn selected_visible_layers_bounding_box(&self) -> Option<[DVec2; 2]> {
		let paths = self.selected_visible_layers();
		self.graphene_document.combined_viewport_bounding_box(paths)
	}

	// TODO: Consider moving this to some kind of overlays manager in the future
	pub fn selected_visible_layers_vector_points(&self) -> Vec<VectorManipulatorShape> {
		let shapes = self.selected_layers().filter_map(|path_to_shape| {
			let viewport_transform = self.graphene_document.generate_transform_relative_to_viewport(path_to_shape).ok()?;
			let layer = self.graphene_document.layer(path_to_shape);

			// Filter out the non-visible layers from the `filter_map`
			match &layer {
				Ok(layer) if layer.visible => {}
				_ => return None,
			};

			let shape = match &layer.ok()?.data {
				LayerDataType::Shape(shape) => Some(shape),
				LayerDataType::Folder(_) => None,
			}?;
			let path = shape.path.clone();

			let place = |point: kurbo::Point| -> DVec2 { viewport_transform.transform_point2(DVec2::from((point.x, point.y))) };

			let segments = path
				.segments()
				.map(|segment| -> VectorManipulatorSegment {
					match segment {
						PathSeg::Line(line) => VectorManipulatorSegment::Line(place(line.p0), place(line.p1)),
						PathSeg::Quad(quad) => VectorManipulatorSegment::Quad(place(quad.p0), place(quad.p1), place(quad.p2)),
						PathSeg::Cubic(cubic) => VectorManipulatorSegment::Cubic(place(cubic.p0), place(cubic.p1), place(cubic.p2), place(cubic.p3)),
					}
				})
				.collect::<Vec<VectorManipulatorSegment>>();

			type IndexedEl = (usize, kurbo::PathEl);
			let create_anchor_manipulator = |first: IndexedEl, second: IndexedEl| -> VectorManipulatorAnchor {
				let mut handle1 = None;
				let mut anchor_position: glam::DVec2 = glam::DVec2::ZERO;
				let mut handle2 = None;
				let (first_id, first_element) = first;
				let (second_id, second_element) = second;

				match first_element {
					kurbo::PathEl::MoveTo(anchor) | kurbo::PathEl::LineTo(anchor) => anchor_position = place(anchor),
					kurbo::PathEl::QuadTo(handle, anchor) | kurbo::PathEl::CurveTo(_, handle, anchor) => {
						anchor_position = place(anchor);
						handle1 = Some(VectorManipulatorPoint {
							element_id: first_id,
							position: place(handle),
						});
					}
					_ => (),
				}

				match second_element {
					kurbo::PathEl::CurveTo(handle, _, _) | kurbo::PathEl::QuadTo(handle, _) => {
						handle2 = Some(VectorManipulatorPoint {
							element_id: second_id,
							position: place(handle),
						});
					}
					_ => (),
				}

				VectorManipulatorAnchor {
					point: VectorManipulatorPoint {
						element_id: first_id,
						position: anchor_position,
					},
					close_element_id: None,
					handles: (handle1, handle2),
				}
			};

			let elements = path.elements().iter().enumerate().map(|(index, element)| (index, *element)).collect::<Vec<IndexedEl>>();

			// Create the manipulation points
			let mut points: Vec<VectorManipulatorAnchor> = vec![];
			let (mut first, mut last): (Option<IndexedEl>, Option<IndexedEl>) = (None, None);
			let mut close_element_id: Option<usize> = None;
			for elements in elements.windows(2) {
				let (current_index, current_element) = elements[0];
				let (_, next_element) = elements[1];
				// TODO: Currently a unique case for [MoveTo, curveTo, ...], refactor more generally if possible
				if matches!(current_element, kurbo::PathEl::MoveTo(_)) && matches!(next_element, kurbo::PathEl::CurveTo(_, _, _)) {
					close_element_id = Some(current_index);
					continue;
				}

				if first.is_none() {
					first = Some(elements[0]);
				}
				last = Some(elements[1]);

				points.push(create_anchor_manipulator(elements[0], elements[1]));
			}

			// Close the shape
			if let (Some(first), Some(last)) = (first, last) {
				let mut element = create_anchor_manipulator(last, first);
				element.close_element_id = close_element_id;
				points.push(element);
			}

			// Useful debug
			//log::debug!("Elements {:#?}", path.elements());
			//log::debug!("Points {:#?}", points);

			Some(VectorManipulatorShape {
				layer_path: path_to_shape.to_vec(),
				path,
				segments,
				points,
				transform: viewport_transform,
				closed: shape.closed,
			})
		});

		// TODO: Consider refactoring this in a way that avoids needing to collect() so we can skip the heap allocations
		shapes.collect::<Vec<VectorManipulatorShape>>()
	}

	pub fn selected_layers(&self) -> impl Iterator<Item = &[LayerId]> {
		self.layer_metadata.iter().filter_map(|(path, data)| data.selected.then(|| path.as_slice()))
	}

	pub fn non_selected_layers(&self) -> impl Iterator<Item = &[LayerId]> {
		self.layer_metadata.iter().filter_map(|(path, data)| (!data.selected).then(|| path.as_slice()))
	}

	pub fn selected_layers_without_children(&self) -> Vec<&[LayerId]> {
		let unique_layers = GrapheneDocument::shallowest_unique_layers(self.selected_layers());

		// We need to maintain layer ordering
		self.sort_layers(unique_layers.iter().copied())
	}

	pub fn selected_layers_contains(&self, path: &[LayerId]) -> bool {
		self.layer_metadata.get(path).map(|layer| layer.selected).unwrap_or(false)
	}

	pub fn selected_visible_layers(&self) -> impl Iterator<Item = &[LayerId]> {
		self.selected_layers().filter(|path| match self.graphene_document.layer(path) {
			Ok(layer) => layer.visible,
			Err(_) => false,
		})
	}

	pub fn visible_layers(&self) -> impl Iterator<Item = &[LayerId]> {
		self.all_layers().filter(|path| match self.graphene_document.layer(path) {
			Ok(layer) => layer.visible,
			Err(_) => false,
		})
	}

	fn serialize_structure(&self, folder: &Folder, structure: &mut Vec<u64>, data: &mut Vec<LayerId>, path: &mut Vec<LayerId>) {
		let mut space = 0;
		for (id, layer) in folder.layer_ids.iter().zip(folder.layers()).rev() {
			data.push(*id);
			space += 1;
			match layer.data {
				LayerDataType::Shape(_) => (),
				LayerDataType::Folder(ref folder) => {
					path.push(*id);
					if self.layer_metadata(path).expanded {
						structure.push(space);
						self.serialize_structure(folder, structure, data, path);
						space = 0;
					}
					path.pop();
				}
			}
		}
		structure.push(space | 1 << 63);
	}

	/// Serializes the layer structure into a condensed 1D structure.
	///
	/// # Format
	/// It is a string of numbers broken into three sections:
	///
	/// | Data                                                                                                                           | Description                                  | Length           |
	/// |--------------------------------------------------------------------------------------------------------------------------------|----------------------------------------------|------------------|
	/// | `4,` `2, 1, -2, -0,` `16533113728871998040,3427872634365736244,18115028555707261608,15878401910454357952,449479075714955186`   | Encoded example data                         |                  |
	/// | `L` = `4` = `structure.len()`                                                                                                  | `L`, the length of the **Structure** section | First value      |
	/// | **Structure** section = `2, 1, -2, -0`                                                                                         | The **Structure** section                    | Next `L` values  |
	/// | **Data** section = `16533113728871998040, 3427872634365736244, 18115028555707261608, 15878401910454357952, 449479075714955186` | The **Data** section (layer IDs)             | Remaining values |
	///
	/// The data section lists the layer IDs for all folders/layers in the tree as read from top to bottom.
	/// The structure section lists signed numbers. The sign indicates a folder indentation change (`+` is down a level, `-` is up a level).
	/// The numbers in the structure block encode the indentation. For example:
	/// - `2` means read two element from the data section, then place a `[`.
	/// - `-x` means read `x` elements from the data section and then insert a `]`.
	///
	/// ```text
	/// 2     V 1  V -2  A -0 A
	/// 16533113728871998040,3427872634365736244,  18115028555707261608, 15878401910454357952,449479075714955186
	/// 16533113728871998040,3427872634365736244,[ 18115028555707261608,[15878401910454357952,449479075714955186]    ]
	/// ```
	///
	/// Resulting layer panel:
	/// ```text
	/// 16533113728871998040
	/// 3427872634365736244
	/// [3427872634365736244,18115028555707261608]
	/// [3427872634365736244,18115028555707261608,15878401910454357952]
	/// [3427872634365736244,18115028555707261608,449479075714955186]
	/// ```
	pub fn serialize_root(&self) -> Vec<u64> {
		let (mut structure, mut data) = (vec![0], Vec::new());
		self.serialize_structure(self.graphene_document.root.as_folder().unwrap(), &mut structure, &mut data, &mut vec![]);
		structure[0] = structure.len() as u64 - 1;
		structure.extend(data);

		structure
	}

	/// Returns an unsorted list of all layer paths including folders at all levels, except the document's top-level root folder itself
	pub fn all_layers(&self) -> impl Iterator<Item = &[LayerId]> {
		self.layer_metadata.keys().filter_map(|path| (!path.is_empty()).then(|| path.as_slice()))
	}

	/// Returns the paths to all layers in order
	fn sort_layers<'a>(&self, paths: impl Iterator<Item = &'a [LayerId]>) -> Vec<&'a [LayerId]> {
		// Compute the indices for each layer to be able to sort them
		let mut layers_with_indices: Vec<(&[LayerId], Vec<usize>)> = paths
			// 'path.len() > 0' filters out root layer since it has no indices
			.filter_map(|path| (!path.is_empty()).then(|| path))
			.filter_map(|path| {
				// TODO: `indices_for_path` can return an error. We currently skip these layers and log a warning. Once this problem is solved this code can be simplified.
				match self.graphene_document.indices_for_path(path) {
					Err(err) => {
						warn!("layers_sorted: Could not get indices for the layer {:?}: {:?}", path, err);
						None
					}
					Ok(indices) => Some((path, indices)),
				}
			})
			.collect();

		layers_with_indices.sort_by_key(|(_, indices)| indices.clone());
		layers_with_indices.into_iter().map(|(path, _)| path).collect()
	}

	/// Returns the paths to all layers in order
	pub fn all_layers_sorted(&self) -> Vec<&[LayerId]> {
		self.sort_layers(self.all_layers())
	}

	/// Returns the paths to all selected layers in order
	pub fn selected_layers_sorted(&self) -> Vec<&[LayerId]> {
		self.sort_layers(self.selected_layers())
	}

	/// Returns the paths to all non_selected layers in order
	#[allow(dead_code)] // used for test cases
	pub fn non_selected_layers_sorted(&self) -> Vec<&[LayerId]> {
		self.sort_layers(self.non_selected_layers())
	}

	pub fn layer_metadata(&self, path: &[LayerId]) -> &LayerMetadata {
		self.layer_metadata.get(path).unwrap_or_else(|| panic!("Editor's layer metadata for {:?} does not exist", path))
	}

	pub fn layer_metadata_mut(&mut self, path: &[LayerId]) -> &mut LayerMetadata {
		Self::layer_metadata_mut_no_borrow_self(&mut self.layer_metadata, path)
	}

	pub fn layer_metadata_mut_no_borrow_self<'a>(layer_metadata: &'a mut HashMap<Vec<LayerId>, LayerMetadata>, path: &[LayerId]) -> &'a mut LayerMetadata {
		layer_metadata
			.get_mut(path)
			.unwrap_or_else(|| panic!("Layer data cannot be found because the path {:?} does not exist", path))
	}

	pub fn backup(&mut self, responses: &mut VecDeque<Message>) {
		self.document_redo_history.clear();
		self.document_undo_history.push((self.graphene_document.clone(), self.layer_metadata.clone()));

		// Push the UpdateOpenDocumentsList message to the bus in order to update the save status of the open documents
		responses.push_back(PortfolioMessage::UpdateOpenDocumentsList.into());
	}

	pub fn rollback(&mut self, responses: &mut VecDeque<Message>) -> Result<(), EditorError> {
		self.backup(responses);
		self.undo(responses)
		// TODO: Consider if we should check if the document is saved
	}

	pub fn undo(&mut self, responses: &mut VecDeque<Message>) -> Result<(), EditorError> {
		// Push the UpdateOpenDocumentsList message to the bus in order to update the save status of the open documents
		responses.push_back(PortfolioMessage::UpdateOpenDocumentsList.into());

		match self.document_undo_history.pop() {
			Some((document, layer_metadata)) => {
				let document = std::mem::replace(&mut self.graphene_document, document);
				let layer_metadata = std::mem::replace(&mut self.layer_metadata, layer_metadata);
				self.document_redo_history.push((document, layer_metadata));
				for layer in self.layer_metadata.keys() {
					responses.push_back(DocumentMessage::LayerChanged { affected_layer_path: layer.clone() }.into())
				}
				Ok(())
			}
			None => Err(EditorError::NoTransactionInProgress),
		}
	}

	pub fn redo(&mut self, responses: &mut VecDeque<Message>) -> Result<(), EditorError> {
		// Push the UpdateOpenDocumentsList message to the bus in order to update the save status of the open documents
		responses.push_back(PortfolioMessage::UpdateOpenDocumentsList.into());

		match self.document_redo_history.pop() {
			Some((document, layer_metadata)) => {
				let document = std::mem::replace(&mut self.graphene_document, document);
				let layer_metadata = std::mem::replace(&mut self.layer_metadata, layer_metadata);
				self.document_undo_history.push((document, layer_metadata));
				for layer in self.layer_metadata.keys() {
					responses.push_back(DocumentMessage::LayerChanged { affected_layer_path: layer.clone() }.into())
				}
				Ok(())
			}
			None => Err(EditorError::NoTransactionInProgress),
		}
	}

	pub fn current_identifier(&self) -> u64 {
		// We can use the last state of the document to serve as the identifier to compare against
		// This is useful since when the document is empty the identifier will be 0
		self.document_undo_history
			.last()
			.map(|(graphene_document, _)| graphene_document.current_state_identifier())
			.unwrap_or(0)
	}

	pub fn is_saved(&self) -> bool {
		self.current_identifier() == self.saved_document_identifier
	}

	pub fn set_save_state(&mut self, is_saved: bool) {
		if is_saved {
			self.saved_document_identifier = self.current_identifier();
		} else {
			self.saved_document_identifier = generate_uuid();
		}
	}

	pub fn layer_panel_entry(&mut self, path: Vec<LayerId>) -> Result<LayerPanelEntry, EditorError> {
		let data: LayerMetadata = *self
			.layer_metadata
			.get_mut(&path)
			.ok_or_else(|| EditorError::Document(format!("Could not get layer metadata for {:?}", path)))?;
		let layer = self.graphene_document.layer(&path)?;
		let entry = layer_panel_entry(&data, self.graphene_document.multiply_transforms(&path)?, layer, path);
		Ok(entry)
	}

	/// Returns a list of `LayerPanelEntry`s intended for display purposes. These don't contain
	/// any actual data, but rather attributes such as visibility and names of the layers.
	pub fn layer_panel(&mut self, path: &[LayerId]) -> Result<Vec<LayerPanelEntry>, EditorError> {
		let folder = self.graphene_document.folder(path)?;
		let paths: Vec<Vec<LayerId>> = folder.layer_ids.iter().map(|id| [path, &[*id]].concat()).collect();
		let entries = paths.iter().rev().filter_map(|path| self.layer_panel_entry_from_path(path)).collect();
		Ok(entries)
	}

	pub fn layer_panel_entry_from_path(&self, path: &[LayerId]) -> Option<LayerPanelEntry> {
		let layer_metadata = self.layer_metadata(path);
		let transform = self
			.graphene_document
			.generate_transform_across_scope(path, Some(self.graphene_document.root.transform.inverse()))
			.ok()?;
		let layer = self.graphene_document.layer(path).ok()?;

		Some(layer_panel_entry(layer_metadata, transform, layer, path.to_vec()))
	}

	/// When working with an insert index, deleting the layers may cause the insert index to point to a different location (if the layer being deleted was located before the insert index).
	///
	/// This function updates the insert index so that it points to the same place after the specified `layers` are deleted.
	fn update_insert_index<'a>(&self, layers: &[&'a [LayerId]], path: &[LayerId], insert_index: isize, reverse_index: bool) -> Result<isize, DocumentError> {
		let folder = self.graphene_document.folder(path)?;
		let insert_index = if reverse_index { folder.layer_ids.len() as isize - insert_index } else { insert_index };
		let layer_ids_above = if insert_index < 0 { &folder.layer_ids } else { &folder.layer_ids[..(insert_index as usize)] };

		Ok(insert_index - layer_ids_above.iter().filter(|layer_id| layers.iter().any(|x| *x == [path, &[**layer_id]].concat())).count() as isize)
	}

	pub fn document_bounds(&self) -> Option<[DVec2; 2]> {
		if self.artboard_message_handler.is_infinite_canvas() {
			self.graphene_document.viewport_bounding_box(&[]).ok().flatten()
		} else {
			self.artboard_message_handler.artboards_graphene_document.viewport_bounding_box(&[]).ok().flatten()
		}
	}
}

impl MessageHandler<DocumentMessage, &InputPreprocessorMessageHandler> for DocumentMessageHandler {
	#[remain::check]
	fn process_action(&mut self, message: DocumentMessage, ipp: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		use DocumentMessage::*;

		#[remain::sorted]
		match message {
			// Sub-messages
			#[remain::unsorted]
			DispatchOperation(op) => match self.graphene_document.handle_operation(&op) {
				Ok(Some(document_responses)) => {
					for response in document_responses {
						match &response {
							DocumentResponse::FolderChanged { path } => responses.push_back(FolderChanged { affected_folder_path: path.clone() }.into()),
							DocumentResponse::DeletedLayer { path } => {
								self.layer_metadata.remove(path);
							}
							DocumentResponse::LayerChanged { path } => responses.push_back(LayerChanged { affected_layer_path: path.clone() }.into()),
							DocumentResponse::CreatedLayer { path } => {
								if self.layer_metadata.contains_key(path) {
									log::warn!("CreatedLayer overrides existing layer metadata.");
								}
								self.layer_metadata.insert(path.clone(), LayerMetadata::new(false));

								responses.push_back(LayerChanged { affected_layer_path: path.clone() }.into());
								self.layer_range_selection_reference = path.clone();
								responses.push_back(
									AddSelectedLayers {
										additional_layers: vec![path.clone()],
									}
									.into(),
								);
							}
							DocumentResponse::DocumentChanged => responses.push_back(RenderDocument.into()),
						};
						responses.push_back(ToolMessage::DocumentIsDirty.into());
					}
				}
				Err(e) => log::error!("DocumentError: {:?}", e),
				Ok(_) => (),
			},
			#[remain::unsorted]
			Artboard(message) => {
				self.artboard_message_handler.process_action(message, (), responses);
			}
			#[remain::unsorted]
			Movement(message) => {
				self.movement_handler.process_action(message, (&self.graphene_document, ipp), responses);
			}
			#[remain::unsorted]
			Overlays(message) => {
				self.overlays_message_handler.process_action(message, self.overlays_visible, responses);
				// responses.push_back(OverlaysMessage::RenderOverlays.into());
			}
			#[remain::unsorted]
			TransformLayers(message) => {
				self.transform_layer_handler
					.process_action(message, (&mut self.layer_metadata, &mut self.graphene_document, ipp), responses);
			}

			// Messages
			AbortTransaction => {
				self.undo(responses).unwrap_or_else(|e| log::warn!("{}", e));
				responses.extend([RenderDocument.into(), DocumentStructureChanged.into()]);
			}
			AddSelectedLayers { additional_layers } => {
				for layer_path in additional_layers {
					responses.extend(self.select_layer(&layer_path));
				}
				// TODO: Correctly update layer panel in clear_selection instead of here
				responses.push_back(FolderChanged { affected_folder_path: vec![] }.into());
				responses.push_back(ToolMessage::DocumentIsDirty.into());
			}
			AlignSelectedLayers { axis, aggregate } => {
				self.backup(responses);
				let (paths, boxes): (Vec<_>, Vec<_>) = self
					.selected_layers()
					.filter_map(|path| self.graphene_document.viewport_bounding_box(path).ok()?.map(|b| (path, b)))
					.unzip();

				let axis = match axis {
					AlignAxis::X => DVec2::X,
					AlignAxis::Y => DVec2::Y,
				};
				let lerp = |bbox: &[DVec2; 2]| bbox[0].lerp(bbox[1], 0.5);
				if let Some(combined_box) = self.graphene_document.combined_viewport_bounding_box(self.selected_layers()) {
					let aggregated = match aggregate {
						AlignAggregate::Min => combined_box[0],
						AlignAggregate::Max => combined_box[1],
						AlignAggregate::Center => lerp(&combined_box),
						AlignAggregate::Average => boxes.iter().map(|b| lerp(b)).reduce(|a, b| a + b).map(|b| b / boxes.len() as f64).unwrap(),
					};
					for (path, bbox) in paths.into_iter().zip(boxes) {
						let center = match aggregate {
							AlignAggregate::Min => bbox[0],
							AlignAggregate::Max => bbox[1],
							_ => lerp(&bbox),
						};
						let translation = (aggregated - center) * axis;
						responses.push_back(
							DocumentOperation::TransformLayerInViewport {
								path: path.to_vec(),
								transform: DAffine2::from_translation(translation).to_cols_array(),
							}
							.into(),
						);
					}
					responses.push_back(ToolMessage::DocumentIsDirty.into());
				}
			}
			CommitTransaction => (),
			CreateEmptyFolder { mut container_path } => {
				let id = generate_uuid();
				container_path.push(id);
				responses.push_back(DocumentOperation::CreateFolder { path: container_path.clone() }.into());
				responses.push_back(
					DocumentMessage::SetLayerExpansion {
						layer_path: container_path,
						set_expanded: true,
					}
					.into(),
				);
			}
			DebugPrintDocument => {
				log::debug!("{:#?}\n{:#?}", self.graphene_document, self.layer_metadata);
			}
			DeleteLayer { layer_path } => responses.push_front(DocumentOperation::DeleteLayer { path: layer_path }.into()),
			DeleteSelectedLayers => {
				self.backup(responses);

				for path in self.selected_layers_without_children() {
					responses.push_front(DocumentOperation::DeleteLayer { path: path.to_vec() }.into());
				}

				responses.push_front(ToolMessage::DocumentIsDirty.into());
			}
			DeselectAllLayers => {
				responses.push_front(SetSelectedLayers { replacement_selected_layers: vec![] }.into());
				self.layer_range_selection_reference.clear();
			}
			DirtyRenderDocument => {
				// Mark all non-overlay caches as dirty
				GrapheneDocument::visit_all_shapes(&mut self.graphene_document.root, &mut |_| {});

				responses.push_back(DocumentMessage::RenderDocument.into());
			}
			DirtyRenderDocumentInOutlineView => {
				if self.view_mode == ViewMode::Outline {
					responses.push_front(DocumentMessage::DirtyRenderDocument.into());
				}
			}
			DocumentHistoryBackward => self.undo(responses).unwrap_or_else(|e| log::warn!("{}", e)),
			DocumentHistoryForward => self.redo(responses).unwrap_or_else(|e| log::warn!("{}", e)),
			DocumentStructureChanged => {
				let data_buffer: RawBuffer = self.serialize_root().into();
				responses.push_back(FrontendMessage::DisplayDocumentLayerTreeStructure { data_buffer }.into())
			}
			DuplicateSelectedLayers => {
				self.backup(responses);
				for path in self.selected_layers_sorted() {
					responses.push_back(DocumentOperation::DuplicateLayer { path: path.to_vec() }.into());
				}
			}
			ExportDocument => {
				// TODO(MFISH33): Add Dialog to select artboards
				let bbox = self.document_bounds().unwrap_or_else(|| [DVec2::ZERO, ipp.viewport_bounds.size()]);
				let size = bbox[1] - bbox[0];
				let name = match self.name.ends_with(FILE_SAVE_SUFFIX) {
					true => self.name.clone().replace(FILE_SAVE_SUFFIX, FILE_EXPORT_SUFFIX),
					false => self.name.clone() + FILE_EXPORT_SUFFIX,
				};
				responses.push_back(
					FrontendMessage::TriggerFileDownload {
						document: format!(
							r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {} {}">{}{}</svg>"#,
							bbox[0].x,
							bbox[0].y,
							size.x,
							size.y,
							"\n",
							self.graphene_document.render_root(self.view_mode)
						),
						name,
					}
					.into(),
				)
			}
			FlipSelectedLayers { flip_axis } => {
				self.backup(responses);
				let scale = match flip_axis {
					FlipAxis::X => DVec2::new(-1., 1.),
					FlipAxis::Y => DVec2::new(1., -1.),
				};
				if let Some([min, max]) = self.graphene_document.combined_viewport_bounding_box(self.selected_layers()) {
					let center = (max + min) / 2.;
					let bbox_trans = DAffine2::from_translation(-center);
					for path in self.selected_layers() {
						responses.push_back(
							DocumentOperation::TransformLayerInScope {
								path: path.to_vec(),
								transform: DAffine2::from_scale(scale).to_cols_array(),
								scope: bbox_trans.to_cols_array(),
							}
							.into(),
						);
					}
					responses.push_back(ToolMessage::DocumentIsDirty.into());
				}
			}
			FolderChanged { affected_folder_path } => {
				let _ = self.graphene_document.render_root(self.view_mode);
				let affected_layer_path = affected_folder_path;
				responses.extend([LayerChanged { affected_layer_path }.into(), DocumentStructureChanged.into()]);
			}
			GroupSelectedLayers => {
				let mut new_folder_path: Vec<u64> = self.graphene_document.shallowest_common_folder(self.selected_layers()).unwrap_or(&[]).to_vec();

				// Required for grouping parent folders with their own children
				if !new_folder_path.is_empty() && self.selected_layers_contains(&new_folder_path) {
					new_folder_path.remove(new_folder_path.len() - 1);
				}

				new_folder_path.push(generate_uuid());

				responses.push_back(PortfolioMessage::Copy { clipboard: Clipboard::System }.into());
				responses.push_back(DocumentMessage::DeleteSelectedLayers.into());
				responses.push_back(DocumentOperation::CreateFolder { path: new_folder_path.clone() }.into());
				responses.push_back(DocumentMessage::ToggleLayerExpansion { layer_path: new_folder_path.clone() }.into());
				responses.push_back(
					PortfolioMessage::PasteIntoFolder {
						clipboard: Clipboard::System,
						folder_path: new_folder_path.clone(),
						insert_index: -1,
					}
					.into(),
				);
				responses.push_back(
					DocumentMessage::SetSelectedLayers {
						replacement_selected_layers: vec![new_folder_path],
					}
					.into(),
				);
			}
			LayerChanged { affected_layer_path } => {
				if let Ok(layer_entry) = self.layer_panel_entry(affected_layer_path) {
					responses.push_back(FrontendMessage::UpdateDocumentLayer { data: layer_entry }.into());
				}
			}
			MoveSelectedLayersTo {
				folder_path,
				insert_index,
				reverse_index,
			} => {
				let selected_layers = self.selected_layers().collect::<Vec<_>>();

				// Prevent trying to insert into self
				if selected_layers.iter().any(|layer| folder_path.starts_with(layer)) {
					return;
				}

				let insert_index = self.update_insert_index(&selected_layers, &folder_path, insert_index, reverse_index).unwrap();

				responses.push_back(PortfolioMessage::Copy { clipboard: Clipboard::System }.into());
				responses.push_back(DocumentMessage::DeleteSelectedLayers.into());
				responses.push_back(
					PortfolioMessage::PasteIntoFolder {
						clipboard: Clipboard::System,
						folder_path,
						insert_index,
					}
					.into(),
				);
			}
			NudgeSelectedLayers { delta_x, delta_y } => {
				self.backup(responses);
				for path in self.selected_layers().map(|path| path.to_vec()) {
					let operation = DocumentOperation::TransformLayerInViewport {
						path,
						transform: DAffine2::from_translation((delta_x, delta_y).into()).to_cols_array(),
					};
					responses.push_back(operation.into());
				}
				responses.push_back(ToolMessage::DocumentIsDirty.into());
			}
			Redo => {
				responses.push_back(SelectMessage::Abort.into());
				responses.push_back(DocumentHistoryForward.into());
				responses.push_back(ToolMessage::DocumentIsDirty.into());
				responses.push_back(RenderDocument.into());
				responses.push_back(FolderChanged { affected_folder_path: vec![] }.into());
			}
			RenameLayer { layer_path, new_name } => responses.push_back(DocumentOperation::RenameLayer { layer_path, new_name }.into()),
			RenderDocument => {
				responses.push_back(
					FrontendMessage::UpdateDocumentArtwork {
						svg: self.graphene_document.render_root(self.view_mode),
					}
					.into(),
				);
				responses.push_back(ArtboardMessage::RenderArtboards.into());

				let document_transform_scale = self.movement_handler.snapped_scale();
				let scale = 0.5 + ASYMPTOTIC_EFFECT + document_transform_scale * SCALE_EFFECT;
				let viewport_size = ipp.viewport_bounds.size();
				let viewport_mid = ipp.viewport_bounds.center();
				let [bounds1, bounds2] = self.document_bounds().unwrap_or([viewport_mid; 2]);
				let bounds1 = bounds1.min(viewport_mid) - viewport_size * scale;
				let bounds2 = bounds2.max(viewport_mid) + viewport_size * scale;
				let bounds_length = (bounds2 - bounds1) * (1. + SCROLLBAR_SPACING);
				let scrollbar_position = DVec2::splat(0.5) - (bounds1.lerp(bounds2, 0.5) - viewport_mid) / (bounds_length - viewport_size);
				let scrollbar_multiplier = bounds_length - viewport_size;
				let scrollbar_size = viewport_size / bounds_length;

				let log = document_transform_scale.log2();
				let ruler_interval = if log < 0. { 100. * 2_f64.powf(-log.ceil()) } else { 100. / 2_f64.powf(log.ceil()) };
				let ruler_spacing = ruler_interval * document_transform_scale;

				let ruler_origin = self.graphene_document.root.transform.transform_point2(DVec2::ZERO);

				responses.push_back(
					FrontendMessage::UpdateDocumentScrollbars {
						position: scrollbar_position.into(),
						size: scrollbar_size.into(),
						multiplier: scrollbar_multiplier.into(),
					}
					.into(),
				);

				responses.push_back(
					FrontendMessage::UpdateDocumentRulers {
						origin: ruler_origin.into(),
						spacing: ruler_spacing,
						interval: ruler_interval,
					}
					.into(),
				);
			}
			ReorderSelectedLayers { relative_index_offset } => {
				self.backup(responses);

				let all_layer_paths = self.all_layers_sorted();
				let selected_layers = self.selected_layers_sorted();

				let first_or_last_selected_layer = match relative_index_offset.signum() {
					-1 => selected_layers.first(),
					1 => selected_layers.last(),
					_ => panic!("ReorderSelectedLayers must be given a non-zero value"),
				};

				if let Some(pivot_layer) = first_or_last_selected_layer {
					let sibling_layer_paths: Vec<_> = all_layer_paths
						.iter()
						.filter(|layer| {
							// Check if this is a sibling of the pivot layer
							// TODO: Break this out into a reusable function `fn are_layers_siblings(layer_a, layer_b) -> bool`
							let containing_folder_path = &pivot_layer[0..pivot_layer.len() - 1];
							layer.starts_with(containing_folder_path) && pivot_layer.len() == layer.len()
						})
						.collect();

					// TODO: Break this out into a reusable function: `fn layer_index_in_containing_folder(layer_path) -> usize`
					let pivot_index_among_siblings = sibling_layer_paths.iter().position(|path| *path == pivot_layer);

					if let Some(pivot_index) = pivot_index_among_siblings {
						let max = sibling_layer_paths.len() as i64 - 1;
						let insert_index = (pivot_index as i64 + relative_index_offset as i64).clamp(0, max) as usize;

						let existing_layer_to_insert_beside = sibling_layer_paths.get(insert_index);

						// TODO: Break this block out into a call to a message called `MoveSelectedLayersNextToLayer { neighbor_path, above_or_below }`
						if let Some(neighbor_path) = existing_layer_to_insert_beside {
							let (neighbor_id, folder_path) = neighbor_path.split_last().expect("Can't move the root folder");

							if let Some(folder) = self.graphene_document.layer(folder_path).ok().and_then(|layer| layer.as_folder().ok()) {
								let neighbor_layer_index = folder.layer_ids.iter().position(|id| id == neighbor_id).unwrap() as isize;

								// If moving down, insert below this layer. If moving up, insert above this layer.
								let insert_index = if relative_index_offset < 0 { neighbor_layer_index } else { neighbor_layer_index + 1 };

								responses.push_back(
									DocumentMessage::MoveSelectedLayersTo {
										folder_path: folder_path.to_vec(),
										insert_index,
										reverse_index: false,
									}
									.into(),
								);
							}
						}
					}
				}
			}
			RollbackTransaction => {
				self.rollback(responses).unwrap_or_else(|e| log::warn!("{}", e));
				responses.extend([RenderDocument.into(), DocumentStructureChanged.into()]);
			}
			SaveDocument => {
				self.set_save_state(true);
				responses.push_back(PortfolioMessage::AutoSaveActiveDocument.into());
				// Update the save status of the just saved document
				responses.push_back(PortfolioMessage::UpdateOpenDocumentsList.into());

				let name = match self.name.ends_with(FILE_SAVE_SUFFIX) {
					true => self.name.clone(),
					false => self.name.clone() + FILE_SAVE_SUFFIX,
				};
				responses.push_back(
					FrontendMessage::TriggerFileDownload {
						document: self.serialize_document(),
						name,
					}
					.into(),
				)
			}
			SelectAllLayers => {
				let all = self.all_layers().map(|path| path.to_vec()).collect();
				responses.push_front(SetSelectedLayers { replacement_selected_layers: all }.into());
			}
			SelectionChanged => {
				// TODO: Hoist this duplicated code into wider system
				responses.push_back(ToolMessage::DocumentIsDirty.into());
			}
			SelectLayer { layer_path, ctrl, shift } => {
				let mut paths = vec![];
				let last_selection_exists = !self.layer_range_selection_reference.is_empty();

				// If we have shift pressed and a layer already selected then fill the range
				if shift && last_selection_exists {
					// Fill the selection range
					self.layer_metadata
						.iter()
						.filter(|(target, _)| self.graphene_document.layer_is_between(target, &layer_path, &self.layer_range_selection_reference))
						.for_each(|(layer_path, _)| {
							paths.push(layer_path.clone());
						});
				} else {
					if ctrl {
						// Toggle selection when holding ctrl
						let layer = self.layer_metadata_mut(&layer_path);
						layer.selected = !layer.selected;
						responses.push_back(
							LayerChanged {
								affected_layer_path: layer_path.clone(),
							}
							.into(),
						);
						responses.push_back(ToolMessage::DocumentIsDirty.into());
					} else {
						paths.push(layer_path.clone());
					}

					// Set our last selection reference
					self.layer_range_selection_reference = layer_path;
				}

				// Don't create messages for empty operations
				if !paths.is_empty() {
					// Add or set our selected layers
					if ctrl {
						responses.push_front(AddSelectedLayers { additional_layers: paths }.into());
					} else {
						responses.push_front(SetSelectedLayers { replacement_selected_layers: paths }.into());
					}
				}
			}
			SetBlendModeForSelectedLayers { blend_mode } => {
				self.backup(responses);
				for path in self.layer_metadata.iter().filter_map(|(path, data)| data.selected.then(|| path.clone())) {
					responses.push_back(DocumentOperation::SetLayerBlendMode { path, blend_mode }.into());
				}
			}
			SetLayerExpansion { layer_path, set_expanded } => {
				self.layer_metadata_mut(&layer_path).expanded = set_expanded;
				responses.push_back(DocumentStructureChanged.into());
				responses.push_back(LayerChanged { affected_layer_path: layer_path }.into())
			}
			SetOpacityForSelectedLayers { opacity } => {
				self.backup(responses);
				let opacity = opacity.clamp(0., 1.);

				for path in self.selected_layers().map(|path| path.to_vec()) {
					responses.push_back(DocumentOperation::SetLayerOpacity { path, opacity }.into());
				}
			}
			SetOverlaysVisibility { visible } => {
				self.overlays_visible = visible;
				responses.push_back(OverlaysMessage::Rerender.into());
			}
			SetSelectedLayers { replacement_selected_layers } => {
				let selected = self.layer_metadata.iter_mut().filter(|(_, layer_metadata)| layer_metadata.selected);
				selected.for_each(|(path, layer_metadata)| {
					layer_metadata.selected = false;
					responses.push_back(LayerChanged { affected_layer_path: path.clone() }.into())
				});

				let additional_layers = replacement_selected_layers;
				responses.push_front(AddSelectedLayers { additional_layers }.into());
			}
			SetSnapping { snap } => {
				self.snapping_enabled = snap;
			}
			SetViewMode { view_mode } => {
				self.view_mode = view_mode;
				responses.push_front(DocumentMessage::DirtyRenderDocument.into());
			}
			StartTransaction => self.backup(responses),
			ToggleLayerExpansion { layer_path } => {
				self.layer_metadata_mut(&layer_path).expanded ^= true;
				responses.push_back(DocumentStructureChanged.into());
				responses.push_back(LayerChanged { affected_layer_path: layer_path }.into())
			}
			ToggleLayerVisibility { layer_path } => {
				responses.push_back(DocumentOperation::ToggleLayerVisibility { path: layer_path }.into());
				responses.push_back(ToolMessage::DocumentIsDirty.into());
			}
			Undo => {
				responses.push_back(SelectMessage::Abort.into());
				responses.push_back(DocumentHistoryBackward.into());
				responses.push_back(ToolMessage::DocumentIsDirty.into());
				responses.push_back(RenderDocument.into());
				responses.push_back(FolderChanged { affected_folder_path: vec![] }.into());
			}
			UngroupLayers { folder_path } => {
				// Select all the children of the folder
				let select = self.graphene_document.folder_children_paths(&folder_path);

				let message_buffer = [
					// Select them
					DocumentMessage::SetSelectedLayers { replacement_selected_layers: select }.into(),
					// Copy them
					PortfolioMessage::Copy { clipboard: Clipboard::System }.into(),
					// Paste them into the folder above
					PortfolioMessage::PasteIntoFolder {
						clipboard: Clipboard::System,
						folder_path: folder_path[..folder_path.len() - 1].to_vec(),
						insert_index: -1,
					}
					.into(),
					// Delete the parent folder
					DocumentMessage::DeleteLayer { layer_path: folder_path }.into(),
				];

				// Push these messages in reverse due to push_front
				for message in message_buffer.into_iter().rev() {
					responses.push_front(message);
				}
			}
			UngroupSelectedLayers => {
				responses.push_back(DocumentMessage::StartTransaction.into());
				let folder_paths = self.graphene_document.sorted_folders_by_depth(self.selected_layers());
				for folder_path in folder_paths {
					responses.push_back(DocumentMessage::UngroupLayers { folder_path: folder_path.to_vec() }.into());
				}
				responses.push_back(DocumentMessage::CommitTransaction.into());
			}
			UpdateLayerMetadata { layer_path, layer_metadata } => {
				self.layer_metadata.insert(layer_path, layer_metadata);
			}
			ZoomCanvasToFitAll => {
				if let Some(bounds) = self.document_bounds() {
					responses.push_back(
						MovementMessage::FitViewportToBounds {
							bounds,
							padding_scale_factor: Some(VIEWPORT_ZOOM_TO_FIT_PADDING_SCALE_FACTOR),
							prevent_zoom_past_100: true,
						}
						.into(),
					)
				}
			}
		}
	}

	fn actions(&self) -> ActionList {
		let mut common = actions!(DocumentMessageDiscriminant;
			Undo,
			Redo,
			SelectAllLayers,
			DeselectAllLayers,
			RenderDocument,
			ExportDocument,
			SaveDocument,
			SetSnapping,
			DebugPrintDocument,
			ZoomCanvasToFitAll,
		);

		if self.layer_metadata.values().any(|data| data.selected) {
			let select = actions!(DocumentMessageDiscriminant;
				DeleteSelectedLayers,
				DuplicateSelectedLayers,
				NudgeSelectedLayers,
				ReorderSelectedLayers,
				GroupSelectedLayers,
				UngroupSelectedLayers,
			);
			common.extend(select);
		}
		common.extend(self.movement_handler.actions());
		common.extend(self.transform_layer_handler.actions());
		common
	}
}

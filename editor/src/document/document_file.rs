use std::collections::HashMap;
use std::collections::VecDeque;

pub use super::layer_panel::*;
use super::movement_handler::{MovementMessage, MovementMessageHandler};
use super::overlay_message_handler::OverlayMessageHandler;
use super::transform_layer_handler::{TransformLayerMessage, TransformLayerMessageHandler};
use super::vectorize_layer_metadata;

use crate::consts::DEFAULT_DOCUMENT_NAME;
use crate::consts::{ASYMPTOTIC_EFFECT, FILE_EXPORT_SUFFIX, FILE_SAVE_SUFFIX, SCALE_EFFECT, SCROLLBAR_SPACING};
use crate::document::Clipboard;
use crate::input::InputPreprocessor;
use crate::message_prelude::*;
use crate::EditorError;

use graphene::layers::{style::ViewMode, BlendMode, LayerDataType};
use graphene::{document::Document as GrapheneDocument, DocumentError, LayerId};
use graphene::{DocumentResponse, Operation as DocumentOperation};

use glam::{DAffine2, DVec2};
use graphene::layers::Folder;
use kurbo::PathSeg;
use log::warn;
use serde::{Deserialize, Serialize};

type DocumentSave = (GrapheneDocument, HashMap<Vec<LayerId>, LayerMetadata>);

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, Hash)]
pub enum FlipAxis {
	X,
	Y,
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, Hash)]
pub enum AlignAxis {
	X,
	Y,
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, Hash)]
pub enum AlignAggregate {
	Min,
	Max,
	Center,
	Average,
}

#[derive(PartialEq, Clone, Debug)]
pub enum VectorManipulatorSegment {
	Line(DVec2, DVec2),
	Quad(DVec2, DVec2, DVec2),
	Cubic(DVec2, DVec2, DVec2, DVec2),
}

#[derive(PartialEq, Clone, Debug)]
pub struct VectorManipulatorShape {
	pub path: kurbo::BezPath,
	pub segments: Vec<VectorManipulatorSegment>,
	pub transform: DAffine2,
}

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
	#[serde(skip)]
	movement_handler: MovementMessageHandler,
	#[serde(skip)]
	overlay_message_handler: OverlayMessageHandler,
	#[serde(skip)]
	transform_layer_handler: TransformLayerMessageHandler,
	pub snapping_enabled: bool,
	pub view_mode: ViewMode,
}

impl Default for DocumentMessageHandler {
	fn default() -> Self {
		Self {
			graphene_document: GrapheneDocument::default(),
			document_undo_history: Vec::new(),
			document_redo_history: Vec::new(),
			name: String::from("Untitled Document"),
			saved_document_identifier: 0,
			layer_metadata: vec![(vec![], LayerMetadata::new(true))].into_iter().collect(),
			layer_range_selection_reference: Vec::new(),
			movement_handler: MovementMessageHandler::default(),
			overlay_message_handler: OverlayMessageHandler::default(),
			transform_layer_handler: TransformLayerMessageHandler::default(),
			snapping_enabled: true,
			view_mode: ViewMode::default(),
		}
	}
}

#[impl_message(Message, DocumentsMessage, Document)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum DocumentMessage {
	#[child]
	Movement(MovementMessage),
	#[child]
	TransformLayers(TransformLayerMessage),
	DispatchOperation(Box<DocumentOperation>),
	#[child]
	Overlay(OverlayMessage),
	UpdateLayerMetadata {
		layer_path: Vec<LayerId>,
		layer_metadata: LayerMetadata,
	},
	SetSelectedLayers(Vec<Vec<LayerId>>),
	AddSelectedLayers(Vec<Vec<LayerId>>),
	SelectAllLayers,
	DebugPrintDocument,
	SelectLayer(Vec<LayerId>, bool, bool),
	SelectionChanged,
	DeselectAllLayers,
	DeleteLayer(Vec<LayerId>),
	DeleteSelectedLayers,
	DuplicateSelectedLayers,
	CreateEmptyFolder(Vec<LayerId>),
	SetBlendModeForSelectedLayers(BlendMode),
	SetOpacityForSelectedLayers(f64),
	RenameLayer(Vec<LayerId>, String),
	ToggleLayerVisibility(Vec<LayerId>),
	FlipSelectedLayers(FlipAxis),
	ToggleLayerExpansion(Vec<LayerId>),
	SetLayerExpansion(Vec<LayerId>, bool),
	FolderChanged(Vec<LayerId>),
	LayerChanged(Vec<LayerId>),
	DocumentStructureChanged,
	StartTransaction,
	RollbackTransaction,
	GroupSelectedLayers,
	AbortTransaction,
	CommitTransaction,
	ExportDocument,
	SaveDocument,
	RenderDocument,
	DirtyRenderDocument,
	DirtyRenderDocumentInOutlineView,
	SetViewMode(ViewMode),
	Undo,
	Redo,
	DocumentHistoryBackward,
	DocumentHistoryForward,
	NudgeSelectedLayers(f64, f64),
	AlignSelectedLayers(AlignAxis, AlignAggregate),
	MoveSelectedLayersTo {
		path: Vec<LayerId>,
		insert_index: isize,
	},
	ReorderSelectedLayers(i32), // relative_position,
	MoveLayerInTree {
		layer: Vec<LayerId>,
		insert_above: bool,
		neighbor: Vec<LayerId>,
	},
	SetSnapping(bool),
}

impl From<DocumentOperation> for DocumentMessage {
	fn from(operation: DocumentOperation) -> DocumentMessage {
		Self::DispatchOperation(Box::new(operation))
	}
}

impl From<DocumentOperation> for Message {
	fn from(operation: DocumentOperation) -> Message {
		DocumentMessage::DispatchOperation(Box::new(operation)).into()
	}
}

impl DocumentMessageHandler {
	pub fn serialize_document(&self) -> String {
		let val = serde_json::to_string(self);
		// We fully expect the serialization to succeed
		val.unwrap()
	}

	pub fn deserialize_document(serialized_content: &str) -> Result<Self, DocumentError> {
		log::info!("Deserializing: {:?}", serialized_content);
		serde_json::from_str(serialized_content).map_err(|e| DocumentError::InvalidFile(e.to_string()))
	}

	pub fn with_name(name: String, ipp: &InputPreprocessor) -> Self {
		let mut document = Self { name, ..Self::default() };
		document.graphene_document.root.transform = document.movement_handler.calculate_offset_transform(ipp.viewport_bounds.size() / 2.);
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

		self.layer_metadata_mut(path).selected = true;
		let data = self.layer_panel_entry(path.to_vec()).ok()?;
		(!path.is_empty()).then(|| FrontendMessage::UpdateLayer { data }.into())
	}

	pub fn selected_visible_layers_bounding_box(&self) -> Option<[DVec2; 2]> {
		let paths = self.selected_visible_layers();
		self.graphene_document.combined_viewport_bounding_box(paths)
	}

	// TODO: Consider moving this to some kind of overlay manager in the future
	pub fn selected_visible_layers_vector_points(&self) -> Vec<VectorManipulatorShape> {
		let shapes = self.selected_layers().filter_map(|path_to_shape| {
			let viewport_transform = self.graphene_document.generate_transform_relative_to_viewport(path_to_shape).ok()?;
			let layer = self.graphene_document.layer(path_to_shape);

			// Filter out the non-visible layers from the filter_map
			match &layer {
				Ok(layer) if layer.visible => {}
				_ => return None,
			};

			let shape = match &layer.ok()?.data {
				LayerDataType::Shape(shape) => Some(shape),
				LayerDataType::Folder(_) => None,
			}?;
			let path = shape.path.clone();

			let segments = path
				.segments()
				.map(|segment| -> VectorManipulatorSegment {
					let place = |point: kurbo::Point| -> DVec2 { viewport_transform.transform_point2(DVec2::from((point.x, point.y))) };

					match segment {
						PathSeg::Line(line) => VectorManipulatorSegment::Line(place(line.p0), place(line.p1)),
						PathSeg::Quad(quad) => VectorManipulatorSegment::Quad(place(quad.p0), place(quad.p1), place(quad.p2)),
						PathSeg::Cubic(cubic) => VectorManipulatorSegment::Cubic(place(cubic.p0), place(cubic.p1), place(cubic.p2), place(cubic.p3)),
					}
				})
				.collect::<Vec<VectorManipulatorSegment>>();

			Some(VectorManipulatorShape {
				path,
				segments,
				transform: viewport_transform,
			})
		});

		// TODO: Consider refactoring this in a way that avoids needing to collect() so we can skip the heap allocations
		shapes.collect::<Vec<VectorManipulatorShape>>()
	}

	pub fn selected_layers(&self) -> impl Iterator<Item = &[LayerId]> {
		self.layer_metadata.iter().filter_map(|(path, data)| data.selected.then(|| path.as_slice()))
	}

	pub fn selected_layers_without_children(&self) -> Vec<&[LayerId]> {
		let mut sorted_layers = self.selected_layers().collect::<Vec<_>>();
		// Sorting here creates groups of similar UUID paths
		sorted_layers.sort();
		sorted_layers.dedup_by(|a, b| a.starts_with(b));

		// We need to maintain layer ordering
		self.sort_layers(sorted_layers.iter().copied())
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

	fn serialize_structure(&self, folder: &Folder, structure: &mut Vec<u64>, data: &mut Vec<LayerId>, path: &mut Vec<LayerId>) {
		let mut space = 0;
		for (id, layer) in folder.layer_ids.iter().zip(folder.layers()) {
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

	/// Serializes the layer structure into a compressed 1d structure
	///
	/// It is a string of numbers broken into three sections:
	/// (4),(2,1,-2,-0),(16533113728871998040,3427872634365736244,18115028555707261608,15878401910454357952,449479075714955186) <- Example encoded data
	/// L = 4 = structure.len()                                                                                                 <- First value in the encoding: L, the length of the structure section
	/// structure = 2,1,-2,-0                                                                                                   <- Subsequent L values: structure section
	/// data = 16533113728871998040,3427872634365736244,18115028555707261608,15878401910454357952,449479075714955186            <- Remaining values: data section (layer IDs)
	///
	/// The data section lists the layer IDs for all folders/layers in the tree as read from top to bottom.
	/// The structure section lists signed numbers. The sign indicates a folder indentation change (+ is down a level, - is up a level).
	/// the numbers in the structure block encode the indentation,
	/// 2 mean read two element from the data section, then place a [
	/// -x means read x elements from the data section and then insert a ]
	///
	/// 2     V 1  V -2  A -0 A
	/// 16533113728871998040,3427872634365736244,  18115028555707261608, 15878401910454357952,449479075714955186
	/// 16533113728871998040,3427872634365736244,[ 18115028555707261608,[15878401910454357952,449479075714955186]    ]
	///
	/// resulting layer panel:
	/// 16533113728871998040
	/// 3427872634365736244
	/// [3427872634365736244,18115028555707261608]
	/// [3427872634365736244,18115028555707261608,15878401910454357952]
	/// [3427872634365736244,18115028555707261608,449479075714955186]
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

	/// Returns the paths to all layers in order, optionally including only selected or non-selected layers.
	fn sort_layers<'a>(&self, paths: impl Iterator<Item = &'a [LayerId]>) -> Vec<&'a [LayerId]> {
		// Compute the indices for each layer to be able to sort them
		let mut layers_with_indices: Vec<(&[LayerId], Vec<usize>)> = paths
			// 'path.len() > 0' filters out root layer since it has no indices
			.filter_map(|path| (!path.is_empty()).then(|| path))
			.filter_map(|path| {
				// TODO: `indices_for_path` can return an error. We currently skip these layers and log a warning. Once this problem is solved this code can be simplified.
				match self.graphene_document.indices_for_path(&path) {
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
		self.sort_layers(self.all_layers().filter(|layer| self.selected_layers().find(|path| path == layer).is_none()))
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
		responses.push_back(DocumentsMessage::UpdateOpenDocumentsList.into());
	}

	pub fn rollback(&mut self, responses: &mut VecDeque<Message>) -> Result<(), EditorError> {
		self.backup(responses);
		self.undo(responses)
		// TODO: Consider if we should check if the document is saved
	}

	pub fn undo(&mut self, responses: &mut VecDeque<Message>) -> Result<(), EditorError> {
		// Push the UpdateOpenDocumentsList message to the bus in order to update the save status of the open documents
		responses.push_back(DocumentsMessage::UpdateOpenDocumentsList.into());

		match self.document_undo_history.pop() {
			Some((document, layer_metadata)) => {
				let document = std::mem::replace(&mut self.graphene_document, document);
				let layer_metadata = std::mem::replace(&mut self.layer_metadata, layer_metadata);
				self.document_redo_history.push((document, layer_metadata));
				Ok(())
			}
			None => Err(EditorError::NoTransactionInProgress),
		}
	}

	pub fn redo(&mut self, responses: &mut VecDeque<Message>) -> Result<(), EditorError> {
		// Push the UpdateOpenDocumentsList message to the bus in order to update the save status of the open documents
		responses.push_back(DocumentsMessage::UpdateOpenDocumentsList.into());

		match self.document_redo_history.pop() {
			Some((document, layer_metadata)) => {
				let document = std::mem::replace(&mut self.graphene_document, document);
				let layer_metadata = std::mem::replace(&mut self.layer_metadata, layer_metadata);
				self.document_undo_history.push((document, layer_metadata));
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
		let data: LayerMetadata = *self.layer_metadata_mut(&path);
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
}

impl MessageHandler<DocumentMessage, &InputPreprocessor> for DocumentMessageHandler {
	fn process_action(&mut self, message: DocumentMessage, ipp: &InputPreprocessor, responses: &mut VecDeque<Message>) {
		use DocumentMessage::*;
		match message {
			Movement(message) => self.movement_handler.process_action(message, (&self.graphene_document, ipp), responses),
			TransformLayers(message) => self
				.transform_layer_handler
				.process_action(message, (&mut self.layer_metadata, &mut self.graphene_document, ipp), responses),
			DeleteLayer(path) => responses.push_back(DocumentOperation::DeleteLayer { path }.into()),
			StartTransaction => self.backup(responses),
			RollbackTransaction => {
				self.rollback(responses).unwrap_or_else(|e| log::warn!("{}", e));
				responses.extend([RenderDocument.into(), DocumentStructureChanged.into()]);
			}
			AbortTransaction => {
				self.undo(responses).unwrap_or_else(|e| log::warn!("{}", e));
				responses.extend([RenderDocument.into(), DocumentStructureChanged.into()]);
			}
			CommitTransaction => (),
			Overlay(message) => {
				self.overlay_message_handler.process_action(
					message,
					(Self::layer_metadata_mut_no_borrow_self(&mut self.layer_metadata, &[]), &self.graphene_document, ipp),
					responses,
				);
				// responses.push_back(OverlayMessage::RenderOverlays.into());
			}
			ExportDocument => {
				let bbox = self.graphene_document.visible_layers_bounding_box().unwrap_or([DVec2::ZERO, ipp.viewport_bounds.size()]);
				let size = bbox[1] - bbox[0];
				let name = match self.name.ends_with(FILE_SAVE_SUFFIX) {
					true => self.name.clone().replace(FILE_SAVE_SUFFIX, FILE_EXPORT_SUFFIX),
					false => self.name.clone() + FILE_EXPORT_SUFFIX,
				};
				responses.push_back(
					FrontendMessage::ExportDocument {
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
			SaveDocument => {
				self.set_save_state(true);
				responses.push_back(DocumentsMessage::AutoSaveActiveDocument.into());
				// Update the save status of the just saved document
				responses.push_back(DocumentsMessage::UpdateOpenDocumentsList.into());

				let name = match self.name.ends_with(FILE_SAVE_SUFFIX) {
					true => self.name.clone(),
					false => self.name.clone() + FILE_SAVE_SUFFIX,
				};
				responses.push_back(
					FrontendMessage::SaveDocument {
						document: self.serialize_document(),
						name,
					}
					.into(),
				)
			}
			CreateEmptyFolder(mut path) => {
				let id = generate_uuid();
				path.push(id);
				responses.push_back(DocumentOperation::CreateFolder { path: path.clone() }.into());
				responses.push_back(DocumentMessage::SetLayerExpansion(path, true).into());
			}
			GroupSelectedLayers => {
				let mut new_folder_path: Vec<u64> = self.graphene_document.shallowest_common_folder(self.selected_layers()).unwrap_or(&[]).to_vec();

				// Required for grouping parent folders with their own children
				if !new_folder_path.is_empty() && self.selected_layers_contains(&new_folder_path) {
					new_folder_path.remove(new_folder_path.len() - 1);
				}

				new_folder_path.push(generate_uuid());

				responses.push_back(DocumentsMessage::Copy(Clipboard::System).into());
				responses.push_back(DocumentMessage::DeleteSelectedLayers.into());
				responses.push_back(DocumentOperation::CreateFolder { path: new_folder_path.clone() }.into());
				responses.push_back(DocumentMessage::ToggleLayerExpansion(new_folder_path.clone()).into());
				responses.push_back(
					DocumentsMessage::PasteIntoFolder {
						clipboard: Clipboard::System,
						path: new_folder_path.clone(),
						insert_index: -1,
					}
					.into(),
				);
				responses.push_back(DocumentMessage::SetSelectedLayers(vec![new_folder_path]).into());
			}
			SetBlendModeForSelectedLayers(blend_mode) => {
				self.backup(responses);
				for path in self.layer_metadata.iter().filter_map(|(path, data)| data.selected.then(|| path.clone())) {
					responses.push_back(DocumentOperation::SetLayerBlendMode { path, blend_mode }.into());
				}
			}
			SetOpacityForSelectedLayers(opacity) => {
				self.backup(responses);
				let opacity = opacity.clamp(0., 1.);

				for path in self.selected_layers().map(|path| path.to_vec()) {
					responses.push_back(DocumentOperation::SetLayerOpacity { path, opacity }.into());
				}
			}
			ToggleLayerVisibility(path) => {
				responses.push_back(DocumentOperation::ToggleLayerVisibility { path }.into());
				responses.push_back(ToolMessage::DocumentIsDirty.into());
			}
			ToggleLayerExpansion(path) => {
				self.layer_metadata_mut(&path).expanded ^= true;
				responses.push_back(DocumentStructureChanged.into());
				responses.push_back(LayerChanged(path).into())
			}
			SetLayerExpansion(path, is_expanded) => {
				self.layer_metadata_mut(&path).expanded = is_expanded;
				responses.push_back(DocumentStructureChanged.into());
				responses.push_back(LayerChanged(path).into())
			}
			SelectionChanged => {
				// TODO: Hoist this duplicated code into wider system
				responses.push_back(ToolMessage::DocumentIsDirty.into());
			}
			DeleteSelectedLayers => {
				self.backup(responses);

				for path in self.selected_layers_without_children() {
					responses.push_front(DocumentOperation::DeleteLayer { path: path.to_vec() }.into());
				}

				responses.push_front(ToolMessage::DocumentIsDirty.into());
			}
			SetViewMode(mode) => {
				self.view_mode = mode;
				responses.push_front(DocumentMessage::DirtyRenderDocument.into());
			}
			DuplicateSelectedLayers => {
				self.backup(responses);
				for path in self.selected_layers_sorted() {
					responses.push_back(DocumentOperation::DuplicateLayer { path: path.to_vec() }.into());
				}
			}
			SelectLayer(selected, ctrl, shift) => {
				let mut paths = vec![];
				let last_selection_exists = !self.layer_range_selection_reference.is_empty();

				// If we have shift pressed and a layer already selected then fill the range
				if shift && last_selection_exists {
					// Fill the selection range
					self.layer_metadata
						.iter()
						.filter(|(target, _)| self.graphene_document.layer_is_between(target, &selected, &self.layer_range_selection_reference))
						.for_each(|(layer_path, _)| {
							paths.push(layer_path.clone());
						});
				} else {
					if ctrl {
						// Toggle selection when holding ctrl
						let layer = self.layer_metadata_mut(&selected);
						layer.selected = !layer.selected;
						responses.push_back(LayerChanged(selected.clone()).into());
						responses.push_back(ToolMessage::DocumentIsDirty.into());
					} else {
						paths.push(selected.clone());
					}

					// Set our last selection reference
					self.layer_range_selection_reference = selected;
				}

				// Don't create messages for empty operations
				if !paths.is_empty() {
					// Add or set our selected layers
					if ctrl {
						responses.push_front(AddSelectedLayers(paths).into());
					} else {
						responses.push_front(SetSelectedLayers(paths).into());
					}
				}
			}
			UpdateLayerMetadata { layer_path: path, layer_metadata } => {
				self.layer_metadata.insert(path, layer_metadata);
			}
			SetSelectedLayers(paths) => {
				let selected = self.layer_metadata.iter_mut().filter(|(_, layer_metadata)| layer_metadata.selected);
				selected.for_each(|(path, layer_metadata)| {
					layer_metadata.selected = false;
					responses.push_back(LayerChanged(path.clone()).into())
				});

				responses.push_front(AddSelectedLayers(paths).into());
			}
			AddSelectedLayers(paths) => {
				for path in paths {
					responses.extend(self.select_layer(&path));
				}
				// TODO: Correctly update layer panel in clear_selection instead of here
				responses.push_back(FolderChanged(Vec::new()).into());
				responses.push_back(ToolMessage::DocumentIsDirty.into());
			}
			DebugPrintDocument => {
				log::debug!("{:#?}\n{:#?}", self.graphene_document, self.layer_metadata);
			}
			SelectAllLayers => {
				let all_layer_paths = self.all_layers();
				responses.push_front(SetSelectedLayers(all_layer_paths.map(|path| path.to_vec()).collect()).into());
			}
			DeselectAllLayers => {
				responses.push_front(SetSelectedLayers(vec![]).into());
				self.layer_range_selection_reference.clear();
			}
			DocumentHistoryBackward => self.undo(responses).unwrap_or_else(|e| log::warn!("{}", e)),
			DocumentHistoryForward => self.redo(responses).unwrap_or_else(|e| log::warn!("{}", e)),
			Undo => {
				responses.push_back(SelectMessage::Abort.into());
				responses.push_back(DocumentHistoryBackward.into());
				responses.push_back(ToolMessage::DocumentIsDirty.into());
				responses.push_back(RenderDocument.into());
				responses.push_back(FolderChanged(vec![]).into());
			}
			Redo => {
				responses.push_back(SelectMessage::Abort.into());
				responses.push_back(DocumentHistoryForward.into());
				responses.push_back(ToolMessage::DocumentIsDirty.into());
				responses.push_back(RenderDocument.into());
				responses.push_back(FolderChanged(vec![]).into());
			}
			FolderChanged(path) => {
				let _ = self.graphene_document.render_root(self.view_mode);
				responses.extend([LayerChanged(path).into(), DocumentStructureChanged.into()]);
			}
			DocumentStructureChanged => {
				let data_buffer: RawBuffer = self.serialize_root().into();
				responses.push_back(FrontendMessage::DisplayFolderTreeStructure { data_buffer }.into())
			}
			LayerChanged(path) => {
				if let Ok(layer_entry) = self.layer_panel_entry(path) {
					responses.push_back(FrontendMessage::UpdateLayer { data: layer_entry }.into());
				}
			}
			DispatchOperation(op) => match self.graphene_document.handle_operation(&op) {
				Ok(Some(document_responses)) => {
					for response in document_responses {
						match &response {
							DocumentResponse::FolderChanged { path } => responses.push_back(FolderChanged(path.clone()).into()),
							DocumentResponse::DeletedLayer { path } => {
								self.layer_metadata.remove(path);
							}
							DocumentResponse::LayerChanged { path } => responses.push_back(LayerChanged(path.clone()).into()),
							DocumentResponse::CreatedLayer { path } => {
								self.layer_metadata.insert(path.clone(), LayerMetadata::new(false));
								responses.push_back(LayerChanged(path.clone()).into());
								self.layer_range_selection_reference = path.clone();
								responses.push_back(SetSelectedLayers(vec![path.clone()]).into());
							}
							DocumentResponse::DocumentChanged => responses.push_back(RenderDocument.into()),
						};
						responses.push_back(ToolMessage::DocumentIsDirty.into());
					}
				}
				Err(e) => log::error!("DocumentError: {:?}", e),
				Ok(_) => (),
			},
			RenderDocument => {
				responses.push_back(
					FrontendMessage::UpdateArtwork {
						svg: self.graphene_document.render_root(self.view_mode),
					}
					.into(),
				);
				let document_transform = &self.movement_handler;

				let scale = 0.5 + ASYMPTOTIC_EFFECT + document_transform.scale * SCALE_EFFECT;
				let viewport_size = ipp.viewport_bounds.size();
				let viewport_mid = ipp.viewport_bounds.center();
				let [bounds1, bounds2] = self.graphene_document.visible_layers_bounding_box().unwrap_or([viewport_mid; 2]);
				let bounds1 = bounds1.min(viewport_mid) - viewport_size * scale;
				let bounds2 = bounds2.max(viewport_mid) + viewport_size * scale;
				let bounds_length = (bounds2 - bounds1) * (1. + SCROLLBAR_SPACING);
				let scrollbar_position = DVec2::splat(0.5) - (bounds1.lerp(bounds2, 0.5) - viewport_mid) / (bounds_length - viewport_size);
				let scrollbar_multiplier = bounds_length - viewport_size;
				let scrollbar_size = viewport_size / bounds_length;

				let log = document_transform.scale.log2();
				let ruler_interval = if log < 0. { 100. * 2_f64.powf(-log.ceil()) } else { 100. / 2_f64.powf(log.ceil()) };
				let ruler_spacing = ruler_interval * document_transform.scale;

				let ruler_origin = self.graphene_document.root.transform.transform_point2(DVec2::ZERO);

				responses.push_back(
					FrontendMessage::UpdateScrollbars {
						position: scrollbar_position.into(),
						size: scrollbar_size.into(),
						multiplier: scrollbar_multiplier.into(),
					}
					.into(),
				);

				responses.push_back(
					FrontendMessage::UpdateRulers {
						origin: ruler_origin.into(),
						spacing: ruler_spacing,
						interval: ruler_interval,
					}
					.into(),
				);
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
			NudgeSelectedLayers(x, y) => {
				self.backup(responses);
				for path in self.selected_layers().map(|path| path.to_vec()) {
					let operation = DocumentOperation::TransformLayerInViewport {
						path,
						transform: DAffine2::from_translation((x, y).into()).to_cols_array(),
					};
					responses.push_back(operation.into());
				}
				responses.push_back(ToolMessage::DocumentIsDirty.into());
			}
			MoveSelectedLayersTo { path, insert_index } => {
				responses.push_back(DocumentsMessage::Copy(Clipboard::System).into());
				responses.push_back(DocumentMessage::DeleteSelectedLayers.into());
				responses.push_back(
					DocumentsMessage::PasteIntoFolder {
						clipboard: Clipboard::System,
						path,
						insert_index,
					}
					.into(),
				);
			}
			ReorderSelectedLayers(relative_position) => {
				self.backup(responses);
				let all_layer_paths = self.all_layers_sorted();
				let selected_layers = self.selected_layers_sorted();
				if let Some(pivot) = match relative_position.signum() {
					-1 => selected_layers.first(),
					1 => selected_layers.last(),
					_ => unreachable!(),
				} {
					let all_layer_paths: Vec<_> = all_layer_paths
						.iter()
						.filter(|layer| layer.starts_with(&pivot[0..pivot.len() - 1]) && pivot.len() == layer.len())
						.collect();
					if let Some(pos) = all_layer_paths.iter().position(|path| *path == pivot) {
						let max = all_layer_paths.len() as i64 - 1;
						let insert_pos = (pos as i64 + relative_position as i64).clamp(0, max) as usize;
						let insert = all_layer_paths.get(insert_pos);
						if let Some(insert_path) = insert {
							let (id, path) = insert_path.split_last().expect("Can't move the root folder");
							if let Some(folder) = self.graphene_document.layer(path).ok().map(|layer| layer.as_folder().ok()).flatten() {
								let selected: Vec<_> = selected_layers
									.iter()
									.filter(|layer| layer.starts_with(path) && layer.len() == path.len() + 1)
									.map(|x| x.last().unwrap())
									.collect();
								let non_selected: Vec<_> = folder.layer_ids.iter().filter(|id| selected.iter().all(|x| x != id)).collect();
								let offset = if relative_position < 0 || non_selected.is_empty() { 0 } else { 1 };
								let fallback = offset * (non_selected.len());
								let insert_index = non_selected.iter().position(|x| *x == id).map(|x| x + offset).unwrap_or(fallback) as isize;
								responses.push_back(DocumentMessage::MoveSelectedLayersTo { path: path.to_vec(), insert_index }.into());
							}
						}
					}
				}
			}
			FlipSelectedLayers(axis) => {
				self.backup(responses);
				let scale = match axis {
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
			AlignSelectedLayers(axis, aggregate) => {
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
			RenameLayer(path, name) => responses.push_back(DocumentOperation::RenameLayer { path, name }.into()),
			MoveLayerInTree {
				layer: target_layer,
				insert_above,
				neighbor,
			} => {
				let neighbor_id = neighbor.last().expect("Tried to move next to root");
				let neighbor_path = &neighbor[..neighbor.len() - 1];

				if !neighbor.starts_with(&target_layer) {
					let containing_folder = self.graphene_document.folder(neighbor_path).expect("Neighbor does not exist");
					let neighbor_index = containing_folder.position_of_layer(*neighbor_id).expect("Neighbor layer does not exist");

					let layer = self.graphene_document.layer(&target_layer).expect("Layer moving does not exist.").to_owned();
					let destination_path = [neighbor_path.to_vec(), vec![generate_uuid()]].concat();
					let insert_index = if insert_above { neighbor_index } else { neighbor_index + 1 } as isize;

					responses.push_back(DocumentMessage::StartTransaction.into());
					responses.push_back(
						DocumentOperation::InsertLayer {
							layer,
							destination_path: destination_path.clone(),
							insert_index,
						}
						.into(),
					);
					responses.push_back(
						DocumentMessage::UpdateLayerMetadata {
							layer_path: destination_path,
							layer_metadata: *self.layer_metadata(&target_layer),
						}
						.into(),
					);
					responses.push_back(DocumentOperation::DeleteLayer { path: target_layer }.into());
					responses.push_back(DocumentMessage::CommitTransaction.into());
				}
			}
			SetSnapping(new_status) => {
				self.snapping_enabled = new_status;
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
			MoveLayerInTree,
		);

		if self.layer_metadata.values().any(|data| data.selected) {
			let select = actions!(DocumentMessageDiscriminant;
				DeleteSelectedLayers,
				DuplicateSelectedLayers,
				NudgeSelectedLayers,
				ReorderSelectedLayers,
				GroupSelectedLayers,
			);
			common.extend(select);
		}
		common.extend(self.movement_handler.actions());
		common.extend(self.transform_layer_handler.actions());
		common
	}
}

use std::collections::HashMap;
use std::collections::VecDeque;

pub use super::layer_panel::*;
use super::movement_handler::{MovementMessage, MovementMessageHandler};
use super::transform_layer_handler::{TransformLayerMessage, TransformLayerMessageHandler};

use crate::consts::{ASYMPTOTIC_EFFECT, FILE_EXPORT_SUFFIX, FILE_SAVE_SUFFIX, SCALE_EFFECT, SCROLLBAR_SPACING};
use crate::input::InputPreprocessor;
use crate::message_prelude::*;
use crate::EditorError;

use glam::{DAffine2, DVec2};
use graphene::layers::Folder;
use kurbo::PathSeg;
use log::warn;
use serde::{Deserialize, Serialize};

use graphene::layers::{BlendMode, style::ViewMode, simple_shape::Shape as GrapheneShape};
use graphene::{document::Document as GrapheneDocument, layers::LayerDataType, DocumentError, LayerId};
use graphene::{DocumentResponse, Operation as DocumentOperation};

type DocumentSave = (GrapheneDocument, HashMap<Vec<LayerId>, LayerData>);

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

#[derive(Clone, Debug)]
pub struct DocumentMessageHandler {
	pub graphene_document: GrapheneDocument,
	pub document_undo_history: Vec<DocumentSave>,
	pub document_redo_history: Vec<DocumentSave>,
	pub saved_document_identifier: u64,
	pub name: String,
	pub layer_data: HashMap<Vec<LayerId>, LayerData>,
	movement_handler: MovementMessageHandler,
	transform_layer_handler: TransformLayerMessageHandler,
	pub snapping_enabled: bool,
}

impl Default for DocumentMessageHandler {
	fn default() -> Self {
		Self {
			graphene_document: GrapheneDocument::default(),
			document_undo_history: Vec::new(),
			document_redo_history: Vec::new(),
			name: String::from("Untitled Document"),
			saved_document_identifier: 0,
			layer_data: vec![(vec![], LayerData::new(true))].into_iter().collect(),
			movement_handler: MovementMessageHandler::default(),
			transform_layer_handler: TransformLayerMessageHandler::default(),
			snapping_enabled: true,
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
	SetSelectedLayers(Vec<Vec<LayerId>>),
	AddSelectedLayers(Vec<Vec<LayerId>>),
	SelectAllLayers,
	SelectionChanged,
	DeselectAllLayers,
	DeleteLayer(Vec<LayerId>),
	DeleteSelectedLayers,
	DuplicateSelectedLayers,
	CreateFolder(Vec<LayerId>),
	SetBlendModeForSelectedLayers(BlendMode),
	SetOpacityForSelectedLayers(f64),
	RenameLayer(Vec<LayerId>, String),
	ToggleLayerVisibility(Vec<LayerId>),
	FlipSelectedLayers(FlipAxis),
	ToggleLayerExpansion(Vec<LayerId>),
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
	Undo,
	Redo,
	DocumentHistoryBackward,
	DocumentHistoryForward,
	ClearOverlays,
	SetViewMode(u8),
	NudgeSelectedLayers(f64, f64),
	AlignSelectedLayers(AlignAxis, AlignAggregate),
	MoveSelectedLayersTo {
		path: Vec<LayerId>,
		insert_index: isize,
	},
	ReorderSelectedLayers(i32), // relative_position,
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
	fn select_layer(&mut self, path: &[LayerId]) -> Option<Message> {
		if self.graphene_document.layer(path).ok()?.overlay {
			return None;
		}
		self.layer_data(path).selected = true;
		let data = self.layer_panel_entry(path.to_vec()).ok()?;
		(!path.is_empty()).then(|| FrontendMessage::UpdateLayer { data }.into())
	}

	pub fn selected_layers_bounding_box(&self) -> Option<[DVec2; 2]> {
		let paths = self.selected_layers();
		self.graphene_document.combined_viewport_bounding_box(paths)
	}

	// TODO: Consider moving this to some kind of overlay manager in the future
	pub fn selected_layers_vector_points(&self) -> Vec<VectorManipulatorShape> {
		let shapes = self.selected_layers().filter_map(|path_to_shape| {
			let viewport_transform = self.graphene_document.generate_transform_relative_to_viewport(path_to_shape).ok()?;

			let shape = match &self.graphene_document.layer(path_to_shape).ok()?.data {
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

	pub fn layerdata(&self, path: &[LayerId]) -> &LayerData {
		self.layer_data.get(path).expect("Layerdata does not exist")
	}

	pub fn layerdata_mut(&mut self, path: &[LayerId]) -> &mut LayerData {
		self.layer_data.entry(path.to_vec()).or_insert_with(|| LayerData::new(true))
	}

	pub fn selected_layers(&self) -> impl Iterator<Item = &[LayerId]> {
		self.layer_data.iter().filter_map(|(path, data)| data.selected.then(|| path.as_slice()))
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
					if self.layerdata(path).expanded {
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

	/// Returns the paths to all layers in order, optionally including only selected or non-selected layers.
	fn layers_sorted(&self, selected: Option<bool>) -> Vec<Vec<LayerId>> {
		// Compute the indices for each layer to be able to sort them
		let mut layers_with_indices: Vec<(Vec<LayerId>, Vec<usize>)> = self

			.layer_data
			.iter()
			// 'path.len() > 0' filters out root layer since it has no indices
			.filter_map(|(path, data)| (!path.is_empty() && (data.selected == selected.unwrap_or(data.selected))).then(|| path.clone()))
			.filter_map(|path| {
				// Currently it is possible that layer_data contains layers that are don't actually exist (has been partially fixed in #281)
				// and thus indices_for_path can return an error. We currently skip these layers and log a warning.
				// Once this problem is solved this code can be simplified
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
	pub fn all_layers_sorted(&self) -> Vec<Vec<LayerId>> {
		self.layers_sorted(None)
	}

	/// Returns the paths to all selected layers in order
	pub fn selected_layers_sorted(&self) -> Vec<Vec<LayerId>> {
		self.layers_sorted(Some(true))
	}

	/// Returns the paths to all non_selected layers in order
	#[allow(dead_code)] // used for test cases
	pub fn non_selected_layers_sorted(&self) -> Vec<Vec<LayerId>> {
		self.layers_sorted(Some(false))
	}

	pub fn with_name(name: String) -> Self {
		Self {
			graphene_document: GrapheneDocument::default(),
			document_undo_history: Vec::new(),
			document_redo_history: Vec::new(),
			saved_document_identifier: 0,
			name,
			layer_data: vec![(vec![], LayerData::new(true))].into_iter().collect(),
			movement_handler: MovementMessageHandler::default(),
			transform_layer_handler: TransformLayerMessageHandler::default(),
			snapping_enabled: true,
		}
	}

	pub fn with_name_and_content(name: String, serialized_content: String) -> Result<Self, EditorError> {
		let mut document = Self::with_name(name);
		let internal_document = GrapheneDocument::with_content(&serialized_content);
		match internal_document {
			Ok(handle) => {
				document.graphene_document = handle;
				Ok(document)
			}
			Err(DocumentError::InvalidFile(msg)) => Err(EditorError::Document(msg)),
			_ => Err(EditorError::Document(String::from("Failed to open file"))),
		}
	}

	pub fn layer_data(&mut self, path: &[LayerId]) -> &mut LayerData {
		layer_data(&mut self.layer_data, path)
	}

	pub fn backup(&mut self, responses: &mut VecDeque<Message>) {
		self.document_redo_history.clear();
		let new_layer_data = self
			.layer_data
			.iter()
			.filter_map(|(key, value)| (!self.graphene_document.layer(key).unwrap().overlay).then(|| (key.clone(), *value)))
			.collect();
		self.document_undo_history.push((self.graphene_document.clone_without_overlays(), new_layer_data));

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
			Some((document, layer_data)) => {
				let document = std::mem::replace(&mut self.graphene_document, document);
				let layer_data = std::mem::replace(&mut self.layer_data, layer_data);
				self.document_redo_history.push((document, layer_data));
				Ok(())
			}
			None => Err(EditorError::NoTransactionInProgress),
		}
	}

	pub fn redo(&mut self, responses: &mut VecDeque<Message>) -> Result<(), EditorError> {
		// Push the UpdateOpenDocumentsList message to the bus in order to update the save status of the open documents
		responses.push_back(DocumentsMessage::UpdateOpenDocumentsList.into());

		match self.document_redo_history.pop() {
			Some((document, layer_data)) => {
				let document = std::mem::replace(&mut self.graphene_document, document);
				let layer_data = std::mem::replace(&mut self.layer_data, layer_data);
				let new_layer_data = layer_data
					.iter()
					.filter_map(|(key, value)| (!self.graphene_document.layer(key).unwrap().overlay).then(|| (key.clone(), *value)))
					.collect();
				self.document_undo_history.push((document.clone_without_overlays(), new_layer_data));
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

	pub fn layer_panel_entry(&mut self, path: Vec<LayerId>) -> Result<LayerPanelEntry, EditorError> {
		let data: LayerData = *layer_data(&mut self.layer_data, &path);
		let layer = self.graphene_document.layer(&path)?;
		let entry = layer_panel_entry(&data, self.graphene_document.multiply_transforms(&path)?, layer, path);
		Ok(entry)
	}

	/// Returns a list of `LayerPanelEntry`s intended for display purposes. These don't contain
	/// any actual data, but rather attributes such as visibility and names of the layers.
	pub fn layer_panel(&mut self, path: &[LayerId]) -> Result<Vec<LayerPanelEntry>, EditorError> {
		let folder = self.graphene_document.folder(path)?;
		let paths: Vec<Vec<LayerId>> = folder.layer_ids.iter().map(|id| [path, &[*id]].concat()).collect();
		let data: Vec<LayerData> = paths.iter().map(|path| *layer_data(&mut self.layer_data, path)).collect();
		let folder = self.graphene_document.folder(path)?;
		let entries = folder
			.layers()
			.iter()
			.zip(paths.iter().zip(data))
			.rev()
			.filter(|(layer, _)| !layer.overlay)
			.map(|(layer, (path, data))| {
				layer_panel_entry(
					&data,
					self.graphene_document
						.generate_transform_across_scope(path, Some(self.graphene_document.root.transform.inverse()))
						.unwrap(),
					layer,
					path.to_vec(),
				)
			})
			.collect();
		Ok(entries)
	}
}

impl MessageHandler<DocumentMessage, &InputPreprocessor> for DocumentMessageHandler {
	fn process_action(&mut self, message: DocumentMessage, ipp: &InputPreprocessor, responses: &mut VecDeque<Message>) {
		use DocumentMessage::*;
		match message {
			Movement(message) => self
				.movement_handler
				.process_action(message, (layer_data(&mut self.layer_data, &[]), &self.graphene_document, ipp), responses),
			TransformLayers(message) => self
				.transform_layer_handler
				.process_action(message, (&mut self.layer_data, &mut self.graphene_document, ipp), responses),
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
							self.graphene_document.render_root()
						),
						name,
					}
					.into(),
				)
			}
			SaveDocument => {
				self.saved_document_identifier = self.current_identifier();
				// Update the save status of the just saved document
				responses.push_back(DocumentsMessage::UpdateOpenDocumentsList.into());

				let name = match self.name.ends_with(FILE_SAVE_SUFFIX) {
					true => self.name.clone(),
					false => self.name.clone() + FILE_SAVE_SUFFIX,
				};
				responses.push_back(
					FrontendMessage::SaveDocument {
						document: self.graphene_document.serialize_document(),
						name,
					}
					.into(),
				)
			}
			CreateFolder(mut path) => {
				let id = generate_uuid();
				path.push(id);
				self.layerdata_mut(&path).expanded = true;
				responses.push_back(DocumentOperation::CreateFolder { path }.into())
			}
			GroupSelectedLayers => {
				let common_prefix = self.graphene_document.common_prefix(self.selected_layers());
				let (_id, common_prefix) = common_prefix.split_last().unwrap_or((&0, &[]));

				let mut new_folder_path = common_prefix.to_vec();
				new_folder_path.push(generate_uuid());

				responses.push_back(DocumentsMessage::Copy.into());
				responses.push_back(DocumentMessage::DeleteSelectedLayers.into());
				responses.push_back(DocumentOperation::CreateFolder { path: new_folder_path.clone() }.into());
				responses.push_back(DocumentMessage::ToggleLayerExpansion(new_folder_path.clone()).into());
				responses.push_back(
					DocumentsMessage::PasteIntoFolder {
						path: new_folder_path.clone(),
						insert_index: -1,
					}
					.into(),
				);
				responses.push_back(DocumentMessage::SetSelectedLayers(vec![new_folder_path]).into());
			}
			SetBlendModeForSelectedLayers(blend_mode) => {
				self.backup(responses);
				for path in self.layer_data.iter().filter_map(|(path, data)| data.selected.then(|| path.clone())) {
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
			}
			ToggleLayerExpansion(path) => {
				self.layer_data(&path).expanded ^= true;
				responses.push_back(DocumentStructureChanged.into());
				responses.push_back(LayerChanged(path).into())
			}
			SelectionChanged => {
				// TODO: Hoist this duplicated code into wider system
				responses.push_back(ToolMessage::SelectedLayersChanged.into());
			}
			DeleteSelectedLayers => {
				self.backup(responses);
				responses.push_front(ToolMessage::SelectedLayersChanged.into());
				for path in self.selected_layers().map(|path| path.to_vec()) {
					responses.push_front(DocumentOperation::DeleteLayer { path }.into());
				}
			}
			ClearOverlays => {
				responses.push_back(ToolMessage::SelectedLayersChanged.into());
				for path in self.layer_data.keys().filter(|path| self.graphene_document.layer(path).unwrap().overlay).cloned() {
					responses.push_front(DocumentOperation::DeleteLayer { path }.into());
				}
			}
			SetViewMode(idx) => {
				let mut mode_update_func = match idx{
					1 => {
						self.graphene_document.view_mode = ViewMode::WireFrame;
						|s: &mut GrapheneShape|{s.style.view_mode(ViewMode::WireFrame)}
					},
					_ => {
						self.graphene_document.view_mode = ViewMode::Normal;
						|s: &mut GrapheneShape|{s.style.view_mode(ViewMode::Normal)}
					},
				};
				GrapheneDocument::visit_all_shapes(&mut self.graphene_document.root, &mut mode_update_func);
				self.graphene_document.root.cache_dirty = true;
				responses.push_back(
					FrontendMessage::UpdateCanvas {
						document: self.graphene_document.render_root(),
					}
					.into(),
				);
			}
			DuplicateSelectedLayers => {
				self.backup(responses);
				for path in self.selected_layers_sorted() {
					responses.push_back(DocumentOperation::DuplicateLayer { path }.into());
				}
			}
			SetSelectedLayers(paths) => {
				self.layer_data.iter_mut().filter(|(_, layer_data)| layer_data.selected).for_each(|(path, layer_data)| {
					layer_data.selected = false;
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
				responses.push_back(ToolMessage::SelectedLayersChanged.into());
			}
			SelectAllLayers => {
				let all_layer_paths = self
					.layer_data
					.keys()
					.filter(|path| !path.is_empty() && !self.graphene_document.layer(path).map(|layer| layer.overlay).unwrap_or(false))
					.cloned()
					.collect::<Vec<_>>();
				responses.push_front(SetSelectedLayers(all_layer_paths).into());
			}
			DeselectAllLayers => responses.push_front(SetSelectedLayers(vec![]).into()),
			DocumentHistoryBackward => self.undo(responses).unwrap_or_else(|e| log::warn!("{}", e)),
			DocumentHistoryForward => self.redo(responses).unwrap_or_else(|e| log::warn!("{}", e)),
			Undo => {
				responses.push_back(SelectMessage::Abort.into());
				responses.push_back(DocumentHistoryBackward.into());
				responses.push_back(ToolMessage::SelectedLayersChanged.into());
				responses.push_back(RenderDocument.into());
				responses.push_back(FolderChanged(vec![]).into());
			}
			Redo => {
				responses.push_back(SelectMessage::Abort.into());
				responses.push_back(DocumentHistoryForward.into());
				responses.push_back(ToolMessage::SelectedLayersChanged.into());
				responses.push_back(RenderDocument.into());
				responses.push_back(FolderChanged(vec![]).into());
			}
			FolderChanged(path) => {
				let _ = self.graphene_document.render_root();
				responses.extend([LayerChanged(path).into(), DocumentStructureChanged.into()]);
			}
			DocumentStructureChanged => {
				let data_buffer: RawBuffer = self.serialize_root().into();
				responses.push_back(FrontendMessage::DisplayFolderTreeStructure { data_buffer }.into())
			}
			LayerChanged(path) => {
				responses.extend(self.layer_panel_entry(path.clone()).ok().and_then(|entry| {
					let overlay = self.graphene_document.layer(&path).unwrap().overlay;
					(!overlay).then(|| FrontendMessage::UpdateLayer { data: entry }.into())
				}));
			}
			DispatchOperation(op) => match self.graphene_document.handle_operation(&op) {
				Ok(Some(document_responses)) => {
					for response in document_responses {
						match response {
							DocumentResponse::FolderChanged { path } => responses.push_back(FolderChanged(path).into()),
							DocumentResponse::DeletedLayer { path } => {
								self.layer_data.remove(&path);
								responses.push_back(ToolMessage::SelectedLayersChanged.into())
							}
							DocumentResponse::LayerChanged { path } => responses.push_back(LayerChanged(path).into()),
							DocumentResponse::CreatedLayer { path } => {
								self.layer_data.insert(path.clone(), LayerData::new(false));
								responses.push_back(LayerChanged(path.clone()).into());
								if !self.graphene_document.layer(&path).unwrap().overlay {
									responses.push_back(SetSelectedLayers(vec![path]).into())
								}
							}
							DocumentResponse::DocumentChanged => responses.push_back(RenderDocument.into()),
						};
					}
					// log::debug!("LayerPanel: {:?}", self.layer_data.keys());
				}
				Err(e) => log::error!("DocumentError: {:?}", e),
				Ok(_) => (),
			},
			RenderDocument => {
				responses.push_back(
					FrontendMessage::UpdateCanvas {
						document: self.graphene_document.render_root(),
					}
					.into(),
				);
				let root_layerdata = self.layerdata(&[]);

				let scale = 0.5 + ASYMPTOTIC_EFFECT + root_layerdata.scale * SCALE_EFFECT;
				let viewport_size = ipp.viewport_bounds.size();
				let viewport_mid = ipp.viewport_bounds.center();
				let [bounds1, bounds2] = self.graphene_document.visible_layers_bounding_box().unwrap_or([viewport_mid; 2]);
				let bounds1 = bounds1.min(viewport_mid) - viewport_size * scale;
				let bounds2 = bounds2.max(viewport_mid) + viewport_size * scale;
				let bounds_length = (bounds2 - bounds1) * (1. + SCROLLBAR_SPACING);
				let scrollbar_position = DVec2::splat(0.5) - (bounds1.lerp(bounds2, 0.5) - viewport_mid) / (bounds_length - viewport_size);
				let scrollbar_multiplier = bounds_length - viewport_size;
				let scrollbar_size = viewport_size / bounds_length;

				let log = root_layerdata.scale.log2();
				let ruler_inverval = if log < 0. { 100. * 2_f64.powf(-log.ceil()) } else { 100. / 2_f64.powf(log.ceil()) };
				let ruler_spacing = ruler_inverval * root_layerdata.scale;

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
						interval: ruler_inverval,
					}
					.into(),
				);
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
				responses.push_back(ToolMessage::SelectedLayersChanged.into());
			}
			MoveSelectedLayersTo { path, insert_index } => {
				responses.push_back(DocumentsMessage::Copy.into());
				responses.push_back(DocumentMessage::DeleteSelectedLayers.into());
				responses.push_back(DocumentsMessage::PasteIntoFolder { path, insert_index }.into());
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
				if let Some([min, max]) = self.graphene_document.combined_viewport_bounding_box(self.selected_layers().map(|x| x)) {
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
					responses.push_back(ToolMessage::SelectedLayersChanged.into());
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
				if let Some(combined_box) = self.graphene_document.combined_viewport_bounding_box(self.selected_layers().map(|x| x)) {
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
					responses.push_back(ToolMessage::SelectedLayersChanged.into());
				}
			}
			RenameLayer(path, name) => responses.push_back(DocumentOperation::RenameLayer { path, name }.into()),
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
		);

		if self.layer_data.values().any(|data| data.selected) {
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

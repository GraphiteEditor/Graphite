pub use super::layer_panel::*;
use crate::{frontend::layer_panel::*, EditorError};
use document_core::{document::Document as InternalDocument, LayerId};
use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::input::InputPreprocessor;
use crate::message_prelude::*;
use document_core::layers::BlendMode;
use document_core::{DocumentResponse, Operation as DocumentOperation};
use log::warn;

use std::collections::VecDeque;

use super::movement_handler::{MovementMessage, MovementMessageHandler};

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

#[derive(Clone, Debug)]
pub struct DocumentMessageHandler {
	pub document: InternalDocument,
	pub document_backup: Option<InternalDocument>,
	pub name: String,
	pub layer_data: HashMap<Vec<LayerId>, LayerData>,
	movement_handler: MovementMessageHandler,
}

impl Default for DocumentMessageHandler {
	fn default() -> Self {
		Self {
			document: InternalDocument::default(),
			document_backup: None,
			name: String::from("Untitled Document"),
			layer_data: vec![(vec![], LayerData::new(true))].into_iter().collect(),
			movement_handler: MovementMessageHandler::default(),
		}
	}
}

#[impl_message(Message, DocumentsMessage, Document)]
#[derive(PartialEq, Clone, Debug)]
pub enum DocumentMessage {
	#[child]
	Movement(MovementMessage),
	DispatchOperation(Box<DocumentOperation>),
	SelectLayers(Vec<Vec<LayerId>>),
	SelectAllLayers,
	DeselectAllLayers,
	DeleteLayer(Vec<LayerId>),
	DeleteSelectedLayers,
	DuplicateSelectedLayers,
	SetBlendModeForSelectedLayers(BlendMode),
	SetOpacityForSelectedLayers(f64),
	AddFolder(Vec<LayerId>),
	RenameLayer(Vec<LayerId>, String),
	ToggleLayerVisibility(Vec<LayerId>),
	FlipSelectedLayers(FlipAxis),
	ToggleLayerExpansion(Vec<LayerId>),
	StartTransaction,
	RollbackTransaction,
	AbortTransaction,
	CommitTransaction,
	ExportDocument,
	RenderDocument,
	Undo,
	NudgeSelectedLayers(f64, f64),
	FlipLayer(Vec<LayerId>, bool, bool),
	AlignSelectedLayers(AlignAxis, AlignAggregate),
	DragLayer(Vec<LayerId>, DVec2),
	MoveSelectedLayersTo {
		path: Vec<LayerId>,
		insert_index: isize,
	},
	ReorderSelectedLayers(i32), // relative_position,
	SetLayerTranslation(Vec<LayerId>, Option<f64>, Option<f64>),
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
	pub fn active_document(&self) -> &DocumentMessageHandler {
		self
	}
	pub fn active_document_mut(&mut self) -> &mut DocumentMessageHandler {
		self
	}
	fn filter_document_responses(&self, document_responses: &mut Vec<DocumentResponse>) -> bool {
		let len = document_responses.len();
		document_responses.retain(|response| !matches!(response, DocumentResponse::DocumentChanged));
		document_responses.len() != len
	}
	fn handle_folder_changed(&mut self, path: Vec<LayerId>) -> Option<Message> {
		let _ = self.document.render_root();
		self.layer_data(&path).expanded.then(|| {
			let children = self.layer_panel(path.as_slice()).expect("The provided Path was not valid");
			FrontendMessage::ExpandFolder { path, children }.into()
		})
	}
	fn clear_selection(&mut self) {
		self.active_document_mut().layer_data.values_mut().for_each(|layer_data| layer_data.selected = false);
	}
	fn select_layer(&mut self, path: &[LayerId]) -> Option<Message> {
		self.layer_data(path).selected = true;
		// TODO: Add deduplication
		(!path.is_empty()).then(|| self.handle_folder_changed(path[..path.len() - 1].to_vec())).flatten()
	}
	pub fn layerdata(&self, path: &[LayerId]) -> &LayerData {
		self.active_document().layer_data.get(path).expect("Layerdata does not exist")
	}
	pub fn layerdata_mut(&mut self, path: &[LayerId]) -> &mut LayerData {
		self.active_document_mut().layer_data.entry(path.to_vec()).or_insert_with(|| LayerData::new(true))
	}

	fn selected_layers(&self) -> impl Iterator<Item = &Vec<LayerId>> {
		self.active_document().layer_data.iter().filter_map(|(path, data)| data.selected.then(|| path))
	}

	/// Returns the paths to all layers in order, optionally including only selected or non-selected layers.
	fn layers_sorted(&self, selected: Option<bool>) -> Vec<Vec<LayerId>> {
		// Compute the indices for each layer to be able to sort them
		let mut layers_with_indices: Vec<(Vec<LayerId>, Vec<usize>)> = self
			.active_document()
			.layer_data
			.iter()
			// 'path.len() > 0' filters out root layer since it has no indices
			.filter_map(|(path, data)| (!path.is_empty() && (data.selected == selected.unwrap_or(data.selected))).then(|| path.clone()))
			.filter_map(|path| {
				// Currently it is possible that layer_data contains layers that are don't actually exist (has been partially fixed in #281)
				// and thus indices_for_path can return an error. We currently skip these layers and log a warning.
				// Once this problem is solved this code can be simplified
				match self.active_document().document.indices_for_path(&path) {
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
			document: InternalDocument::default(),
			document_backup: None,
			name,
			layer_data: vec![(vec![], LayerData::new(true))].into_iter().collect(),
			movement_handler: MovementMessageHandler::default(),
		}
	}

	pub fn layer_data(&mut self, path: &[LayerId]) -> &mut LayerData {
		layer_data(&mut self.layer_data, path)
	}

	pub fn backup(&mut self) {
		self.document_backup = Some(self.document.clone())
	}

	pub fn rollback(&mut self) -> Result<(), EditorError> {
		match &self.document_backup {
			Some(backup) => Ok(self.document = backup.clone()),
			None => Err(EditorError::NoTransactionInProgress),
		}
	}

	pub fn reset(&mut self) -> Result<(), EditorError> {
		match self.document_backup.take() {
			Some(backup) => Ok(self.document = backup),
			None => Err(EditorError::NoTransactionInProgress),
		}
	}

	pub fn layer_panel_entry(&mut self, path: Vec<LayerId>) -> Result<LayerPanelEntry, EditorError> {
		let data: LayerData = *layer_data(&mut self.layer_data, &path);
		let layer = self.document.layer(&path)?;
		let entry = layer_panel_entry(&data, self.document.multiply_transoforms(&path).unwrap(), layer, path);
		Ok(entry)
	}

	/// Returns a list of `LayerPanelEntry`s intended for display purposes. These don't contain
	/// any actual data, but ratfolderch as visibility and names of the layers.
	pub fn layer_panel(&mut self, path: &[LayerId]) -> Result<Vec<LayerPanelEntry>, EditorError> {
		let folder = self.document.folder(path)?;
		let paths: Vec<Vec<LayerId>> = folder.layer_ids.iter().map(|id| [path, &[*id]].concat()).collect();
		let data: Vec<LayerData> = paths.iter().map(|path| *layer_data(&mut self.layer_data, path)).collect();
		let folder = self.document.folder(path)?;
		let entries = folder
			.layers()
			.iter()
			.zip(paths.iter().zip(data))
			.rev()
			.map(|(layer, (path, data))| layer_panel_entry(&data, self.document.multiply_transoforms(path).unwrap(), layer, path.to_vec()))
			.collect();
		Ok(entries)
	}
}

impl MessageHandler<DocumentMessage, &InputPreprocessor> for DocumentMessageHandler {
	fn process_action(&mut self, message: DocumentMessage, ipp: &InputPreprocessor, responses: &mut VecDeque<Message>) {
		use DocumentMessage::*;
		match message {
			Movement(message) => self.movement_handler.process_action(message, (layer_data(&mut self.layer_data, &[]), &self.document, ipp), responses),
			DeleteLayer(path) => responses.push_back(DocumentOperation::DeleteLayer { path }.into()),
			AddFolder(path) => responses.push_back(DocumentOperation::AddFolder { path }.into()),
			StartTransaction => self.active_document_mut().backup(),
			RollbackTransaction => self.active_document_mut().rollback().unwrap_or_else(|e| log::warn!("{}", e)),
			AbortTransaction => self.active_document_mut().reset().unwrap_or_else(|e| log::warn!("{}", e)),
			CommitTransaction => self.active_document_mut().document_backup = None,
			ExportDocument => responses.push_back(
				FrontendMessage::ExportDocument {
					//TODO: Add canvas size instead of using 1920x1080 by default
					document: format!(
						r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1920 1080">{}{}</svg>"#,
						"\n",
						self.active_document_mut().document.render_root(),
					),
				}
				.into(),
			),
			SetBlendModeForSelectedLayers(blend_mode) => {
				let active_document = self.active_document();

				for path in active_document.layer_data.iter().filter_map(|(path, data)| data.selected.then(|| path.clone())) {
					responses.push_back(DocumentOperation::SetLayerBlendMode { path, blend_mode }.into());
				}
			}
			SetOpacityForSelectedLayers(opacity) => {
				let opacity = opacity.clamp(0., 1.);

				for path in self.selected_layers().cloned() {
					responses.push_back(DocumentOperation::SetLayerOpacity { path, opacity }.into());
				}
			}
			ToggleLayerVisibility(path) => {
				responses.push_back(DocumentOperation::ToggleVisibility { path }.into());
			}
			ToggleLayerExpansion(path) => {
				self.active_document_mut().layer_data(&path).expanded ^= true;
				responses.extend(self.handle_folder_changed(path));
			}
			DeleteSelectedLayers => {
				for path in self.selected_layers().cloned() {
					responses.push_back(DocumentOperation::DeleteLayer { path }.into())
				}
			}
			DuplicateSelectedLayers => {
				for path in self.selected_layers_sorted() {
					responses.push_back(DocumentOperation::DuplicateLayer { path }.into())
				}
			}
			SelectLayers(paths) => {
				self.clear_selection();
				for path in paths {
					responses.extend(self.select_layer(&path));
				}
				// TODO: Correctly update layer panel in clear_selection instead of here
				responses.extend(self.handle_folder_changed(Vec::new()));
			}
			SelectAllLayers => {
				let all_layer_paths = self.active_document().layer_data.keys().filter(|path| !path.is_empty()).cloned().collect::<Vec<_>>();
				for path in all_layer_paths {
					responses.extend(self.select_layer(&path));
				}
			}
			DeselectAllLayers => {
				self.clear_selection();
				let children = self.active_document_mut().layer_panel(&[]).expect("The provided Path was not valid");
				responses.push_back(FrontendMessage::ExpandFolder { path: vec![], children }.into());
			}
			Undo => {
				// this is a temporary fix and will be addressed by #123
				if let Some(id) = self.active_document().document.root.as_folder().unwrap().list_layers().last() {
					responses.push_back(DocumentOperation::DeleteLayer { path: vec![*id] }.into())
				}
			}
			DispatchOperation(op) => match self.active_document_mut().document.handle_operation(&op) {
				Ok(Some(mut document_responses)) => {
					let canvas_dirty = self.filter_document_responses(&mut document_responses);
					responses.extend(
						document_responses
							.into_iter()
							.map(|response| match response {
								DocumentResponse::FolderChanged { path } => self.handle_folder_changed(path),
								DocumentResponse::DeletedLayer { path } => {
									self.active_document_mut().layer_data.remove(&path);
									None
								}
								DocumentResponse::LayerChanged { path } => Some(
									FrontendMessage::UpdateLayer {
										path: path.clone(),
										data: self.layer_panel_entry(path).unwrap(),
									}
									.into(),
								),
								DocumentResponse::CreatedLayer { path } => self.select_layer(&path),
								DocumentResponse::DocumentChanged => unreachable!(),
							})
							.flatten(),
					);
					if canvas_dirty {
						responses.push_back(RenderDocument.into())
					}
				}
				Err(e) => log::error!("DocumentError: {:?}", e),
				Ok(_) => (),
			},
			RenderDocument => responses.push_back(
				FrontendMessage::UpdateCanvas {
					document: self.active_document_mut().document.render_root(),
				}
				.into(),
			),
			NudgeSelectedLayers(x, y) => {
				let delta = {
					let root_layer_rotation = self.layerdata_mut(&[]).rotation;
					let rotate_to_viewport_space = DAffine2::from_angle(root_layer_rotation).inverse();
					rotate_to_viewport_space.transform_point2((x, y).into())
				};
				for path in self.selected_layers().cloned() {
					let operation = DocumentOperation::TransformLayer {
						path,
						transform: DAffine2::from_translation(delta).to_cols_array(),
					};
					responses.push_back(operation.into());
				}
			}
			MoveSelectedLayersTo { path, insert_index } => {
				responses.push_back(DocumentsMessage::CopySelectedLayers.into());
				responses.push_back(DocumentMessage::DeleteSelectedLayers.into());
				responses.push_back(DocumentsMessage::PasteLayers { path, insert_index }.into());
			}
			ReorderSelectedLayers(relative_position) => {
				let all_layer_paths = self.all_layers_sorted();
				let selected_layers = self.selected_layers_sorted();
				if let Some(pivot) = match relative_position.signum() {
					-1 => selected_layers.first(),
					1 => selected_layers.last(),
					_ => unreachable!(),
				} {
					if let Some(pos) = all_layer_paths.iter().position(|path| path == pivot) {
						let max = all_layer_paths.len() as i64 - 1;
						let insert_pos = (pos as i64 + relative_position as i64).clamp(0, max) as usize;
						let insert = all_layer_paths.get(insert_pos);
						if let Some(insert_path) = insert {
							let (id, path) = insert_path.split_last().expect("Can't move the root folder");
							if let Some(folder) = self.active_document().document.layer(path).ok().map(|layer| layer.as_folder().ok()).flatten() {
								let selected: Vec<_> = selected_layers
									.iter()
									.filter(|layer| layer.starts_with(path) && layer.len() == path.len() + 1)
									.map(|x| x.last().unwrap())
									.collect();
								let non_selected: Vec<_> = folder.layer_ids.iter().filter(|id| selected.iter().all(|x| x != id)).collect();
								let offset = if relative_position < 0 || non_selected.is_empty() { 0 } else { 1 };
								let fallback = offset * (non_selected.len());
								let insert_index = non_selected.iter().position(|x| *x == id).map(|x| x + offset).unwrap_or(fallback) as isize;
								responses.push_back(DocumentMessage::MoveSelectedLayersTo { path: path.to_vec(), insert_index }.into())
							}
						}
					}
				}
			}
			FlipLayer(path, flip_horizontal, flip_vertical) => {
				if let Ok(layer) = self.active_document_mut().document.layer_mut(&path) {
					let scale = DVec2::new(if flip_horizontal { -1. } else { 1. }, if flip_vertical { -1. } else { 1. });
					responses.push_back(
						DocumentOperation::SetLayerTransform {
							path,
							transform: (layer.transform * DAffine2::from_scale(scale)).to_cols_array(),
						}
						.into(),
					);
				}
			}
			FlipSelectedLayers(axis) => {
				// TODO: Handle folder nested transforms with the transforms API
				if self.selected_layers().next().is_none() {
					return;
				}

				let selected_layers = self.selected_layers().cloned().filter_map(|path| {
					let layer = self.active_document().document.layer(&path).ok()?;
					// TODO: Refactor with `reduce` and `merge_bounding_boxes` once the latter is added
					let (min, max) = {
						let bounding_box = layer.current_bounding_box()?;
						match axis {
							FlipAxis::X => (bounding_box[0].x, bounding_box[1].x),
							FlipAxis::Y => (bounding_box[0].y, bounding_box[1].y),
						}
					};
					Some((path, (min, max)))
				});
				let (paths, layers): (Vec<_>, Vec<_>) = selected_layers.unzip();
				if let Some(middle) = layers
					.iter()
					.copied()
					.reduce(|(min_a, max_a), (min_b, max_b)| (min_a.min(min_b), max_a.max(max_b)))
					.map(|(min, max)| (min + max) / 2.)
				{
					for path in paths {
						let layer = self.active_document().document.layer(&path).unwrap();
						let mut transform = layer.transform;
						let scale = match axis {
							FlipAxis::X => DVec2::new(-1., 1.),
							FlipAxis::Y => DVec2::new(1., -1.),
						};
						transform = transform * DAffine2::from_scale(scale);

						let coord = match axis {
							FlipAxis::X => &mut transform.translation.x,
							FlipAxis::Y => &mut transform.translation.y,
						};
						*coord = *coord - 2. * (*coord - middle);

						responses.push_back(
							DocumentOperation::SetLayerTransform {
								path,
								transform: transform.to_cols_array(),
							}
							.into(),
						);
					}
				}
			}
			AlignSelectedLayers(axis, aggregate) => {
				// TODO: Handle folder nested transforms with the transforms API
				if self.selected_layers().next().is_none() {
					return;
				}

				let selected_layers = self.selected_layers().cloned().filter_map(|path| {
					let layer = self.active_document().document.layer(&path).ok()?;
					let point = {
						let bounding_box = layer.current_bounding_box()?;
						match aggregate {
							AlignAggregate::Min => bounding_box[0],
							AlignAggregate::Max => bounding_box[1],
							AlignAggregate::Center => bounding_box[0].lerp(bounding_box[1], 0.5),
							AlignAggregate::Average => bounding_box[0].lerp(bounding_box[1], 0.5),
						}
					};
					let (bounding_box_coord, translation_coord) = match axis {
						AlignAxis::X => (point.x, layer.transform.translation.x),
						AlignAxis::Y => (point.y, layer.transform.translation.y),
					};
					Some((path, bounding_box_coord, translation_coord))
				});
				let selected_layers: Vec<_> = selected_layers.collect();

				let bounding_box_coords = selected_layers.iter().map(|(_, bounding_box_coord, _)| bounding_box_coord).cloned();
				if let Some(aggregated_coord) = match aggregate {
					AlignAggregate::Min => bounding_box_coords.reduce(|a, b| a.min(b)),
					AlignAggregate::Max => bounding_box_coords.reduce(|a, b| a.max(b)),
					AlignAggregate::Center => {
						// TODO: Refactor with `reduce` and `merge_bounding_boxes` once the latter is added
						self.selected_layers()
							.filter_map(|path| self.active_document().document.layer(path).ok().map(|layer| layer.current_bounding_box()).flatten())
							.map(|bbox| match axis {
								AlignAxis::X => (bbox[0].x, bbox[1].x),
								AlignAxis::Y => (bbox[0].y, bbox[1].y),
							})
							.reduce(|(a, b), (c, d)| (a.min(c), b.max(d)))
							.map(|(min, max)| (min + max) / 2.)
					}
					AlignAggregate::Average => Some(bounding_box_coords.sum::<f64>() / selected_layers.len() as f64),
				} {
					for (path, bounding_box_coord, translation_coord) in selected_layers {
						let new_coord = aggregated_coord - (bounding_box_coord - translation_coord);
						match axis {
							AlignAxis::X => responses.push_back(DocumentMessage::SetLayerTranslation(path, Some(new_coord), None).into()),
							AlignAxis::Y => responses.push_back(DocumentMessage::SetLayerTranslation(path, None, Some(new_coord)).into()),
						}
					}
				}
			}
			DragLayer(path, offset) => {
				// TODO: Replace root transformations with functions of the transform api
				// and do the same with all instances of `root.transform.inverse()` in other messages
				let transformed_mouse_pos = self.active_document().document.root.transform.inverse().transform_vector2(ipp.mouse.position.as_dvec2());
				let translation = offset + transformed_mouse_pos;
				if let Ok(layer) = self.active_document_mut().document.layer_mut(&path) {
					let transform = {
						let mut transform = layer.transform;
						transform.translation = translation;
						transform.to_cols_array()
					};
					responses.push_back(DocumentOperation::SetLayerTransform { path, transform }.into());
				}
			}
			SetLayerTranslation(path, x_option, y_option) => {
				if let Ok(layer) = self.active_document_mut().document.layer_mut(&path) {
					let mut transform = layer.transform;
					transform.translation = DVec2::new(x_option.unwrap_or(transform.translation.x), y_option.unwrap_or(transform.translation.y));
					responses.push_back(
						DocumentOperation::SetLayerTransform {
							path,
							transform: transform.to_cols_array(),
						}
						.into(),
					);
				}
			}
			RenameLayer(path, name) => responses.push_back(DocumentOperation::RenameLayer { path, name }.into()),
		}
	}
	fn actions(&self) -> ActionList {
		let mut common = actions!(DocumentMessageDiscriminant;
			Undo,
			SelectAllLayers,
			DeselectAllLayers,
			RenderDocument,
			ExportDocument,
		);

		if self.active_document().layer_data.values().any(|data| data.selected) {
			let select = actions!(DocumentMessageDiscriminant;
				DeleteSelectedLayers,
				DuplicateSelectedLayers,
				NudgeSelectedLayers,
				ReorderSelectedLayers,
			);
			common.extend(select);
		}
		common.extend(self.movement_handler.actions());
		common
	}
}

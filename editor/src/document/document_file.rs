pub use super::layer_panel::*;
use crate::{
	consts::{ASYMPTOTIC_EFFECT, FILE_EXPORT_SUFFIX, FILE_SAVE_SUFFIX, SCALE_EFFECT, SCROLLBAR_SPACING},
	frontend::layer_panel::*,
	EditorError,
};
use glam::{DAffine2, DVec2};
use graphene::{document::Document as InternalDocument, layers::LayerDataType, DocumentError, LayerId};
use kurbo::PathSeg;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::input::InputPreprocessor;
use crate::message_prelude::*;
use graphene::layers::BlendMode;
use graphene::{DocumentResponse, Operation as DocumentOperation};
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
	// TODO: Convert Vec<LayerId> into typedef "LayerPath"
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
	SetSelectedLayers(Vec<Vec<LayerId>>),
	AddSelectedLayers(Vec<Vec<LayerId>>),
	SelectAllLayers,
	SelectionChanged,
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
	FolderChanged(Vec<LayerId>),
	StartTransaction,
	RollbackTransaction,
	AbortTransaction,
	CommitTransaction,
	ExportDocument,
	SaveDocument,
	RenderDocument,
	Undo,
	NudgeSelectedLayers(f64, f64),
	AlignSelectedLayers(AlignAxis, AlignAggregate),
	MoveSelectedLayersTo {
		path: Vec<LayerId>,
		insert_index: isize,
	},
	ReorderSelectedLayers(i32), // relative_position,
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
	fn filter_document_responses(&self, document_responses: &mut Vec<DocumentResponse>) -> bool {
		let len = document_responses.len();
		document_responses.retain(|response| !matches!(response, DocumentResponse::DocumentChanged));
		document_responses.len() != len
	}
	pub fn handle_folder_changed(&mut self, path: Vec<LayerId>) -> Option<Message> {
		let _ = self.document.render_root();
		self.layer_data(&path).expanded.then(|| {
			let children = self.layer_panel(path.as_slice()).expect("The provided Path was not valid");
			FrontendMessage::ExpandFolder { path, children }.into()
		})
	}
	fn clear_selection(&mut self) {
		self.layer_data.values_mut().for_each(|layer_data| layer_data.selected = false);
	}
	fn select_layer(&mut self, path: &[LayerId]) -> Option<Message> {
		if self.document.layer(path).ok()?.overlay {
			return None;
		}
		self.layer_data(path).selected = true;
		let data = self.layer_panel_entry(path.to_vec()).ok()?;
		// TODO: Add deduplication
		(!path.is_empty()).then(|| FrontendMessage::UpdateLayer { path: path.to_vec(), data }.into())
	}
	pub fn selected_layers_bounding_box(&self) -> Option<[DVec2; 2]> {
		let paths = self.selected_layers().map(|vec| &vec[..]);
		self.document.combined_viewport_bounding_box(paths)
	}
	pub fn selected_layers_vector_handles(&self) -> Vec<DVec2> {
		let paths = self.selected_layers().map(|vec| &vec[..]);
		let shapes = paths.filter_map(|layer| match &self.document.layer(layer).unwrap().data {
			LayerDataType::Shape(shape) => Some(shape),
			LayerDataType::Folder(_) => None,
		});

		let p2v = |point: kurbo::Point| -> DVec2 { DVec2::from((point.x, point.y)) };

		let points = shapes.map(|shape| {
			shape.path.segments().map(|segment| -> Vec<DVec2> {
				match segment {
					PathSeg::Line(line) => vec![p2v(line.p0), p2v(line.p1)],
					PathSeg::Quad(quad) => vec![p2v(quad.p0), p2v(quad.p1), p2v(quad.p2)],
					PathSeg::Cubic(cubic) => vec![p2v(cubic.p0), p2v(cubic.p1), p2v(cubic.p2), p2v(cubic.p3)],
				}
			})
		});

		points.flatten().flatten().collect::<Vec<DVec2>>()
	}
	pub fn layerdata(&self, path: &[LayerId]) -> &LayerData {
		self.layer_data.get(path).expect("Layerdata does not exist")
	}
	pub fn layerdata_mut(&mut self, path: &[LayerId]) -> &mut LayerData {
		self.layer_data.entry(path.to_vec()).or_insert_with(|| LayerData::new(true))
	}

	pub fn selected_layers(&self) -> impl Iterator<Item = &Vec<LayerId>> {
		self.layer_data.iter().filter_map(|(path, data)| data.selected.then(|| path))
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
				match self.document.indices_for_path(&path) {
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
	pub fn with_name_and_content(name: String, serialized_content: String) -> Result<Self, EditorError> {
		let mut document = Self::with_name(name);
		let internal_document = InternalDocument::with_content(&serialized_content);
		match internal_document {
			Ok(handle) => {
				document.document = handle;
				Ok(document)
			}
			Err(DocumentError::InvalidFile(msg)) => Err(EditorError::Document(msg)),
			_ => Err(EditorError::Document(String::from("Failed to open file"))),
		}
	}

	pub fn layer_data(&mut self, path: &[LayerId]) -> &mut LayerData {
		layer_data(&mut self.layer_data, path)
	}

	pub fn backup(&mut self) {
		self.document_backup = Some(self.document.clone())
	}

	pub fn rollback(&mut self) -> Result<(), EditorError> {
		self.backup();
		self.reset()
	}

	pub fn reset(&mut self) -> Result<(), EditorError> {
		match self.document_backup.take() {
			Some(backup) => {
				self.document = backup;
				Ok(())
			}
			None => Err(EditorError::NoTransactionInProgress),
		}
	}

	pub fn layer_panel_entry(&mut self, path: Vec<LayerId>) -> Result<LayerPanelEntry, EditorError> {
		self.document.render_root();
		let data: LayerData = *layer_data(&mut self.layer_data, &path);
		let layer = self.document.layer(&path)?;
		let entry = layer_panel_entry(&data, self.document.multiply_transforms(&path).unwrap(), layer, path);
		Ok(entry)
	}

	/// Returns a list of `LayerPanelEntry`s intended for display purposes. These don't contain
	/// any actual data, but rather attributes such as visibility and names of the layers.
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
			.filter(|(layer, _)| !layer.overlay)
			.map(|(layer, (path, data))| {
				layer_panel_entry(
					&data,
					self.document.generate_transform_across_scope(path, Some(self.document.root.transform.inverse())).unwrap(),
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
			Movement(message) => self.movement_handler.process_action(message, (layer_data(&mut self.layer_data, &[]), &self.document, ipp), responses),
			DeleteLayer(path) => responses.push_back(DocumentOperation::DeleteLayer { path }.into()),
			AddFolder(path) => responses.push_back(DocumentOperation::AddFolder { path }.into()),
			StartTransaction => self.backup(),
			RollbackTransaction => {
				self.rollback().unwrap_or_else(|e| log::warn!("{}", e));
				responses.extend([DocumentMessage::RenderDocument.into(), self.handle_folder_changed(vec![]).unwrap()]);
			}
			AbortTransaction => {
				self.reset().unwrap_or_else(|e| log::warn!("{}", e));
				responses.extend([DocumentMessage::RenderDocument.into(), self.handle_folder_changed(vec![]).unwrap()]);
			}
			CommitTransaction => self.document_backup = None,
			ExportDocument => {
				let bbox = self.document.visible_layers_bounding_box().unwrap_or([DVec2::ZERO, ipp.viewport_bounds.size()]);
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
							self.document.render_root()
						),
						name,
					}
					.into(),
				)
			}
			SaveDocument => {
				let name = match self.name.ends_with(FILE_SAVE_SUFFIX) {
					true => self.name.clone(),
					false => self.name.clone() + FILE_SAVE_SUFFIX,
				};
				responses.push_back(
					FrontendMessage::SaveDocument {
						document: self.document.serialize_document(),
						name,
					}
					.into(),
				)
			}
			SetBlendModeForSelectedLayers(blend_mode) => {
				for path in self.layer_data.iter().filter_map(|(path, data)| data.selected.then(|| path.clone())) {
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
				self.layer_data(&path).expanded ^= true;
				responses.extend(self.handle_folder_changed(path));
			}
			SelectionChanged => {
				// TODO: Hoist this duplicated code into wider system
				responses.push_back(SelectMessage::SelectedLayersChanged.into());
				responses.push_back(PathMessage::SelectedLayersChanged.into());
			}
			DeleteSelectedLayers => {
				for path in self.selected_layers().cloned() {
					responses.push_back(DocumentOperation::DeleteLayer { path }.into())
				}
				responses.push_back(SelectMessage::SelectedLayersChanged.into());
				responses.push_back(PathMessage::SelectedLayersChanged.into());
			}
			DuplicateSelectedLayers => {
				for path in self.selected_layers_sorted() {
					responses.push_back(DocumentOperation::DuplicateLayer { path }.into())
				}
			}
			SetSelectedLayers(paths) => {
				self.clear_selection();
				responses.push_front(AddSelectedLayers(paths).into());
			}
			AddSelectedLayers(paths) => {
				for path in paths {
					responses.extend(self.select_layer(&path));
				}
				// TODO: Correctly update layer panel in clear_selection instead of here
				responses.extend(self.handle_folder_changed(Vec::new()));
				responses.push_back(SelectMessage::SelectedLayersChanged.into());
				responses.push_back(PathMessage::SelectedLayersChanged.into());
			}
			SelectAllLayers => {
				let all_layer_paths = self
					.layer_data
					.keys()
					.filter(|path| !path.is_empty() && !self.document.layer(path).unwrap().overlay)
					.cloned()
					.collect::<Vec<_>>();
				responses.push_back(SetSelectedLayers(all_layer_paths).into());
			}
			DeselectAllLayers => {
				responses.push_back(SetSelectedLayers(vec![]).into());
			}
			Undo => {
				// this is a temporary fix and will be addressed by #123
				if let Some(id) = self.document.root.as_folder().unwrap().list_layers().last() {
					responses.push_back(DocumentOperation::DeleteLayer { path: vec![*id] }.into())
				}
			}
			FolderChanged(path) => responses.extend(self.handle_folder_changed(path)),
			DispatchOperation(op) => match self.document.handle_operation(&op) {
				Ok(Some(mut document_responses)) => {
					let canvas_dirty = self.filter_document_responses(&mut document_responses);
					responses.extend(
						document_responses
							.into_iter()
							.map(|response| match response {
								DocumentResponse::FolderChanged { path } => self.handle_folder_changed(path),
								DocumentResponse::DeletedLayer { path } => {
									self.layer_data.remove(&path);
									// TODO: Handle PathMessage:SelectedLayersChanged
									Some(SelectMessage::SelectedLayersChanged.into())
								}
								DocumentResponse::LayerChanged { path } => Some(
									FrontendMessage::UpdateLayer {
										path: path.clone(),
										data: self.layer_panel_entry(path).unwrap(),
									}
									.into(),
								),
								DocumentResponse::CreatedLayer { path } => (!self.document.layer(&path).unwrap().overlay).then(|| SetSelectedLayers(vec![path]).into()),
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
			RenderDocument => {
				responses.push_back(
					FrontendMessage::UpdateCanvas {
						document: self.document.render_root(),
					}
					.into(),
				);
				let scale = 0.5 + ASYMPTOTIC_EFFECT + self.layerdata(&[]).scale * SCALE_EFFECT;
				let viewport_size = ipp.viewport_bounds.size();
				let viewport_mid = ipp.viewport_bounds.center();
				let [bounds1, bounds2] = self.document.visible_layers_bounding_box().unwrap_or([viewport_mid; 2]);
				let bounds1 = bounds1.min(viewport_mid) - viewport_size * scale;
				let bounds2 = bounds2.max(viewport_mid) + viewport_size * scale;
				let bounds_length = (bounds2 - bounds1) * (1. + SCROLLBAR_SPACING);
				let scrollbar_position = DVec2::splat(0.5) - (bounds1.lerp(bounds2, 0.5) - viewport_mid) / (bounds_length - viewport_size);
				let scrollbar_multiplier = bounds_length - viewport_size;
				let scrollbar_size = viewport_size / bounds_length;
				responses.push_back(
					FrontendMessage::UpdateScrollbars {
						position: scrollbar_position.into(),
						size: scrollbar_size.into(),
						multiplier: scrollbar_multiplier.into(),
					}
					.into(),
				);
			}

			NudgeSelectedLayers(x, y) => {
				for path in self.selected_layers().cloned() {
					let operation = DocumentOperation::TransformLayerInViewport {
						path,
						transform: DAffine2::from_translation((x, y).into()).to_cols_array(),
					};
					responses.push_back(operation.into());
				}
				responses.push_back(SelectMessage::SelectedLayersChanged.into());
				responses.push_back(PathMessage::SelectedLayersChanged.into());
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
							if let Some(folder) = self.document.layer(path).ok().map(|layer| layer.as_folder().ok()).flatten() {
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
			FlipSelectedLayers(axis) => {
				let scale = match axis {
					FlipAxis::X => DVec2::new(-1., 1.),
					FlipAxis::Y => DVec2::new(1., -1.),
				};
				if let Some([min, max]) = self.document.combined_viewport_bounding_box(self.selected_layers().map(|x| x.as_slice())) {
					let center = (max + min) / 2.;
					let bbox_trans = DAffine2::from_translation(-center);
					for path in self.selected_layers() {
						responses.push_back(
							DocumentOperation::TransformLayerInScope {
								path: path.clone(),
								transform: DAffine2::from_scale(scale).to_cols_array(),
								scope: bbox_trans.to_cols_array(),
							}
							.into(),
						);
					}
					responses.push_back(SelectMessage::SelectedLayersChanged.into());
					responses.push_back(PathMessage::SelectedLayersChanged.into());
				}
			}
			AlignSelectedLayers(axis, aggregate) => {
				let (paths, boxes): (Vec<_>, Vec<_>) = self.selected_layers().filter_map(|path| self.document.viewport_bounding_box(path).ok()?.map(|b| (path, b))).unzip();

				let axis = match axis {
					AlignAxis::X => DVec2::X,
					AlignAxis::Y => DVec2::Y,
				};
				let lerp = |bbox: &[DVec2; 2]| bbox[0].lerp(bbox[1], 0.5);
				if let Some(combined_box) = self.document.combined_viewport_bounding_box(self.selected_layers().map(|x| x.as_slice())) {
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
								path: path.clone(),
								transform: DAffine2::from_translation(translation).to_cols_array(),
							}
							.into(),
						);
					}
					responses.push_back(SelectMessage::SelectedLayersChanged.into());
					responses.push_back(PathMessage::SelectedLayersChanged.into());
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
			SaveDocument,
		);

		if self.layer_data.values().any(|data| data.selected) {
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

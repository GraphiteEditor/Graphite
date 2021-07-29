use crate::message_prelude::*;
use crate::{
	consts::{MOUSE_ZOOM_RATE, VIEWPORT_SCROLL_RATE, VIEWPORT_ZOOM_SCALE_MAX, VIEWPORT_ZOOM_SCALE_MIN, WHEEL_ZOOM_RATE},
	input::{mouse::ViewportPosition, InputPreprocessor},
};
use document_core::layers::BlendMode;
use document_core::layers::Layer;
use document_core::{DocumentResponse, LayerId, Operation as DocumentOperation};
use glam::{DAffine2, DVec2};
use log::warn;
use serde::{Deserialize, Serialize};

use crate::document::Document;
use std::collections::VecDeque;

use super::LayerData;

#[impl_message(Message, Document)]
#[derive(PartialEq, Clone, Debug)]
pub enum DocumentMessage {
	DispatchOperation(DocumentOperation),
	SelectLayers(Vec<Vec<LayerId>>),
	SelectAllLayers,
	DeselectAllLayers,
	DeleteLayer(Vec<LayerId>),
	DeleteSelectedLayers,
	DuplicateSelectedLayers,
	CopySelectedLayers,
	SetBlendModeForSelectedLayers(BlendMode),
	SetOpacityForSelectedLayers(f64),
	PasteLayers { path: Vec<LayerId>, insert_index: isize },
	AddFolder(Vec<LayerId>),
	RenameLayer(Vec<LayerId>, String),
	ToggleLayerVisibility(Vec<LayerId>),
	ToggleLayerExpansion(Vec<LayerId>),
	SelectDocument(usize),
	CloseDocument(usize),
	CloseActiveDocumentWithConfirmation,
	CloseAllDocumentsWithConfirmation,
	CloseAllDocuments,
	NewDocument,
	GetOpenDocumentsList,
	NextDocument,
	PrevDocument,
	ExportDocument,
	RenderDocument,
	Undo,
	MouseMove,
	TranslateCanvasBegin,
	WheelCanvasTranslate { use_y_as_x: bool },
	RotateCanvasBegin { snap: bool },
	EnableSnapping,
	DisableSnapping,
	ZoomCanvasBegin,
	TranslateCanvasEnd,
	SetCanvasZoom(f64),
	MultiplyCanvasZoom(f64),
	WheelCanvasZoom,
	SetCanvasRotation(f64),
	NudgeSelectedLayers(f64, f64),
	FlipSelectedLayers(FlipAxis),
	AlignSelectedLayers(AlignAxis, AlignAggregate),
	DragLayer(Vec<LayerId>, DVec2),
	MoveSelectedLayersTo { path: Vec<LayerId>, insert_index: isize },
	ReorderSelectedLayers(i32), // relative_position,
	SetLayerTranslation(Vec<LayerId>, Option<f64>, Option<f64>),
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum FlipAxis {
	X,
	Y,
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum AlignAxis {
	X,
	Y,
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum AlignAggregate {
	Min,
	Max,
	Center,
	Average,
}

impl From<DocumentOperation> for DocumentMessage {
	fn from(operation: DocumentOperation) -> DocumentMessage {
		Self::DispatchOperation(operation)
	}
}
impl From<DocumentOperation> for Message {
	fn from(operation: DocumentOperation) -> Message {
		DocumentMessage::DispatchOperation(operation).into()
	}
}

#[derive(Debug, Clone)]
pub struct DocumentMessageHandler {
	documents: Vec<Document>,
	active_document_index: usize,
	translating: bool,
	rotating: bool,
	zooming: bool,
	snapping: bool,
	mouse_pos: ViewportPosition,
	copy_buffer: Vec<Layer>,
}

impl DocumentMessageHandler {
	pub fn active_document(&self) -> &Document {
		&self.documents[self.active_document_index]
	}
	pub fn active_document_mut(&mut self) -> &mut Document {
		&mut self.documents[self.active_document_index]
	}
	fn filter_document_responses(&self, document_responses: &mut Vec<DocumentResponse>) -> bool {
		let len = document_responses.len();
		document_responses.retain(|response| !matches!(response, DocumentResponse::DocumentChanged));
		document_responses.len() != len
	}
	fn handle_folder_changed(&mut self, path: Vec<LayerId>) -> Option<Message> {
		let document = self.active_document_mut();
		let _ = document.document.render_root();
		document.layer_data(&path).expanded.then(|| {
			let children = document.layer_panel(path.as_slice()).expect("The provided Path was not valid");
			FrontendMessage::ExpandFolder { path, children }.into()
		})
	}
	fn clear_selection(&mut self) {
		self.active_document_mut().layer_data.values_mut().for_each(|layer_data| layer_data.selected = false);
	}
	fn select_layer(&mut self, path: &[LayerId]) -> Option<Message> {
		self.active_document_mut().layer_data(&path).selected = true;
		// TODO: Add deduplication
		(!path.is_empty()).then(|| self.handle_folder_changed(path[..path.len() - 1].to_vec())).flatten()
	}
	fn selected_layers(&self) -> impl Iterator<Item = &Vec<LayerId>> {
		self.active_document().layer_data.iter().filter_map(|(path, data)| data.selected.then(|| path))
	}
	fn layerdata(&self, path: &[LayerId]) -> &LayerData {
		self.active_document().layer_data.get(path).expect("Layerdata does not exist")
	}
	fn layerdata_mut(&mut self, path: &[LayerId]) -> &mut LayerData {
		self.active_document_mut().layer_data.entry(path.to_vec()).or_insert_with(|| LayerData::new(true))
	}
	fn create_document_transform_from_layerdata(&self, viewport_size: &ViewportPosition, responses: &mut VecDeque<Message>) {
		let half_viewport = viewport_size.as_dvec2() / 2.;
		let layerdata = self.layerdata(&[]);
		let scaled_half_viewport = half_viewport / layerdata.scale;
		responses.push_back(
			DocumentOperation::SetLayerTransform {
				path: vec![],
				transform: layerdata.calculate_offset_transform(scaled_half_viewport).to_cols_array(),
			}
			.into(),
		);
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
}

impl Default for DocumentMessageHandler {
	fn default() -> Self {
		Self {
			documents: vec![Document::default()],
			active_document_index: 0,
			translating: false,
			rotating: false,
			zooming: false,
			snapping: false,
			mouse_pos: ViewportPosition::default(),
			copy_buffer: vec![],
		}
	}
}

impl MessageHandler<DocumentMessage, &InputPreprocessor> for DocumentMessageHandler {
	fn process_action(&mut self, message: DocumentMessage, ipp: &InputPreprocessor, responses: &mut VecDeque<Message>) {
		use DocumentMessage::*;
		match message {
			DeleteLayer(path) => responses.push_back(DocumentOperation::DeleteLayer { path }.into()),
			AddFolder(path) => responses.push_back(DocumentOperation::AddFolder { path }.into()),
			SelectDocument(id) => {
				assert!(id < self.documents.len(), "Tried to select a document that was not initialized");
				self.active_document_index = id;
				responses.push_back(
					FrontendMessage::SetActiveDocument {
						document_index: self.active_document_index,
					}
					.into(),
				);
				responses.push_back(RenderDocument.into());
			}
			CloseActiveDocumentWithConfirmation => {
				responses.push_back(
					FrontendMessage::DisplayConfirmationToCloseDocument {
						document_index: self.active_document_index,
					}
					.into(),
				);
			}
			CloseAllDocumentsWithConfirmation => {
				responses.push_back(FrontendMessage::DisplayConfirmationToCloseAllDocuments.into());
			}
			CloseAllDocuments => {
				// Empty the list of internal document data
				self.documents.clear();

				// Create a new blank document
				responses.push_back(DocumentMessage::NewDocument.into());
			}
			CloseDocument(id) => {
				assert!(id < self.documents.len(), "Tried to select a document that was not initialized");
				// Remove doc from the backend store; use `id` as client tabs and backend documents will be in sync
				self.documents.remove(id);

				// Send the new list of document tab names
				let open_documents = self.documents.iter().map(|doc| doc.name.clone()).collect();
				responses.push_back(FrontendMessage::UpdateOpenDocumentsList { open_documents }.into());

				// Last tab was closed, so create a new blank tab
				if self.documents.is_empty() {
					self.active_document_index = 0;
					responses.push_back(DocumentMessage::NewDocument.into());
				}
				// The currently selected doc is being closed
				else if id == self.active_document_index {
					// The currently selected tab was the rightmost tab
					if id == self.documents.len() {
						self.active_document_index -= 1;
					}

					let lp = self.active_document_mut().layer_panel(&[]).expect("Could not get panel for active doc");
					responses.push_back(FrontendMessage::ExpandFolder { path: Vec::new(), children: lp }.into());
					responses.push_back(
						FrontendMessage::SetActiveDocument {
							document_index: self.active_document_index,
						}
						.into(),
					);
					responses.push_back(
						FrontendMessage::UpdateCanvas {
							document: self.active_document_mut().document.render_root(),
						}
						.into(),
					);
				}
				// Active doc will move one space to the left
				else if id < self.active_document_index {
					self.active_document_index -= 1;
					responses.push_back(
						FrontendMessage::SetActiveDocument {
							document_index: self.active_document_index,
						}
						.into(),
					);
				}
			}
			NewDocument => {
				let digits = ('0'..='9').collect::<Vec<char>>();
				let mut doc_title_numbers = self
					.documents
					.iter()
					.map(|d| {
						if d.name.ends_with(digits.as_slice()) {
							let (_, number) = d.name.split_at(17);
							number.trim().parse::<usize>().unwrap()
						} else {
							1
						}
					})
					.collect::<Vec<usize>>();
				doc_title_numbers.sort_unstable();
				let mut new_doc_title_num = 1;
				while new_doc_title_num <= self.documents.len() {
					if new_doc_title_num != doc_title_numbers[new_doc_title_num - 1] {
						break;
					}
					new_doc_title_num += 1;
				}
				let name = match new_doc_title_num {
					1 => "Untitled Document".to_string(),
					_ => format!("Untitled Document {}", new_doc_title_num),
				};

				self.active_document_index = self.documents.len();
				let new_document = Document::with_name(name);
				self.documents.push(new_document);

				// Send the new list of document tab names
				let open_documents = self.documents.iter().map(|doc| doc.name.clone()).collect();
				responses.push_back(FrontendMessage::UpdateOpenDocumentsList { open_documents }.into());

				responses.push_back(
					FrontendMessage::ExpandFolder {
						path: Vec::new(),
						children: Vec::new(),
					}
					.into(),
				);
				responses.push_back(SelectDocument(self.active_document_index).into());
			}
			GetOpenDocumentsList => {
				// Send the list of document tab names
				let open_documents = self.documents.iter().map(|doc| doc.name.clone()).collect();
				responses.push_back(FrontendMessage::UpdateOpenDocumentsList { open_documents }.into());
			}
			NextDocument => {
				let id = (self.active_document_index + 1) % self.documents.len();
				responses.push_back(SelectDocument(id).into());
			}
			PrevDocument => {
				let id = (self.active_document_index + self.documents.len() - 1) % self.documents.len();
				responses.push_back(SelectDocument(id).into());
			}
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
				for path in self.selected_layers().cloned() {
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
			CopySelectedLayers => {
				let paths = self.selected_layers_sorted();
				self.copy_buffer.clear();
				for path in paths {
					match self.active_document().document.layer(&path).map(|t| t.clone()) {
						Ok(layer) => {
							self.copy_buffer.push(layer);
						}
						Err(e) => warn!("Could not access selected layer {:?}: {:?}", path, e),
					}
				}
			}
			PasteLayers { path, insert_index } => {
				let paste = |layer: &Layer, responses: &mut VecDeque<_>| {
					log::trace!("pasting into folder {:?} as index: {}", path, insert_index);
					responses.push_back(
						DocumentOperation::PasteLayer {
							layer: layer.clone(),
							path: path.clone(),
							insert_index,
						}
						.into(),
					)
				};
				if insert_index == -1 {
					for layer in self.copy_buffer.iter() {
						paste(layer, responses)
					}
				} else {
					for layer in self.copy_buffer.iter().rev() {
						paste(layer, responses)
					}
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
			DispatchOperation(op) => {
				if let Ok(Some(mut document_responses)) = self.active_document_mut().document.handle_operation(op) {
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
								DocumentResponse::CreatedLayer { path } => {
									if !self.active_document().document.work_mounted {
										self.select_layer(&path)
									} else {
										None
									}
								}
								DocumentResponse::DocumentChanged => unreachable!(),
							})
							.flatten(),
					);
					if canvas_dirty {
						responses.push_back(RenderDocument.into())
					}
				}
			}
			RenderDocument => responses.push_back(
				FrontendMessage::UpdateCanvas {
					document: self.active_document_mut().document.render_root(),
				}
				.into(),
			),
			TranslateCanvasBegin => {
				self.translating = true;
				self.mouse_pos = ipp.mouse.position;
			}

			RotateCanvasBegin { snap } => {
				self.rotating = true;
				self.snapping = snap;
				let layerdata = self.layerdata_mut(&[]);
				layerdata.snap_rotate = snap;
				self.mouse_pos = ipp.mouse.position;
			}
			EnableSnapping => self.snapping = true,
			DisableSnapping => self.snapping = false,
			ZoomCanvasBegin => {
				self.zooming = true;
				self.mouse_pos = ipp.mouse.position;
			}
			TranslateCanvasEnd => {
				let layerdata = self.layerdata_mut(&[]);
				layerdata.rotation = layerdata.snapped_angle();
				layerdata.snap_rotate = false;
				self.translating = false;
				self.rotating = false;
				self.zooming = false;
			}
			MouseMove => {
				if self.translating {
					let delta = ipp.mouse.position.as_dvec2() - self.mouse_pos.as_dvec2();
					let transformed_delta = self.active_document().document.root.transform.inverse().transform_vector2(delta);

					let layerdata = self.layerdata_mut(&[]);
					layerdata.translation += transformed_delta;
					self.create_document_transform_from_layerdata(&ipp.viewport_size, responses);
				}
				if self.rotating {
					let half_viewport = ipp.viewport_size.as_dvec2() / 2.;
					let rotation = {
						let start_vec = self.mouse_pos.as_dvec2() - half_viewport;
						let end_vec = ipp.mouse.position.as_dvec2() - half_viewport;
						start_vec.angle_between(end_vec)
					};

					let snapping = self.snapping;
					let layerdata = self.layerdata_mut(&[]);
					layerdata.rotation += rotation;
					layerdata.snap_rotate = snapping;
					responses.push_back(
						FrontendMessage::SetCanvasRotation {
							new_radians: layerdata.snapped_angle(),
						}
						.into(),
					);
					self.create_document_transform_from_layerdata(&ipp.viewport_size, responses);
				}
				if self.zooming {
					let difference = self.mouse_pos.y as f64 - ipp.mouse.position.y as f64;
					let amount = 1. + difference * MOUSE_ZOOM_RATE;
					let layerdata = self.layerdata_mut(&[]);
					let new = (layerdata.scale * amount).clamp(VIEWPORT_ZOOM_SCALE_MIN, VIEWPORT_ZOOM_SCALE_MAX);
					layerdata.scale = new;
					responses.push_back(FrontendMessage::SetCanvasZoom { new_zoom: layerdata.scale }.into());
					self.create_document_transform_from_layerdata(&ipp.viewport_size, responses);
				}
				self.mouse_pos = ipp.mouse.position;
			}
			SetCanvasZoom(new) => {
				let layerdata = self.layerdata_mut(&[]);
				layerdata.scale = new.clamp(VIEWPORT_ZOOM_SCALE_MIN, VIEWPORT_ZOOM_SCALE_MAX);
				responses.push_back(FrontendMessage::SetCanvasZoom { new_zoom: layerdata.scale }.into());
				self.create_document_transform_from_layerdata(&ipp.viewport_size, responses);
			}
			MultiplyCanvasZoom(multiplier) => {
				let layerdata = self.layerdata_mut(&[]);
				let new = (layerdata.scale * multiplier).clamp(VIEWPORT_ZOOM_SCALE_MIN, VIEWPORT_ZOOM_SCALE_MAX);
				layerdata.scale = new;
				responses.push_back(FrontendMessage::SetCanvasZoom { new_zoom: layerdata.scale }.into());
				self.create_document_transform_from_layerdata(&ipp.viewport_size, responses);
			}
			WheelCanvasZoom => {
				let scroll = ipp.mouse.scroll_delta.scroll_delta();
				let mouse = ipp.mouse.position.as_dvec2();
				let viewport_size = ipp.viewport_size.as_dvec2();
				let mut zoom_factor = 1. + scroll.abs() * WHEEL_ZOOM_RATE;
				if ipp.mouse.scroll_delta.y > 0 {
					zoom_factor = 1. / zoom_factor
				};
				let new_viewport_size = viewport_size * (1. / zoom_factor);
				let delta_size = viewport_size - new_viewport_size;
				let mouse_percent = mouse / viewport_size;
				let delta = delta_size * -2. * (mouse_percent - (0.5, 0.5).into());

				let transformed_delta = self.active_document().document.root.transform.inverse().transform_vector2(delta);
				let layerdata = self.layerdata_mut(&[]);
				let new = (layerdata.scale * zoom_factor).clamp(VIEWPORT_ZOOM_SCALE_MIN, VIEWPORT_ZOOM_SCALE_MAX);
				layerdata.scale = new;
				layerdata.translation += transformed_delta;
				responses.push_back(FrontendMessage::SetCanvasZoom { new_zoom: layerdata.scale }.into());
				self.create_document_transform_from_layerdata(&ipp.viewport_size, responses);
			}
			WheelCanvasTranslate { use_y_as_x } => {
				let delta = match use_y_as_x {
					false => -ipp.mouse.scroll_delta.as_dvec2(),
					true => (-ipp.mouse.scroll_delta.y as f64, 0.).into(),
				} * VIEWPORT_SCROLL_RATE;
				let transformed_delta = self.active_document().document.root.transform.inverse().transform_vector2(delta);
				let layerdata = self.layerdata_mut(&[]);
				layerdata.translation += transformed_delta;
				self.create_document_transform_from_layerdata(&ipp.viewport_size, responses);
			}
			SetCanvasRotation(new) => {
				let layerdata = self.layerdata_mut(&[]);
				layerdata.rotation = new;
				self.create_document_transform_from_layerdata(&ipp.viewport_size, responses);
				responses.push_back(FrontendMessage::SetCanvasRotation { new_radians: new }.into());
			}
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
				responses.push_back(DocumentMessage::CopySelectedLayers.into());
				responses.push_back(DocumentMessage::DeleteSelectedLayers.into());
				responses.push_back(DocumentMessage::PasteLayers { path, insert_index }.into());
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
							if let Some(folder) = self.active_document().document.document_layer(path).ok().map(|layer| layer.as_folder().ok()).flatten() {
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
				// TODO: Handle folder nested transforms with the transforms API
				let selected_paths = self.selected_layers_sorted();
				if selected_paths.is_empty() {
					return;
				}

				let selected_layers = selected_paths.iter().filter_map(|path| {
					let layer = self.active_document().document.layer(path).ok()?;
					// TODO: Refactor with `reduce` and `merge_bounding_boxes` once the latter is added
					let (min, max) = {
						let bounding_box = layer.current_bounding_box()?;
						match axis {
							FlipAxis::X => (bounding_box[0].x, bounding_box[1].x),
							FlipAxis::Y => (bounding_box[0].y, bounding_box[1].y),
						}
					};
					Some((path.clone(), (min, max)))
				});

				let (min, max) = selected_layers
					.clone()
					.map(|(_, extrema)| extrema)
					.reduce(|(min_a, max_a), (min_b, max_b)| (min_a.min(min_b), max_a.max(max_b)))
					.unwrap();
				let middle = (min + max) / 2.;

				for (path, _) in selected_layers {
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
			message => todo!("document_action_handler does not implement: {}", message.to_discriminant().global_name()),
		}
	}
	fn actions(&self) -> ActionList {
		let mut common = actions!(DocumentMessageDiscriminant;
			Undo,
			SelectAllLayers,
			DeselectAllLayers,
			RenderDocument,
			ExportDocument,
			NewDocument,
			CloseActiveDocumentWithConfirmation,
			CloseAllDocumentsWithConfirmation,
			CloseAllDocuments,
			NextDocument,
			PrevDocument,
			MouseMove,
			TranslateCanvasEnd,
			TranslateCanvasBegin,
			PasteLayers,
			RotateCanvasBegin,
			ZoomCanvasBegin,
			SetCanvasZoom,
			MultiplyCanvasZoom,
			SetCanvasRotation,
			WheelCanvasZoom,
			WheelCanvasTranslate,
		);

		if self.active_document().layer_data.values().any(|data| data.selected) {
			let select = actions!(DocumentMessageDiscriminant;
				DeleteSelectedLayers,
				DuplicateSelectedLayers,
				CopySelectedLayers,
				NudgeSelectedLayers,
				ReorderSelectedLayers,
			);
			common.extend(select);
		}
		if self.rotating {
			let snapping = actions!(DocumentMessageDiscriminant;
				EnableSnapping,
				DisableSnapping,
			);
			common.extend(snapping);
		}
		common
	}
}

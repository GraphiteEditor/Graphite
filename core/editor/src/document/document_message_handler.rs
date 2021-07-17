use crate::message_prelude::*;
use crate::{
	consts::{MOUSE_ZOOM_RATE, VIEWPORT_SCROLL_RATE, VIEWPORT_ZOOM_SCALE_MAX, VIEWPORT_ZOOM_SCALE_MIN, WHEEL_ZOOM_RATE},
	input::{mouse::ViewportPosition, InputPreprocessor},
};
use document_core::layers::Layer;
use document_core::{DocumentResponse, LayerId, Operation as DocumentOperation};
use glam::DAffine2;
use log::warn;

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
	PasteLayers,
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
	ReorderSelectedLayer(i32),
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
	fn layerdata(&self, path: &[LayerId]) -> &LayerData {
		self.active_document().layer_data.get(path).expect("Layerdata does not exist")
	}
	fn layerdata_mut(&mut self, path: &[LayerId]) -> &mut LayerData {
		self.active_document_mut().layer_data.entry(path.to_vec()).or_insert_with(|| LayerData::new(true))
	}
	#[allow(dead_code)]
	fn create_transform_from_layerdata(&self, path: Vec<u64>, responses: &mut VecDeque<Message>) {
		let layerdata = self.layerdata(&path);
		responses.push_back(
			DocumentOperation::SetLayerTransform {
				path,
				transform: layerdata.calculate_transform().to_cols_array(),
			}
			.into(),
		);
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

	/// Returns the paths to all layers in order, optionally including only selected layers
	fn layers_sorted(&self, only_selected: bool) -> Vec<Vec<LayerId>> {
		// Compute the indices for each layer to be able to sort them
		// TODO: Replace with drain_filter https://github.com/rust-lang/rust/issues/59618
		let mut layers_with_indices: Vec<(Vec<LayerId>, Vec<usize>)> = self
			.active_document()
			.layer_data
			.iter()
			// 'path.len() > 0' filters out root layer since it has no indices
			.filter_map(|(path, data)| (!path.is_empty() && !only_selected || data.selected).then(|| path.clone()))
			.filter_map(|path| {
				// Currently it is possible that layer_data contains layers that are don't actually exist
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
	fn all_layers_sorted(&self) -> Vec<Vec<LayerId>> {
		self.layers_sorted(false)
	}

	/// Returns the paths to all selected layers in order
	fn selected_layers_sorted(&self) -> Vec<Vec<LayerId>> {
		self.layers_sorted(true)
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
			ToggleLayerVisibility(path) => {
				responses.push_back(DocumentOperation::ToggleVisibility { path }.into());
			}
			ToggleLayerExpansion(path) => {
				self.active_document_mut().layer_data(&path).expanded ^= true;
				responses.extend(self.handle_folder_changed(path));
			}
			DeleteSelectedLayers => {
				let paths = self.selected_layers_sorted();
				for path in paths {
					self.active_document_mut().layer_data.remove(&path);
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
			PasteLayers => {
				for layer in self.copy_buffer.iter() {
					//TODO: Should be the path to the current folder instead of root
					responses.push_back(DocumentOperation::PasteLayer { layer: layer.clone(), path: vec![] }.into())
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
								DocumentResponse::SelectLayer { path } => {
									if !self.active_document().document.work_mounted {
										self.clear_selection();
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
				let paths: Vec<Vec<LayerId>> = self.selected_layers_sorted();

				let delta = {
					let root_layer_rotation = self.layerdata_mut(&[]).rotation;
					let rotate_to_viewport_space = DAffine2::from_angle(root_layer_rotation).inverse();
					rotate_to_viewport_space.transform_point2((x, y).into())
				};
				for path in paths {
					let operation = DocumentOperation::TransformLayer {
						path,
						transform: DAffine2::from_translation(delta).to_cols_array(),
					};
					responses.push_back(operation.into());
				}
			}
			ReorderSelectedLayer(delta) => {
				let mut paths: Vec<Vec<LayerId>> = self.selected_layers_sorted();
				// TODO: Support moving more than one layer
				if paths.len() == 1 {
					let all_layer_paths = self.all_layers_sorted();

					let max_index = all_layer_paths.len() as i64 - 1;

					let mut selected_layer_index = -1;
					let mut next_layer_index = -1;
					for (i, path) in all_layer_paths.iter().enumerate() {
						if *path == paths[0] {
							selected_layer_index = i as i32;
							next_layer_index = (selected_layer_index as i64 + delta as i64).clamp(0, max_index) as i32;
							break;
						}
					}

					if next_layer_index != -1 && next_layer_index != selected_layer_index {
						let operation = DocumentOperation::ReorderLayers {
							source_path: paths.drain(0..1).next().unwrap(),
							target_path: all_layer_paths[next_layer_index as usize].to_vec(),
						};
						responses.push_back(operation.into());
					}
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
				ReorderSelectedLayer,
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

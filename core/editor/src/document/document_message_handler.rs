use crate::{
	input::{mouse::ViewportPosition, InputPreprocessor},
	message_prelude::*,
};
use document_core::{DocumentResponse, LayerId, Operation as DocumentOperation};
use glam::DVec2;

use crate::document::Document;
use std::collections::VecDeque;

#[impl_message(Message, Document)]
#[derive(PartialEq, Clone, Debug)]
pub enum DocumentMessage {
	DispatchOperation(DocumentOperation),
	SelectLayers(Vec<Vec<LayerId>>),
	DeleteLayer(Vec<LayerId>),
	DeleteSelectedLayers,
	DuplicateSelectedLayers,
	AddFolder(Vec<LayerId>),
	RenameLayer(Vec<LayerId>, String),
	ToggleLayerVisibility(Vec<LayerId>),
	ToggleLayerExpansion(Vec<LayerId>),
	SelectDocument(usize),
	CloseDocument(usize),
	CloseActiveDocument,
	NewDocument,
	NextDocument,
	PrevDocument,
	ExportDocument,
	RenderDocument,
	Undo,
	MouseMove,
	TranslateDown,
	WheelTranslate,
	RotateDown,
	ZoomDown,
	TransformUp,
	SetZoom(f64),
	MultiplyZoom(f64),
	WheelZoom,
	SetRotation(f64),
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
	active_document: usize,
	translating: bool,
	rotating: bool,
	zooming: bool,
	mouse_pos: ViewportPosition,
}

impl DocumentMessageHandler {
	pub fn active_document(&self) -> &Document {
		&self.documents[self.active_document]
	}
	pub fn active_document_mut(&mut self) -> &mut Document {
		&mut self.documents[self.active_document]
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
	fn get_layerdata_mut(&mut self, path: Vec<u64>) -> &mut super::LayerData {
		self.active_document_mut().layer_data.get_mut(&path).expect("Layerdata does not exist")
	}
	fn _create_transform_from_layerdata(&self, path: Vec<u64>, responses: &mut VecDeque<Message>) {
		let layerdata = self.active_document().layer_data.get(&path).expect("Layerdata does not exist");
		responses.push_back(
			DocumentOperation::SetLayerTransform {
				path: path,
				transform: layerdata.get_transform().to_cols_array(),
			}
			.into(),
		);
	}
	fn create_document_transform_from_layerdata(&self, ipp: &InputPreprocessor, responses: &mut VecDeque<Message>) {
		let half_viewport = DVec2::new(ipp.viewport_size.x as f64 / 2., ipp.viewport_size.y as f64 / 2.);
		let layerdata = self.active_document().layer_data.get(&vec![]).expect("Layerdata does not exist");
		let scaled_half_viewport = half_viewport / layerdata.scale;
		responses.push_back(
			DocumentOperation::SetLayerTransform {
				path: vec![],
				transform: layerdata.get_offset_transform(scaled_half_viewport).to_cols_array(),
			}
			.into(),
		);
	}
}

impl Default for DocumentMessageHandler {
	fn default() -> Self {
		Self {
			documents: vec![Document::default()],
			active_document: 0,
			translating: false,
			rotating: false,
			zooming: false,
			mouse_pos: ViewportPosition::default(),
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
				self.active_document = id;
				responses.push_back(FrontendMessage::SetActiveDocument { document_index: self.active_document }.into());
				responses.push_back(RenderDocument.into());
			}
			CloseActiveDocument => {
				responses.push_back(FrontendMessage::PromptCloseConfirmationModal.into());
			}
			CloseDocument(id) => {
				assert!(id < self.documents.len(), "Tried to select a document that was not initialized");
				// Remove doc from the backend store. Use 'id' as FE tabs and BE documents will be in sync.
				self.documents.remove(id);
				responses.push_back(FrontendMessage::CloseDocument { document_index: id }.into());

				// Last tab was closed, so create a new blank tab
				if self.documents.is_empty() {
					self.active_document = 0;
					responses.push_back(DocumentMessage::NewDocument.into());
				}
				// The currently selected doc is being closed
				else if id == self.active_document {
					// The currently selected tab was the rightmost tab
					if id == self.documents.len() {
						self.active_document -= 1;
					}

					let lp = self.active_document_mut().layer_panel(&[]).expect("Could not get panel for active doc");
					responses.push_back(FrontendMessage::ExpandFolder { path: Vec::new(), children: lp }.into());
					responses.push_back(FrontendMessage::SetActiveDocument { document_index: self.active_document }.into());
					responses.push_back(
						FrontendMessage::UpdateCanvas {
							document: self.active_document_mut().document.render_root(),
						}
						.into(),
					);
				}
				// Active doc will move one space to the left
				else if id < self.active_document {
					self.active_document -= 1;
					responses.push_back(FrontendMessage::SetActiveDocument { document_index: self.active_document }.into());
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
				doc_title_numbers.sort();
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

				self.active_document = self.documents.len();
				let new_document = Document::with_name(name);
				self.documents.push(new_document);
				responses.push_back(
					FrontendMessage::NewDocument {
						document_name: self.active_document().name.clone(),
					}
					.into(),
				);

				responses.push_back(
					FrontendMessage::ExpandFolder {
						path: Vec::new(),
						children: Vec::new(),
					}
					.into(),
				);
				responses.push_back(SelectDocument(self.active_document).into());
			}
			NextDocument => {
				let id = (self.active_document + 1) % self.documents.len();
				responses.push_back(SelectDocument(id).into());
			}
			PrevDocument => {
				let id = (self.active_document + self.documents.len() - 1) % self.documents.len();
				responses.push_back(SelectDocument(id).into());
			}
			ExportDocument => responses.push_back(
				FrontendMessage::ExportDocument {
					//TODO: Add canvas size instead of using 1080p per default
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
				// TODO: Replace with drain_filter https://github.com/rust-lang/rust/issues/59618
				let paths: Vec<Vec<LayerId>> = self.active_document().layer_data.iter().filter_map(|(path, data)| data.selected.then(|| path.clone())).collect();
				for path in paths {
					self.active_document_mut().layer_data.remove(&path);
					responses.push_back(DocumentOperation::DeleteLayer { path }.into())
				}
			}
			DuplicateSelectedLayers => {
				for path in self.active_document().layer_data.iter().filter_map(|(path, data)| data.selected.then(|| path.clone())) {
					responses.push_back(DocumentOperation::DuplicateLayer { path }.into())
				}
			}
			SelectLayers(paths) => {
				self.clear_selection();
				for path in paths {
					responses.extend(self.select_layer(&path));
				}
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
			TranslateDown => {
				self.translating = true;
				self.mouse_pos = ipp.mouse.position;
			}
			RotateDown => {
				self.rotating = true;
				self.mouse_pos = ipp.mouse.position;
			}
			ZoomDown => {
				self.zooming = true;
				self.mouse_pos = ipp.mouse.position;
			}
			TransformUp => {
				self.translating = false;
				self.rotating = false;
				self.zooming = false;
			}
			MouseMove => {
				if self.translating {
					let delta = DVec2::new(ipp.mouse.position.x as f64 - self.mouse_pos.x as f64, ipp.mouse.position.y as f64 - self.mouse_pos.y as f64);
					let transformed_delta = self.active_document().document.root.transform.inverse().transform_vector2(delta);

					let layerdata = self.get_layerdata_mut(vec![]);
					layerdata.translation = layerdata.translation + transformed_delta;
					self.create_document_transform_from_layerdata(ipp, responses);
				}
				if self.rotating {
					let half_viewport = DVec2::new(ipp.viewport_size.x as f64 / 2., ipp.viewport_size.y as f64 / 2.);
					let rotation = {
						let start_vec = DVec2::new(self.mouse_pos.x as f64, self.mouse_pos.y as f64) - half_viewport;
						let end_vec = DVec2::new(ipp.mouse.position.x as f64, ipp.mouse.position.y as f64) - half_viewport;
						start_vec.angle_between(end_vec)
					};

					let layerdata = self.get_layerdata_mut(vec![]);
					layerdata.rotation += rotation;
					responses.push_back(FrontendMessage::SetRotation { new_radians: layerdata.rotation }.into());
					self.create_document_transform_from_layerdata(ipp, responses);
				}
				if self.zooming {
					let difference = self.mouse_pos.y as f64 - ipp.mouse.position.y as f64;
					let amount = 1. + difference / 100.;
					let layerdata = self.get_layerdata_mut(vec![]);
					layerdata.scale *= amount;
					responses.push_back(FrontendMessage::SetZoom { new_zoom: layerdata.scale }.into());
					self.create_document_transform_from_layerdata(ipp, responses);
				}
				self.mouse_pos = ipp.mouse.position;
			}
			SetZoom(new) => {
				let layerdata = self.get_layerdata_mut(vec![]);
				layerdata.scale = new;
				responses.push_back(FrontendMessage::SetZoom { new_zoom: layerdata.scale }.into());
				self.create_document_transform_from_layerdata(ipp, responses);
			}
			MultiplyZoom(mult) => {
				let layerdata = self.get_layerdata_mut(vec![]);
				layerdata.scale *= mult;
				responses.push_back(FrontendMessage::SetZoom { new_zoom: layerdata.scale }.into());
				self.create_document_transform_from_layerdata(ipp, responses);
			}
			WheelZoom => {
				let scroll = (ipp.mouse.scroll_delta.y + ipp.mouse.scroll_delta.x) as f64;
				let amount = if ipp.mouse.scroll_delta.y > 0 { 1. + scroll / -500. } else { 1. / (1. + scroll / 500.) };
				let layerdata = self.get_layerdata_mut(vec![]);
				layerdata.scale *= amount;
				responses.push_back(FrontendMessage::SetZoom { new_zoom: layerdata.scale }.into());
				self.create_document_transform_from_layerdata(ipp, responses);
			}
			WheelTranslate => {
				let delta = DVec2::new(-ipp.mouse.scroll_delta.x as f64, -ipp.mouse.scroll_delta.y as f64);
				let transformed_delta = self.active_document().document.root.transform.inverse().transform_vector2(delta);
				let layerdata = self.get_layerdata_mut(vec![]);
				layerdata.translation += transformed_delta;
				self.create_document_transform_from_layerdata(ipp, responses);
			}
			SetRotation(new) => {
				let layerdata = self.get_layerdata_mut(vec![]);
				layerdata.rotation = new;
				self.create_document_transform_from_layerdata(ipp, responses);
				responses.push_back(FrontendMessage::SetRotation { new_radians: new }.into());
			}
			message => todo!("document_action_handler does not implement: {}", message.to_discriminant().global_name()),
		}
	}
	fn actions(&self) -> ActionList {
		if self.active_document().layer_data.values().any(|data| data.selected) {
			actions!(DocumentMessageDiscriminant; Undo, DeleteSelectedLayers, DuplicateSelectedLayers, RenderDocument, ExportDocument, NewDocument, CloseActiveDocument, NextDocument, PrevDocument, MouseMove, TransformUp, TranslateDown, RotateDown, ZoomDown, SetZoom, SetRotation, WheelZoom, WheelTranslate)
		} else {
			actions!(DocumentMessageDiscriminant; Undo, RenderDocument, ExportDocument, NewDocument, CloseActiveDocument, NextDocument, PrevDocument, MouseMove, TransformUp, TranslateDown, RotateDown, ZoomDown, SetZoom, SetRotation, WheelZoom, WheelTranslate)
		}
	}
}

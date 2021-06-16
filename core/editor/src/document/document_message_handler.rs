use crate::message_prelude::*;
use document_core::{DocumentResponse, LayerId, Operation as DocumentOperation};

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
	NewDocument,
	NextDocument,
	PrevDocument,
	ExportDocument,
	RenderDocument,
	Undo,
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
}

impl Default for DocumentMessageHandler {
	fn default() -> Self {
		Self {
			documents: vec![Document::default()],
			active_document: 0,
		}
	}
}

impl MessageHandler<DocumentMessage, ()> for DocumentMessageHandler {
	fn process_action(&mut self, message: DocumentMessage, _data: (), responses: &mut VecDeque<Message>) {
		use DocumentMessage::*;
		match message {
			DeleteLayer(path) => responses.push_back(DocumentOperation::DeleteLayer { path }.into()),
			AddFolder(path) => responses.push_back(DocumentOperation::AddFolder { path }.into()),
			SelectDocument(id) => {
				assert!(id < self.documents.len(), "Tried to select a document that was not initialized");
				self.active_document = id;
				responses.push_back(FrontendMessage::SetActiveDocument { document_index: self.active_document }.into());
				responses.push_back(
					FrontendMessage::UpdateCanvas {
						document: self.active_document_mut().document.render_root(),
					}
					.into(),
				);
			}
			NewDocument => {
				self.active_document = self.documents.len();
				let new_document = Document::with_name(format!("Untitled Document {}", self.active_document + 1));
				self.documents.push(new_document);
				responses.push_back(
					FrontendMessage::NewDocument {
						document_name: self.active_document().name.clone(),
					}
					.into(),
				);
				responses.push_back(FrontendMessage::SetActiveDocument { document_index: self.active_document }.into());
				responses.push_back(
					FrontendMessage::UpdateCanvas {
						document: self.active_document_mut().document.render_root(),
					}
					.into(),
				);
			}
			NextDocument => {
				self.active_document = (self.active_document + 1) % self.documents.len();
				responses.push_back(FrontendMessage::SetActiveDocument { document_index: self.active_document }.into());
				responses.push_back(
					FrontendMessage::UpdateCanvas {
						document: self.active_document_mut().document.render_root(),
					}
					.into(),
				);
			}
			PrevDocument => {
				self.active_document = (self.active_document + self.documents.len() - 1) % self.documents.len();
				responses.push_back(FrontendMessage::SetActiveDocument { document_index: self.active_document }.into());
				responses.push_back(
					FrontendMessage::UpdateCanvas {
						document: self.active_document_mut().document.render_root(),
					}
					.into(),
				);
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
				// TODO: Replace with drain_filter https://github.com/rust-lang/rust/issues/59618
				let paths: Vec<Vec<LayerId>> = self.active_document().layer_data.iter().filter_map(|(path, data)| data.selected.then(|| path.clone())).collect();
				for path in paths {
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
				if let Some(id) = self.active_document().document.root.list_layers().last() {
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
			message => todo!("document_action_handler does not implement: {}", message.to_discriminant().global_name()),
		}
	}
	fn actions(&self) -> ActionList {
		if self.active_document().layer_data.values().any(|data| data.selected) {
			actions!(DocumentMessageDiscriminant; Undo, DeleteSelectedLayers, DuplicateSelectedLayers, RenderDocument, ExportDocument, NewDocument, NextDocument, PrevDocument)
		} else {
			actions!(DocumentMessageDiscriminant; Undo, RenderDocument, ExportDocument, NewDocument, NextDocument, PrevDocument)
		}
	}
}

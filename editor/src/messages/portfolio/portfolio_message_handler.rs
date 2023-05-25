use std::sync::Arc;

use super::utility_types::PersistentData;
use crate::application::generate_uuid;
use crate::consts::{DEFAULT_DOCUMENT_NAME, GRAPHITE_DOCUMENT_VERSION};
use crate::messages::dialog::simple_dialogs;
use crate::messages::frontend::utility_types::FrontendDocumentDetails;
use crate::messages::layout::utility_types::layout_widget::PropertyHolder;
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::portfolio::document::node_graph::IMAGINATE_NODE;
use crate::messages::portfolio::document::utility_types::clipboards::{Clipboard, CopyBufferEntry, INTERNAL_CLIPBOARD_COUNT};
use crate::messages::portfolio::utility_types::ImaginateServerStatus;
use crate::messages::prelude::*;
use crate::messages::tool::utility_types::{HintData, HintGroup};
use crate::node_graph_executor::NodeGraphExecutor;

use document_legacy::layers::layer_info::LayerDataType;
use document_legacy::layers::style::RenderData;
use document_legacy::Operation as DocumentOperation;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeId, NodeInput};
use graphene_core::raster::Image;
use graphene_core::text::Font;

#[derive(Debug, Default)]
pub struct PortfolioMessageHandler {
	menu_bar_message_handler: MenuBarMessageHandler,
	documents: HashMap<u64, DocumentMessageHandler>,
	document_ids: Vec<u64>,
	pub executor: NodeGraphExecutor,
	active_document_id: Option<u64>,
	copy_buffer: [Vec<CopyBufferEntry>; INTERNAL_CLIPBOARD_COUNT as usize],
	pub persistent_data: PersistentData,
}

impl MessageHandler<PortfolioMessage, (&InputPreprocessorMessageHandler, &PreferencesMessageHandler)> for PortfolioMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: PortfolioMessage, responses: &mut VecDeque<Message>, (ipp, preferences): (&InputPreprocessorMessageHandler, &PreferencesMessageHandler)) {
		#[remain::sorted]
		match message {
			// Sub-messages
			#[remain::unsorted]
			PortfolioMessage::MenuBar(message) => self.menu_bar_message_handler.process_message(message, responses, ()),
			#[remain::unsorted]
			PortfolioMessage::Document(message) => {
				if let Some(document_id) = self.active_document_id {
					if let Some(document) = self.documents.get_mut(&document_id) {
						document.process_message(message, responses, (document_id, ipp, &self.persistent_data, preferences, &mut self.executor))
					}
				}
			}

			// Messages
			#[remain::unsorted]
			PortfolioMessage::DocumentPassMessage { document_id, message } => {
				if let Some(document) = self.documents.get_mut(&document_id) {
					document.process_message(message, responses, (document_id, ipp, &self.persistent_data, preferences, &mut self.executor))
				}
			}
			PortfolioMessage::AutoSaveActiveDocument => {
				if let Some(document_id) = self.active_document_id {
					if let Some(document) = self.active_document_mut() {
						document.set_auto_save_state(true);
					}
					responses.add(PortfolioMessage::AutoSaveDocument { document_id });
				}
			}
			PortfolioMessage::AutoSaveDocument { document_id } => {
				let document = self.documents.get(&document_id).unwrap();
				responses.add(FrontendMessage::TriggerIndexedDbWriteDocument {
					document: document.serialize_document(),
					details: FrontendDocumentDetails {
						is_auto_saved: document.is_auto_saved(),
						is_saved: document.is_saved(),
						id: document_id,
						name: document.name.clone(),
					},
					version: GRAPHITE_DOCUMENT_VERSION.to_string(),
				})
			}
			PortfolioMessage::CloseActiveDocumentWithConfirmation => {
				if let Some(document_id) = self.active_document_id {
					responses.add(PortfolioMessage::CloseDocumentWithConfirmation { document_id });
				}
			}
			PortfolioMessage::CloseAllDocuments => {
				if self.active_document_id.is_some() {
					responses.add(PropertiesPanelMessage::Deactivate);
					responses.add(BroadcastEvent::ToolAbort);
					responses.add(ToolMessage::DeactivateTools);

					// Clear relevant UI layouts if there are no documents
					responses.add(PropertiesPanelMessage::ClearSelection);
					responses.add(DocumentMessage::ClearLayerTree);
					let hint_data = HintData(vec![HintGroup(vec![])]);
					responses.add(FrontendMessage::UpdateInputHints { hint_data });
				}

				for document_id in &self.document_ids {
					responses.add(FrontendMessage::TriggerIndexedDbRemoveDocument { document_id: *document_id });
				}

				responses.add(PortfolioMessage::DestroyAllDocuments);
				responses.add(PortfolioMessage::UpdateOpenDocumentsList);
			}
			PortfolioMessage::CloseDocument { document_id } => {
				// Is this the last document?
				if self.documents.len() == 1 && self.document_ids[0] == document_id {
					// Clear UI layouts that assume the existence of a document
					responses.add(PropertiesPanelMessage::ClearSelection);
					responses.add(DocumentMessage::ClearLayerTree);
					let hint_data = HintData(vec![HintGroup(vec![])]);
					responses.add(FrontendMessage::UpdateInputHints { hint_data });
				}
				// Actually delete the document (delay to delete document is required to let the document and properties panel messages above get processed)
				responses.add(PortfolioMessage::DeleteDocument { document_id });

				// Send the new list of document tab names
				responses.add(PortfolioMessage::UpdateOpenDocumentsList);
				responses.add(FrontendMessage::TriggerIndexedDbRemoveDocument { document_id });
				responses.add(DocumentMessage::RenderDocument);
				responses.add(DocumentMessage::DocumentStructureChanged);
				if let Some(document) = self.active_document() {
					for layer in document.layer_metadata.keys() {
						responses.add(DocumentMessage::LayerChanged { affected_layer_path: layer.clone() });
					}
				}
			}
			PortfolioMessage::CloseDocumentWithConfirmation { document_id } => {
				let target_document = self.documents.get(&document_id).unwrap();
				if target_document.is_saved() {
					responses.add(BroadcastEvent::ToolAbort);
					responses.add(PortfolioMessage::CloseDocument { document_id });
				} else {
					let dialog = simple_dialogs::CloseDocumentDialog {
						document_name: target_document.name.clone(),
						document_id,
					};
					dialog.register_properties(responses, LayoutTarget::DialogDetails);
					responses.add(FrontendMessage::DisplayDialog { icon: "File".to_string() });

					// Select the document being closed
					responses.add(PortfolioMessage::SelectDocument { document_id });
				}
			}
			PortfolioMessage::Copy { clipboard } => {
				// We can't use `self.active_document()` because it counts as an immutable borrow of the entirety of `self`
				if let Some(active_document) = self.active_document_id.and_then(|id| self.documents.get(&id)) {
					let copy_val = |buffer: &mut Vec<CopyBufferEntry>| {
						for layer_path in active_document.selected_layers_without_children() {
							match (active_document.document_legacy.layer(layer_path).map(|t| t.clone()), *active_document.layer_metadata(layer_path)) {
								(Ok(layer), layer_metadata) => {
									buffer.push(CopyBufferEntry { layer, layer_metadata });
								}
								(Err(e), _) => warn!("Could not access selected layer {:?}: {:?}", layer_path, e),
							}
						}
					};

					if clipboard == Clipboard::Device {
						let mut buffer = Vec::new();
						copy_val(&mut buffer);
						let mut copy_text = String::from("graphite/layer: ");
						copy_text += &serde_json::to_string(&buffer).expect("Could not serialize paste");

						responses.add(FrontendMessage::TriggerTextCopy { copy_text });
					} else {
						let copy_buffer = &mut self.copy_buffer;
						copy_buffer[clipboard as usize].clear();
						copy_val(&mut copy_buffer[clipboard as usize]);
					}
				}
			}
			PortfolioMessage::Cut { clipboard } => {
				responses.add(PortfolioMessage::Copy { clipboard });
				responses.add(DocumentMessage::DeleteSelectedLayers);
			}
			PortfolioMessage::DeleteDocument { document_id } => {
				let document_index = self.document_index(document_id);
				self.documents.remove(&document_id);
				self.document_ids.remove(document_index);

				if self.document_ids.is_empty() {
					self.active_document_id = None;
				} else if self.active_document_id.is_some() {
					let document_id = if document_index == self.document_ids.len() {
						// If we closed the last document take the one previous (same as last)
						*self.document_ids.last().unwrap()
					} else {
						// Move to the next tab
						self.document_ids[document_index]
					};
					responses.add(PortfolioMessage::SelectDocument { document_id });
				}
			}
			PortfolioMessage::DestroyAllDocuments => {
				// Empty the list of internal document data
				self.documents.clear();
				self.document_ids.clear();
				self.active_document_id = None;
			}
			PortfolioMessage::FontLoaded {
				font_family,
				font_style,
				preview_url,
				data,
				is_default,
			} => {
				let font = Font::new(font_family, font_style);

				if let Some(document) = self.active_document_mut() {
					Self::uploaded_new_font(document, &font, responses);
					responses.add(DocumentMessage::RenderDocument);
					responses.add(BroadcastEvent::DocumentIsDirty);
				}

				self.persistent_data.font_cache.insert(font, preview_url, data, is_default);
				self.executor.update_font_cache(self.persistent_data.font_cache.clone());
			}
			PortfolioMessage::ImaginateCheckServerStatus => {
				self.persistent_data.imaginate_server_status = ImaginateServerStatus::Checking;
				responses.add(FrontendMessage::TriggerImaginateCheckServerStatus {
					hostname: preferences.imaginate_server_hostname.clone(),
				});
				responses.add(PropertiesPanelMessage::ResendActiveProperties);
			}
			PortfolioMessage::ImaginateSetGeneratingStatus {
				document_id,
				layer_path,
				node_path,
				percent,
				status,
			} => {
				let get = |name: &str| IMAGINATE_NODE.inputs.iter().position(|input| input.name == name).unwrap_or_else(|| panic!("Input {name} not found"));
				if let Some(percentage) = percent {
					responses.add(PortfolioMessage::DocumentPassMessage {
						document_id,
						message: NodeGraphMessage::SetQualifiedInputValue {
							layer_path: layer_path.clone(),
							node_path: node_path.clone(),
							input_index: get("Percent Complete"),
							value: TaggedValue::F64(percentage),
						}
						.into(),
					});
				}

				responses.add(PortfolioMessage::DocumentPassMessage {
					document_id,
					message: NodeGraphMessage::SetQualifiedInputValue {
						layer_path,
						node_path,
						input_index: get("Status"),
						value: TaggedValue::ImaginateStatus(status),
					}
					.into(),
				});
			}
			PortfolioMessage::ImaginateSetImageData {
				document_id,
				layer_path,
				node_path,
				image_data,
				width,
				height,
			} => {
				let get = |name: &str| IMAGINATE_NODE.inputs.iter().position(|input| input.name == name).unwrap_or_else(|| panic!("Input {name} not found"));

				let image = Image::from_image_data(&image_data, width, height);
				responses.add(PortfolioMessage::DocumentPassMessage {
					document_id,
					message: NodeGraphMessage::SetQualifiedInputValue {
						layer_path,
						node_path,
						input_index: get("Cached Data"),
						value: TaggedValue::RcImage(Some(std::sync::Arc::new(image))),
					}
					.into(),
				});
			}
			PortfolioMessage::ImaginateSetServerStatus { status } => {
				self.persistent_data.imaginate_server_status = status;
				responses.add(PropertiesPanelMessage::ResendActiveProperties);
			}
			PortfolioMessage::Import => {
				// This portfolio message wraps the frontend message so it can be listed as an action, which isn't possible for frontend messages
				if self.active_document().is_some() {
					responses.add(FrontendMessage::TriggerImport);
				}
			}
			PortfolioMessage::LoadDocumentResources { document_id } => {
				if let Some(document) = self.document_mut(document_id) {
					document.load_layer_resources(responses, &document.document_legacy.root.data, Vec::new(), document_id);
				}
			}
			PortfolioMessage::LoadFont { font, is_default } => {
				if !self.persistent_data.font_cache.loaded_font(&font) {
					responses.add_front(FrontendMessage::TriggerFontLoad { font, is_default });
				}
			}
			PortfolioMessage::NewDocumentWithName { name } => {
				let new_document = DocumentMessageHandler::with_name(name, ipp);
				let document_id = generate_uuid();
				if self.active_document().is_some() {
					responses.add(BroadcastEvent::ToolAbort);
					responses.add(NavigationMessage::TranslateCanvas { delta: (0., 0.).into() });
				}

				self.load_document(new_document, document_id, responses);
			}
			PortfolioMessage::NextDocument => {
				if let Some(active_document_id) = self.active_document_id {
					let current_index = self.document_index(active_document_id);
					let next_index = (current_index + 1) % self.document_ids.len();
					let next_id = self.document_ids[next_index];

					responses.add(PortfolioMessage::SelectDocument { document_id: next_id });
				}
			}
			PortfolioMessage::OpenDocument => {
				// This portfolio message wraps the frontend message so it can be listed as an action, which isn't possible for frontend messages
				responses.add(FrontendMessage::TriggerOpenDocument);
			}
			PortfolioMessage::OpenDocumentFile {
				document_name,
				document_serialized_content,
			} => {
				responses.add(PortfolioMessage::OpenDocumentFileWithId {
					document_id: generate_uuid(),
					document_name,
					document_is_auto_saved: false,
					document_is_saved: true,
					document_serialized_content,
				});
			}
			PortfolioMessage::OpenDocumentFileWithId {
				document_id,
				document_name,
				document_is_auto_saved,
				document_is_saved,
				document_serialized_content,
			} => {
				let document = DocumentMessageHandler::with_name_and_content(document_name, document_serialized_content);
				match document {
					Ok(mut document) => {
						document.set_auto_save_state(document_is_auto_saved);
						document.set_save_state(document_is_saved);
						self.load_document(document, document_id, responses);
					}
					Err(e) => {
						if !document_is_auto_saved {
							responses.add(DialogMessage::DisplayDialogError {
								title: "Failed to open document".to_string(),
								description: e.to_string(),
							});
						}
					}
				}
			}
			// TODO: Paste message is unused, delete it?
			PortfolioMessage::Paste { clipboard } => {
				let shallowest_common_folder = self.active_document().map(|document| {
					document
						.document_legacy
						.shallowest_common_folder(document.selected_layers())
						.expect("While pasting, the selected layers did not exist while attempting to find the appropriate folder path for insertion")
				});

				if let Some(folder) = shallowest_common_folder {
					responses.add(DocumentMessage::DeselectAllLayers);
					responses.add(DocumentMessage::StartTransaction);
					responses.add(PortfolioMessage::PasteIntoFolder {
						clipboard,
						folder_path: folder.to_vec(),
						insert_index: -1,
					});
					responses.add(DocumentMessage::CommitTransaction);
				}
			}
			PortfolioMessage::PasteIntoFolder {
				clipboard,
				folder_path: path,
				insert_index,
			} => {
				let paste = |entry: &CopyBufferEntry, responses: &mut VecDeque<_>| {
					if let Some(document) = self.active_document() {
						trace!("Pasting into folder {:?} as index: {}", &path, insert_index);
						let destination_path = [path.to_vec(), vec![generate_uuid()]].concat();

						responses.add_front(DocumentMessage::UpdateLayerMetadata {
							layer_path: destination_path.clone(),
							layer_metadata: entry.layer_metadata,
						});
						document.load_layer_resources(responses, &entry.layer.data, destination_path.clone(), self.active_document_id.unwrap());
						responses.add_front(DocumentOperation::InsertLayer {
							layer: Box::new(entry.layer.clone()),
							destination_path,
							insert_index,
							duplicating: false,
						});
					}
				};

				if insert_index == -1 {
					for entry in self.copy_buffer[clipboard as usize].iter().rev() {
						paste(entry, responses)
					}
				} else {
					for entry in self.copy_buffer[clipboard as usize].iter() {
						paste(entry, responses)
					}
				}
			}
			PortfolioMessage::PasteSerializedData { data } => {
				if let Some(document) = self.active_document() {
					if let Ok(data) = serde_json::from_str::<Vec<CopyBufferEntry>>(&data) {
						let shallowest_common_folder = document
							.document_legacy
							.shallowest_common_folder(document.selected_layers())
							.expect("While pasting from serialized, the selected layers did not exist while attempting to find the appropriate folder path for insertion");
						responses.add(DocumentMessage::DeselectAllLayers);
						responses.add(DocumentMessage::StartTransaction);

						for entry in data.iter().rev() {
							let destination_path = [shallowest_common_folder.to_vec(), vec![generate_uuid()]].concat();

							responses.add_front(DocumentMessage::UpdateLayerMetadata {
								layer_path: destination_path.clone(),
								layer_metadata: entry.layer_metadata,
							});
							document.load_layer_resources(responses, &entry.layer.data, destination_path.clone(), self.active_document_id.unwrap());
							responses.add_front(DocumentOperation::InsertLayer {
								layer: Box::new(entry.layer.clone()),
								destination_path,
								insert_index: -1,
								duplicating: false,
							});
						}

						responses.add(DocumentMessage::CommitTransaction);
					}
				}
			}
			PortfolioMessage::PrevDocument => {
				if let Some(active_document_id) = self.active_document_id {
					let len = self.document_ids.len();
					let current_index = self.document_index(active_document_id);
					let prev_index = (current_index + len - 1) % len;
					let prev_id = self.document_ids[prev_index];
					responses.add(PortfolioMessage::SelectDocument { document_id: prev_id });
				}
			}
			PortfolioMessage::RenderGraphUsingRasterizedRegionBelowLayer {
				document_id,
				layer_path,
				input_image_data,
				size,
				imaginate_node_path,
			} => {
				let result = self.executor.submit_node_graph_evaluation(
					(document_id, &mut self.documents),
					layer_path,
					(input_image_data, size),
					imaginate_node_path,
					(preferences, &self.persistent_data),
					responses,
				);

				if let Err(description) = result {
					responses.add(DialogMessage::DisplayDialogError {
						title: "Unable to update node graph".to_string(),
						description,
					});
				}
			}
			PortfolioMessage::SelectDocument { document_id } => {
				if let Some(document) = self.active_document() {
					if !document.is_auto_saved() {
						responses.add(PortfolioMessage::AutoSaveDocument {
							// Safe to unwrap since we know that there is an active document
							document_id: self.active_document_id.unwrap(),
						});
					}
				}

				if self.active_document().is_some() {
					responses.add(BroadcastEvent::ToolAbort);
					responses.add(OverlaysMessage::ClearAllOverlays);
				}

				// TODO: Remove this message in favor of having tools have specific data per document instance
				responses.add(PortfolioMessage::SetActiveDocument { document_id });
				responses.add(PortfolioMessage::UpdateOpenDocumentsList);
				responses.add(FrontendMessage::UpdateActiveDocument { document_id });
				responses.add(DocumentMessage::RenderDocument);
				responses.add(DocumentMessage::DocumentStructureChanged);
				for layer in self.documents.get(&document_id).unwrap().layer_metadata.keys() {
					responses.add(DocumentMessage::LayerChanged { affected_layer_path: layer.clone() });
				}
				responses.add(BroadcastEvent::SelectionChanged);
				responses.add(BroadcastEvent::DocumentIsDirty);
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
				responses.add(NavigationMessage::TranslateCanvas { delta: (0., 0.).into() });
			}
			PortfolioMessage::SetActiveDocument { document_id } => self.active_document_id = Some(document_id),
			PortfolioMessage::SetImageBlobUrl {
				document_id,
				layer_path,
				node_id,
				blob_url,
				resolution,
			} => {
				if let (Some(layer_id), Some(node_id)) = (layer_path.last().copied(), node_id) {
					self.executor.insert_thumbnail_blob_url(blob_url, layer_id, node_id, responses);
					return;
				}
				let message = DocumentMessage::SetImageBlobUrl {
					layer_path,
					blob_url,
					resolution,
					document_id,
				};
				responses.add(PortfolioMessage::DocumentPassMessage { document_id, message });
			}
			PortfolioMessage::UpdateDocumentWidgets => {
				if let Some(document) = self.active_document() {
					document.update_document_widgets(responses);
				}
			}
			PortfolioMessage::UpdateOpenDocumentsList => {
				// Send the list of document tab names
				let open_documents = self
					.document_ids
					.iter()
					.filter_map(|id| {
						self.documents.get(id).map(|document| FrontendDocumentDetails {
							is_auto_saved: document.is_auto_saved(),
							is_saved: document.is_saved(),
							id: *id,
							name: document.name.clone(),
						})
					})
					.collect::<Vec<_>>();
				responses.add(FrontendMessage::UpdateOpenDocumentsList { open_documents });
			}
		}
	}

	fn actions(&self) -> ActionList {
		let mut common = actions!(PortfolioMessageDiscriminant;
			CloseActiveDocumentWithConfirmation,
			CloseAllDocuments,
			Import,
			NextDocument,
			OpenDocument,
			Paste,
			PasteIntoFolder,
			PrevDocument,
		);

		if let Some(document) = self.active_document() {
			if document.layer_metadata.values().any(|data| data.selected) {
				let select = actions!(PortfolioMessageDiscriminant;
					Copy,
					Cut,
				);
				common.extend(select);
			}
			common.extend(document.actions());
		}

		common
	}
}

impl PortfolioMessageHandler {
	pub fn introspect_node(&self, node_path: &[NodeId]) -> Option<Arc<dyn std::any::Any>> {
		self.executor.introspect_node(node_path)
	}

	pub fn document(&self, document_id: u64) -> Option<&DocumentMessageHandler> {
		self.documents.get(&document_id)
	}

	pub fn document_mut(&mut self, document_id: u64) -> Option<&mut DocumentMessageHandler> {
		self.documents.get_mut(&document_id)
	}

	pub fn active_document(&self) -> Option<&DocumentMessageHandler> {
		self.active_document_id.and_then(|id| self.documents.get(&id))
	}

	pub fn active_document_mut(&mut self) -> Option<&mut DocumentMessageHandler> {
		self.active_document_id.and_then(|id| self.documents.get_mut(&id))
	}

	pub fn active_document_id(&self) -> Option<u64> {
		self.active_document_id
	}

	pub fn generate_new_document_name(&self) -> String {
		let mut doc_title_numbers = self
			.ordered_document_iterator()
			.filter_map(|doc| {
				doc.name
					.rsplit_once(DEFAULT_DOCUMENT_NAME)
					.map(|(prefix, number)| (prefix.is_empty()).then(|| number.trim().parse::<isize>().ok()).flatten().unwrap_or(1))
			})
			.collect::<Vec<isize>>();

		doc_title_numbers.sort_unstable();
		doc_title_numbers.iter_mut().enumerate().for_each(|(i, number)| *number = *number - i as isize - 2);
		// Uses binary search to find the index of the element where number is bigger than i
		let new_doc_title_num = doc_title_numbers.binary_search(&0).map_or_else(|e| e, |v| v) + 1;

		match new_doc_title_num {
			1 => DEFAULT_DOCUMENT_NAME.to_string(),
			_ => format!("{} {}", DEFAULT_DOCUMENT_NAME, new_doc_title_num),
		}
	}

	// TODO: Fix how this doesn't preserve tab order upon loading new document from *File > Load*
	fn load_document(&mut self, new_document: DocumentMessageHandler, document_id: u64, responses: &mut VecDeque<Message>) {
		let render_data = RenderData::new(&self.persistent_data.font_cache, new_document.view_mode, None);

		self.document_ids.push(document_id);

		responses.extend(
			new_document
				.layer_metadata
				.keys()
				.filter_map(|path| new_document.layer_panel_entry_from_path(path, &render_data))
				.map(|entry| FrontendMessage::UpdateDocumentLayerDetails { data: entry }.into())
				.collect::<Vec<_>>(),
		);
		new_document.update_layer_tree_options_bar_widgets(responses, &render_data);

		self.documents.insert(document_id, new_document);

		if self.active_document().is_some() {
			responses.add(PropertiesPanelMessage::Deactivate);
			responses.add(BroadcastEvent::ToolAbort);
			responses.add(ToolMessage::DeactivateTools);
		}

		responses.add(PortfolioMessage::UpdateOpenDocumentsList);
		responses.add(PortfolioMessage::SelectDocument { document_id });
		responses.add(PortfolioMessage::LoadDocumentResources { document_id });
		responses.add(PortfolioMessage::UpdateDocumentWidgets);
		responses.add(ToolMessage::InitTools);
		responses.add(PropertiesPanelMessage::Init);
		responses.add(NavigationMessage::TranslateCanvas { delta: (0., 0.).into() });
		responses.add(DocumentMessage::DocumentStructureChanged);
		responses.add(PropertiesPanelMessage::ClearSelection);
		responses.add(PropertiesPanelMessage::UpdateSelectedDocumentProperties);
	}

	/// Returns an iterator over the open documents in order.
	pub fn ordered_document_iterator(&self) -> impl Iterator<Item = &DocumentMessageHandler> {
		self.document_ids.iter().map(|id| self.documents.get(id).expect("document id was not found in the document hashmap"))
	}

	fn document_index(&self, document_id: u64) -> usize {
		self.document_ids.iter().position(|id| id == &document_id).expect("Active document is missing from document ids")
	}

	fn uploaded_new_font(document: &mut DocumentMessageHandler, target_font: &Font, responses: &mut VecDeque<Message>) {
		let mut stack = vec![(&document.document_legacy.root, Vec::new())];

		while let Some((layer, layer_path)) = stack.pop() {
			match &layer.data {
				LayerDataType::Folder(folder) => stack.extend(folder.layers.iter().zip(folder.layer_ids.iter().map(|id| {
					let mut x = layer_path.clone();
					x.push(*id);
					x
				}))),
				LayerDataType::Layer(layer) => {
					let input_is_font = |input: &NodeInput| {
						let NodeInput::Value { tagged_value: TaggedValue::Font(font), .. } = input else {
							return false;
						};
						font == target_font
					};
					let should_rerender = layer.network.nodes.values().any(|node| node.inputs.iter().any(input_is_font));
					if should_rerender {
						responses.add(DocumentMessage::InputFrameRasterizeRegionBelowLayer { layer_path });
					}
				}
				_ => {}
			}
		}
	}

	pub fn poll_node_graph_evaluation(&mut self, responses: &mut VecDeque<Message>) {
		self.executor.poll_node_graph_evaluation(responses).unwrap_or_else(|e| {
			log::error!("Error while evaluating node graph: {}", e);
		});
	}
}

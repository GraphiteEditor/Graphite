use super::utility_types::PersistentData;
use crate::application::generate_uuid;
use crate::consts::DEFAULT_DOCUMENT_NAME;
use crate::messages::dialog::simple_dialogs;
use crate::messages::frontend::utility_types::FrontendDocumentDetails;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::utility_types::clipboards::{Clipboard, CopyBufferEntry, INTERNAL_CLIPBOARD_COUNT};
use crate::messages::portfolio::document::DocumentInputs;
use crate::messages::prelude::*;
use crate::messages::tool::utility_types::{HintData, HintGroup};
use crate::node_graph_executor::{ExportConfig, NodeGraphExecutor};

use graph_craft::document::NodeId;
use graphene_core::text::Font;

use std::sync::Arc;

#[derive(Debug, Default)]
pub struct PortfolioMessageHandler {
	menu_bar_message_handler: MenuBarMessageHandler,
	documents: HashMap<DocumentId, DocumentMessageHandler>,
	document_ids: Vec<DocumentId>,
	active_document_id: Option<DocumentId>,
	copy_buffer: [Vec<CopyBufferEntry>; INTERNAL_CLIPBOARD_COUNT as usize],
	pub persistent_data: PersistentData,
	pub executor: NodeGraphExecutor,
}

impl MessageHandler<PortfolioMessage, (&InputPreprocessorMessageHandler, &PreferencesMessageHandler)> for PortfolioMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: PortfolioMessage, responses: &mut VecDeque<Message>, (ipp, preferences): (&InputPreprocessorMessageHandler, &PreferencesMessageHandler)) {
		#[remain::sorted]
		match message {
			// Sub-messages
			#[remain::unsorted]
			PortfolioMessage::MenuBar(message) => {
				let mut has_active_document = false;
				let mut rulers_visible = false;

				if let Some(document) = self.active_document_id.and_then(|document_id| self.documents.get_mut(&document_id)) {
					has_active_document = true;
					rulers_visible = document.rulers_visible;
				}

				self.menu_bar_message_handler.process_message(message, responses, (has_active_document, rulers_visible));
			}
			#[remain::unsorted]
			PortfolioMessage::Document(message) => {
				if let Some(document_id) = self.active_document_id {
					if let Some(document) = self.documents.get_mut(&document_id) {
						let document_inputs = DocumentInputs {
							document_id,
							ipp,
							persistent_data: &self.persistent_data,
							executor: &mut self.executor,
						};
						document.process_message(message, responses, document_inputs)
					}
				}
			}

			// Messages
			#[remain::unsorted]
			PortfolioMessage::DocumentPassMessage { document_id, message } => {
				if let Some(document) = self.documents.get_mut(&document_id) {
					let document_inputs = DocumentInputs {
						document_id,
						ipp,
						persistent_data: &self.persistent_data,
						executor: &mut self.executor,
					};
					document.process_message(message, responses, document_inputs)
				}
			}
			PortfolioMessage::AutoSaveActiveDocument => {
				if let Some(document_id) = self.active_document_id {
					if let Some(document) = self.active_document_mut() {
						document.set_auto_save_state(true);
						responses.add(PortfolioMessage::AutoSaveDocument { document_id });
					}
				}
			}
			PortfolioMessage::AutoSaveAllDocuments => {
				for (document_id, document) in self.documents.iter_mut() {
					if !document.is_auto_saved() {
						document.set_auto_save_state(true);
						responses.add(PortfolioMessage::AutoSaveDocument { document_id: *document_id });
					}
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
				})
			}
			PortfolioMessage::CloseActiveDocumentWithConfirmation => {
				if let Some(document_id) = self.active_document_id {
					responses.add(PortfolioMessage::CloseDocumentWithConfirmation { document_id });
				}
			}
			PortfolioMessage::CloseAllDocuments => {
				if self.active_document_id.is_some() {
					responses.add(BroadcastEvent::ToolAbort);
					responses.add(ToolMessage::DeactivateTools);

					// Clear relevant UI layouts if there are no documents
					responses.add(PropertiesPanelMessage::Clear);
					responses.add(DocumentMessage::ClearLayersPanel);
					let hint_data = HintData(vec![HintGroup(vec![])]);
					responses.add(FrontendMessage::UpdateInputHints { hint_data });
				}

				for document_id in &self.document_ids {
					responses.add(FrontendMessage::TriggerIndexedDbRemoveDocument { document_id: *document_id });
				}

				responses.add(PortfolioMessage::DestroyAllDocuments);
				responses.add(PortfolioMessage::UpdateOpenDocumentsList);
			}
			PortfolioMessage::CloseAllDocumentsWithConfirmation => {
				if self.unsaved_document_names().is_empty() {
					responses.add(PortfolioMessage::CloseAllDocuments)
				} else {
					responses.add(DialogMessage::CloseAllDocumentsWithConfirmation)
				}
			}
			PortfolioMessage::CloseDocument { document_id } => {
				// Is this the last document?
				if self.documents.len() == 1 && self.document_ids[0] == document_id {
					// Clear UI layouts that assume the existence of a document
					responses.add(PropertiesPanelMessage::Clear);
					responses.add(DocumentMessage::ClearLayersPanel);
					let hint_data = HintData(vec![HintGroup(vec![])]);
					responses.add(FrontendMessage::UpdateInputHints { hint_data });
				}

				// Actually delete the document (delay to delete document is required to let the document and properties panel messages above get processed)
				responses.add(PortfolioMessage::DeleteDocument { document_id });
				responses.add(FrontendMessage::TriggerIndexedDbRemoveDocument { document_id });

				// Send the new list of document tab names
				responses.add(PortfolioMessage::UpdateOpenDocumentsList);
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
					dialog.send_dialog_to_frontend(responses);

					// Select the document being closed
					responses.add(PortfolioMessage::SelectDocument { document_id });
				}
			}
			PortfolioMessage::Copy { clipboard } => {
				// We can't use `self.active_document()` because it counts as an immutable borrow of the entirety of `self`
				let Some(active_document) = self.active_document_id.and_then(|id| self.documents.get(&id)) else {
					return;
				};

				let copy_val = |buffer: &mut Vec<CopyBufferEntry>| {
					for layer_path in active_document
						.metadata()
						.shallowest_unique_layers(active_document.selected_nodes.selected_layers(active_document.metadata()))
					{
						let Some(layer) = layer_path.last().copied() else {
							continue;
						};

						let node = layer.to_node();
						let Some(node) = active_document.network().nodes.get(&node).and_then(|node| node.inputs.first()).and_then(|input| input.as_node()) else {
							continue;
						};

						buffer.push(CopyBufferEntry {
							nodes: NodeGraphMessageHandler::copy_nodes(
								active_document.network(),
								&active_document
									.network()
									.upstream_flow_back_from_nodes(vec![node], false)
									.enumerate()
									.map(|(index, (_, node_id))| (node_id, NodeId(index as u64)))
									.collect(),
							)
							.collect(),
							selected: active_document.selected_nodes.selected_layers_contains(layer, active_document.metadata()),
							collapsed: false,
						});
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
					responses.add(MenuBarMessage::SendLayout);
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
				responses.add(MenuBarMessage::SendLayout);
			}
			PortfolioMessage::FontLoaded {
				font_family,
				font_style,
				preview_url,
				data,
				is_default,
			} => {
				let font = Font::new(font_family, font_style);

				self.persistent_data.font_cache.insert(font, preview_url, data, is_default);
				self.executor.update_font_cache(self.persistent_data.font_cache.clone());

				if self.active_document_mut().is_some() {
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}
			}
			PortfolioMessage::ImaginateCheckServerStatus => {
				let server_status = self.persistent_data.imaginate.server_status().clone();
				self.persistent_data.imaginate.poll_server_check();
				#[cfg(target_arch = "wasm32")]
				if let Some(fut) = self.persistent_data.imaginate.initiate_server_check() {
					wasm_bindgen_futures::spawn_local(async move {
						let () = fut.await;
						use wasm_bindgen::prelude::*;

						#[wasm_bindgen(module = "/../frontend/src/wasm-communication/editor.ts")]
						extern "C" {
							#[wasm_bindgen(js_name = injectImaginatePollServerStatus)]
							fn inject();
						}
						inject();
					})
				}
				if &server_status != self.persistent_data.imaginate.server_status() {
					responses.add(PropertiesPanelMessage::Refresh);
				}
			}
			PortfolioMessage::ImaginatePollServerStatus => {
				self.persistent_data.imaginate.poll_server_check();
				responses.add(PropertiesPanelMessage::Refresh);
			}
			PortfolioMessage::ImaginatePreferences => self.executor.update_imaginate_preferences(preferences.get_imaginate_preferences()),
			PortfolioMessage::ImaginateServerHostname => {
				self.persistent_data.imaginate.set_host_name(&preferences.imaginate_server_hostname);
			}
			PortfolioMessage::Import => {
				// This portfolio message wraps the frontend message so it can be listed as an action, which isn't possible for frontend messages
				if self.active_document().is_some() {
					responses.add(FrontendMessage::TriggerImport);
				}
			}
			PortfolioMessage::LoadDocumentResources { document_id } => {
				if let Some(document) = self.document_mut(document_id) {
					document.load_layer_resources(responses);
				}
			}
			PortfolioMessage::LoadFont { font, is_default } => {
				if !self.persistent_data.font_cache.loaded_font(&font) {
					responses.add_front(FrontendMessage::TriggerFontLoad { font, is_default });
				}
			}
			PortfolioMessage::NewDocumentWithName { name } => {
				let new_document = DocumentMessageHandler::with_name(name, ipp, responses);
				let document_id = DocumentId(generate_uuid());
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
					document_id: DocumentId(generate_uuid()),
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
						println!("Failed to open document: {e}");
						if !document_is_auto_saved {
							responses.add(DialogMessage::DisplayDialogError {
								title: "Failed to open document".to_string(),
								description: e.to_string(),
							});
						}
					}
				}
			}
			PortfolioMessage::PasteIntoFolder { clipboard, parent, insert_index } => {
				let paste = |entry: &CopyBufferEntry, responses: &mut VecDeque<_>| {
					if self.active_document().is_some() {
						trace!("Pasting into folder {parent:?} as index: {insert_index}");
						let id = NodeId(generate_uuid());
						responses.add(GraphOperationMessage::NewCustomLayer {
							id,
							nodes: entry.nodes.clone(),
							parent,
							insert_index,
						});
						if entry.selected {
							responses.add(NodeGraphMessage::SelectedNodesAdd { nodes: vec![id] });
						}
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
						let parent = document.new_layer_parent();

						responses.add(DocumentMessage::DeselectAllLayers);
						responses.add(DocumentMessage::StartTransaction);

						for entry in data.into_iter().rev() {
							document.load_layer_resources(responses);
							let id = NodeId(generate_uuid());
							responses.add(GraphOperationMessage::NewCustomLayer {
								id,
								nodes: entry.nodes,
								parent,
								insert_index: -1,
							});
							if entry.selected {
								responses.add(NodeGraphMessage::SelectedNodesAdd { nodes: vec![id] });
							}
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
			PortfolioMessage::SelectDocument { document_id } => {
				// Auto-save the document we are leaving
				if let Some(document) = self.active_document() {
					if !document.is_auto_saved() {
						responses.add(PortfolioMessage::AutoSaveDocument {
							// Safe to unwrap since we know that there is an active document
							document_id: self.active_document_id.unwrap(),
						});
					}
				}

				// Set the new active document ID
				self.active_document_id = Some(document_id);

				responses.add(MenuBarMessage::SendLayout);
				responses.add(PortfolioMessage::UpdateOpenDocumentsList);
				responses.add(FrontendMessage::UpdateActiveDocument { document_id });
				responses.add(OverlaysMessage::Draw);
				responses.add(BroadcastEvent::ToolAbort);
				responses.add(BroadcastEvent::SelectionChanged);
				responses.add(PortfolioMessage::UpdateDocumentWidgets);
				responses.add(NavigationMessage::TranslateCanvas { delta: (0., 0.).into() });
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			PortfolioMessage::SubmitDocumentExport {
				file_name,
				file_type,
				scale_factor,
				bounds,
				transparent_background,
			} => {
				let document = self.active_document_id.and_then(|id| self.documents.get_mut(&id)).expect("Tried to render no existent Document");
				let export_config = ExportConfig {
					file_name,
					file_type,
					scale_factor,
					bounds,
					transparent_background,
					..Default::default()
				};
				let result = self.executor.submit_document_export(document, export_config);

				if let Err(description) = result {
					responses.add(DialogMessage::DisplayDialogError {
						title: "Unable to export document".to_string(),
						description,
					});
				}
			}
			PortfolioMessage::SubmitGraphRender { document_id } => {
				let result = self.executor.submit_node_graph_evaluation(
					self.documents.get_mut(&document_id).expect("Tried to render no existent Document"),
					ipp.viewport_bounds.size().as_uvec2(),
				);

				if let Err(description) = result {
					responses.add(DialogMessage::DisplayDialogError {
						title: "Unable to update node graph".to_string(),
						description,
					});
				}
			}
			PortfolioMessage::ToggleRulers => {
				if let Some(document) = self.active_document_mut() {
					document.rulers_visible = !document.rulers_visible;

					responses.add(DocumentMessage::RenderRulers);
					responses.add(MenuBarMessage::SendLayout);
					responses.add(FrontendMessage::TriggerRefreshBoundsOfViewports);
				}
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
			CloseAllDocumentsWithConfirmation,
			Import,
			NextDocument,
			OpenDocument,
			PasteIntoFolder,
			PrevDocument,
			ToggleRulers,
		);

		if let Some(document) = self.active_document() {
			if document.selected_nodes.selected_layers(document.metadata()).next().is_some() {
				let select = actions!(PortfolioMessageDiscriminant;
					Copy,
					Cut,
				);
				common.extend(select);
			}

			common.extend(document.actions_with_graph_open());
		}

		common
	}
}

impl PortfolioMessageHandler {
	pub fn introspect_node(&self, node_path: &[NodeId]) -> Option<Arc<dyn std::any::Any>> {
		self.executor.introspect_node(node_path)
	}

	pub fn document(&self, document_id: DocumentId) -> Option<&DocumentMessageHandler> {
		self.documents.get(&document_id)
	}

	pub fn document_mut(&mut self, document_id: DocumentId) -> Option<&mut DocumentMessageHandler> {
		self.documents.get_mut(&document_id)
	}

	pub fn active_document(&self) -> Option<&DocumentMessageHandler> {
		self.active_document_id.and_then(|id| self.documents.get(&id))
	}

	pub fn active_document_mut(&mut self) -> Option<&mut DocumentMessageHandler> {
		self.active_document_id.and_then(|id| self.documents.get_mut(&id))
	}

	pub fn active_document_id(&self) -> Option<DocumentId> {
		self.active_document_id
	}

	pub fn unsaved_document_names(&self) -> Vec<String> {
		self.documents.values().filter(|document| !document.is_saved()).map(|document| document.name.clone()).collect()
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
			_ => format!("{DEFAULT_DOCUMENT_NAME} {new_doc_title_num}"),
		}
	}

	// TODO: Fix how this doesn't preserve tab order upon loading new document from *File > Load*
	fn load_document(&mut self, new_document: DocumentMessageHandler, document_id: DocumentId, responses: &mut VecDeque<Message>) {
		self.document_ids.push(document_id);

		new_document.update_layers_panel_options_bar_widgets(responses);

		self.documents.insert(document_id, new_document);

		if self.active_document().is_some() {
			responses.add(BroadcastEvent::ToolAbort);
			responses.add(ToolMessage::DeactivateTools);
		}

		responses.add(PortfolioMessage::UpdateOpenDocumentsList);
		responses.add(PortfolioMessage::SelectDocument { document_id });
		responses.add(PortfolioMessage::LoadDocumentResources { document_id });
		responses.add(PortfolioMessage::UpdateDocumentWidgets);
		responses.add(ToolMessage::InitTools);
		responses.add(NodeGraphMessage::Init);
		responses.add(NavigationMessage::TranslateCanvas { delta: (0., 0.).into() });
		responses.add(PropertiesPanelMessage::Clear);
		responses.add(NodeGraphMessage::UpdateNewNodeGraph);
	}

	/// Returns an iterator over the open documents in order.
	pub fn ordered_document_iterator(&self) -> impl Iterator<Item = &DocumentMessageHandler> {
		self.document_ids.iter().map(|id| self.documents.get(id).expect("document id was not found in the document hashmap"))
	}

	fn document_index(&self, document_id: DocumentId) -> usize {
		self.document_ids.iter().position(|id| id == &document_id).expect("Active document is missing from document ids")
	}

	pub fn poll_node_graph_evaluation(&mut self, responses: &mut VecDeque<Message>) {
		let Some(active_document) = self.active_document_id.and_then(|id| self.documents.get_mut(&id)) else {
			return;
		};

		self.executor.poll_node_graph_evaluation(active_document, responses).unwrap_or_else(|e| {
			log::error!("Error while evaluating node graph: {e}");

			let error = r#"
				<rect x="50%" y="50%" width="480" height="100" transform="translate(-240 -50)" rx="4" fill="var(--color-error-red)" />
				<text x="50%" y="50%" dominant-baseline="middle" text-anchor="middle" font-size="18" fill="var(--color-2-mildblack)">
					<tspan x="50%" dy="-24" font-weight="bold">The document cannot be rendered in its current state.</tspan>
					<tspan x="50%" dy="24">Check for error details in the node graph, which can be</tspan>
					<tspan x="50%" dy="24">opened with the viewport's top right <tspan font-style="italic">Node Graph</tspan> button.</tspan>
				/text>"#
				// It's a mystery why the `/text>` tag above needs to be missing its `<`, but when it exists it prints the `<` character in the text. However this works with it removed.
				.to_string();
			responses.add(FrontendMessage::UpdateDocumentArtwork { svg: error });
		});
	}
}

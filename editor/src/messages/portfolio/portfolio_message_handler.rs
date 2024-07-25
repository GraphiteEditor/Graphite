use super::utility_types::PersistentData;
use crate::application::generate_uuid;
use crate::consts::DEFAULT_DOCUMENT_NAME;
use crate::messages::dialog::simple_dialogs;
use crate::messages::frontend::utility_types::FrontendDocumentDetails;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::graph_operation::utility_types::ModifyInputsContext;
use crate::messages::portfolio::document::utility_types::clipboards::{Clipboard, CopyBufferEntry, INTERNAL_CLIPBOARD_COUNT};
use crate::messages::portfolio::document::utility_types::nodes::SelectedNodes;
use crate::messages::portfolio::document::DocumentMessageData;
use crate::messages::prelude::*;
use crate::messages::tool::utility_types::{HintData, HintGroup};
use crate::node_graph_executor::{ExportConfig, NodeGraphExecutor};

use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeId, NodeInput};
use graphene_core::text::Font;
use graphene_std::vector::style::{Fill, FillType, Gradient};

use std::sync::Arc;

pub struct PortfolioMessageData<'a> {
	pub ipp: &'a InputPreprocessorMessageHandler,
	pub preferences: &'a PreferencesMessageHandler,
}

#[derive(Debug, Default)]
pub struct PortfolioMessageHandler {
	menu_bar_message_handler: MenuBarMessageHandler,
	pub documents: HashMap<DocumentId, DocumentMessageHandler>,
	document_ids: Vec<DocumentId>,
	pub(crate) active_document_id: Option<DocumentId>,
	copy_buffer: [Vec<CopyBufferEntry>; INTERNAL_CLIPBOARD_COUNT as usize],
	pub persistent_data: PersistentData,
	pub executor: NodeGraphExecutor,
}

impl MessageHandler<PortfolioMessage, PortfolioMessageData<'_>> for PortfolioMessageHandler {
	fn process_message(&mut self, message: PortfolioMessage, responses: &mut VecDeque<Message>, data: PortfolioMessageData) {
		let PortfolioMessageData { ipp, preferences } = data;

		match message {
			// Sub-messages
			PortfolioMessage::MenuBar(message) => {
				let mut has_active_document = false;
				let mut rulers_visible = false;
				let mut node_graph_open = false;

				if let Some(document) = self.active_document_id.and_then(|document_id| self.documents.get_mut(&document_id)) {
					has_active_document = true;
					rulers_visible = document.rulers_visible;
					node_graph_open = document.is_graph_overlay_open();
				}
				self.menu_bar_message_handler.process_message(
					message,
					responses,
					MenuBarMessageData {
						has_active_document,
						rulers_visible,
						node_graph_open,
					},
				);
			}
			PortfolioMessage::Document(message) => {
				if let Some(document_id) = self.active_document_id {
					if let Some(document) = self.documents.get_mut(&document_id) {
						let document_inputs = DocumentMessageData {
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
			PortfolioMessage::DocumentPassMessage { document_id, message } => {
				if let Some(document) = self.documents.get_mut(&document_id) {
					let document_inputs = DocumentMessageData {
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
					let binding = active_document
						.metadata()
						.shallowest_unique_layers(active_document.selected_nodes.selected_layers(active_document.metadata()));

					let get_last_elements: Vec<_> = binding.iter().map(|x| x.last().expect("empty path")).collect();

					let ordered_last_elements: Vec<_> = active_document.metadata.all_layers().filter(|layer| get_last_elements.contains(&layer)).collect();

					for layer in ordered_last_elements {
						let layer_node_id = layer.to_node();
						let previous_alias = active_document.network().nodes.get(&layer_node_id).map(|node| node.alias.clone()).unwrap_or_default();

						let mut copy_ids = HashMap::new();
						copy_ids.insert(layer_node_id, NodeId(0_u64));
						if let Some(input_node) = active_document
							.network()
							.nodes
							.get(&layer_node_id)
							.and_then(|node| if node.is_layer { node.inputs.get(1) } else { node.inputs.first() })
							.and_then(|input| input.as_node())
						{
							active_document
								.network()
								.upstream_flow_back_from_nodes(vec![input_node], graph_craft::document::FlowType::UpstreamFlow)
								.enumerate()
								.for_each(|(index, (_, node_id))| {
									copy_ids.insert(node_id, NodeId((index + 1) as u64));
								});
						};

						buffer.push(CopyBufferEntry {
							nodes: NodeGraphMessageHandler::copy_nodes(
								active_document.network(),
								&active_document.node_graph_handler.network,
								&active_document.node_graph_handler.resolved_types,
								&copy_ids,
							)
							.collect(),
							selected: active_document.selected_nodes.selected_layers_contains(layer, active_document.metadata()),
							visible: active_document.selected_nodes.layer_visible(layer, active_document.metadata()),
							locked: active_document.selected_nodes.layer_locked(layer, active_document.metadata()),
							collapsed: false,
							alias: previous_alias.to_string(),
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
				for document_id in self.document_ids.iter() {
					let _ = self.executor.submit_node_graph_evaluation(
						self.documents.get_mut(document_id).expect("Tried to render non-existent document"),
						ipp.viewport_bounds.size().as_uvec2(),
						true,
					);
				}

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
			PortfolioMessage::EditorPreferences => self.executor.update_editor_preferences(preferences.editor_preferences()),
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
					responses.add(NavigationMessage::CanvasPan { delta: (0., 0.).into() });
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
				// It can be helpful to temporarily set `upgrade_from_before_editable_subgraphs` to true if it's desired to upgrade a piece of artwork to use fresh copies of all nodes
				let upgrade_from_before_editable_subgraphs = document_serialized_content.contains("node_output_index");
				let upgrade_vector_manipulation_format = document_serialized_content.contains("ManipulatorGroupIds") && !document_name.contains("__DO_NOT_UPGRADE__");
				let document_name = document_name.replace("__DO_NOT_UPGRADE__", "");

				let document = DocumentMessageHandler::with_name_and_content(document_name.clone(), document_serialized_content);
				let mut document = match document {
					Ok(document) => document,
					Err(e) => {
						if !document_is_auto_saved {
							responses.add(DialogMessage::DisplayDialogError {
								title: "Failed to open document".to_string(),
								description: e.to_string(),
							});
						}

						return;
					}
				};

				// TODO: Eventually remove this (probably starting late 2024)
				// Upgrade all old nodes to support editable subgraphs introduced in #1750
				if upgrade_from_before_editable_subgraphs {
					for node in document.network.nodes.values_mut() {
						let node_definition = crate::messages::portfolio::document::node_graph::document_node_types::resolve_document_node_type(&node.name).unwrap();
						let default_definition_node = node_definition.default_document_node();

						node.implementation = default_definition_node.implementation.clone();
					}
				}
				if document.network.nodes.iter().any(|(node_id, node)| node.name == "Output" && *node_id == NodeId(0)) {
					ModifyInputsContext::delete_nodes(
						&mut document.node_graph_handler,
						&mut document.network,
						&mut SelectedNodes(vec![]),
						vec![NodeId(0)],
						true,
						responses,
						Vec::new(),
					);
				}

				// TODO: Eventually remove this (probably starting late 2024)
				// Upgrade Fill nodes to the format change in #1778
				for node in document.network.nodes.values_mut() {
					if node.name == "Fill" && node.inputs.len() == 8 {
						let node_definition = crate::messages::portfolio::document::node_graph::document_node_types::resolve_document_node_type(&node.name).unwrap();
						let default_definition_node = node_definition.default_document_node();

						node.implementation = default_definition_node.implementation.clone();
						let old_inputs = std::mem::replace(&mut node.inputs, default_definition_node.inputs.clone());

						node.inputs[0] = old_inputs[0].clone();

						let Some(fill_type) = old_inputs[1].as_value().cloned() else { continue };
						let TaggedValue::FillType(fill_type) = fill_type else { continue };
						let Some(solid_color) = old_inputs[2].as_value().cloned() else { continue };
						let TaggedValue::OptionalColor(solid_color) = solid_color else { continue };
						let Some(gradient_type) = old_inputs[3].as_value().cloned() else { continue };
						let TaggedValue::GradientType(gradient_type) = gradient_type else { continue };
						let Some(start) = old_inputs[4].as_value().cloned() else { continue };
						let TaggedValue::DVec2(start) = start else { continue };
						let Some(end) = old_inputs[5].as_value().cloned() else { continue };
						let TaggedValue::DVec2(end) = end else { continue };
						let Some(transform) = old_inputs[6].as_value().cloned() else { continue };
						let TaggedValue::DAffine2(transform) = transform else { continue };
						let Some(positions) = old_inputs[7].as_value().cloned() else { continue };
						let TaggedValue::GradientStops(positions) = positions else { continue };

						let fill = match (fill_type, solid_color) {
							(FillType::Solid, None) => Fill::None,
							(FillType::Solid, Some(color)) => Fill::Solid(color),
							(FillType::Gradient, _) => Fill::Gradient(Gradient {
								stops: positions,
								gradient_type,
								start,
								end,
								transform,
							}),
						};
						node.inputs[1] = NodeInput::value(TaggedValue::Fill(fill.clone()), false);
						match fill {
							Fill::None => {
								node.inputs[2] = NodeInput::value(TaggedValue::OptionalColor(None), false);
							}
							Fill::Solid(color) => {
								node.inputs[2] = NodeInput::value(TaggedValue::OptionalColor(Some(color)), false);
							}
							Fill::Gradient(gradient) => {
								node.inputs[3] = NodeInput::value(TaggedValue::Gradient(gradient), false);
							}
						}
					}
				}

				// TODO: Eventually remove this (probably starting late 2024)
				// Upgrade document to the new vector manipulation format introduced in #1676
				let document_serialized_content = document.serialize_document();
				if upgrade_vector_manipulation_format && !document_serialized_content.is_empty() {
					responses.add(FrontendMessage::TriggerUpgradeDocumentToVectorManipulationFormat {
						document_id,
						document_name,
						document_is_auto_saved,
						document_is_saved,
						document_serialized_content,
					});
					return;
				}

				document.set_auto_save_state(document_is_auto_saved);
				document.set_save_state(document_is_saved);

				self.load_document(document, document_id, responses);
			}
			PortfolioMessage::PasteIntoFolder { clipboard, parent, insert_index } => {
				let paste = |entry: &CopyBufferEntry, responses: &mut VecDeque<_>| {
					if self.active_document().is_some() {
						trace!("Pasting into folder {parent:?} as index: {insert_index}");
						let nodes = entry.clone().nodes;
						let new_ids: HashMap<_, _> = nodes.iter().map(|(&id, _)| (id, NodeId(generate_uuid()))).collect();
						responses.add(GraphOperationMessage::AddNodesAsChild { nodes, new_ids, parent, insert_index });
					}
				};

				responses.add(DocumentMessage::DeselectAllLayers);

				for entry in self.copy_buffer[clipboard as usize].iter().rev() {
					paste(entry, responses)
				}
			}
			PortfolioMessage::PasteSerializedData { data } => {
				if let Some(document) = self.active_document() {
					if let Ok(data) = serde_json::from_str::<Vec<CopyBufferEntry>>(&data) {
						let parent = document.new_layer_parent(false);

						responses.add(DocumentMessage::DeselectAllLayers);
						responses.add(DocumentMessage::StartTransaction);

						for entry in data.into_iter().rev() {
							document.load_layer_resources(responses);
							let new_ids: HashMap<_, _> = entry.nodes.iter().map(|(&id, _)| (id, NodeId(generate_uuid()))).collect();
							responses.add(GraphOperationMessage::AddNodesAsChild {
								nodes: entry.nodes,
								new_ids,
								parent,
								insert_index: -1,
							});
						}
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
				let mut node_graph_open = false;
				if let Some(document) = self.active_document() {
					if !document.is_auto_saved() {
						responses.add(PortfolioMessage::AutoSaveDocument {
							// Safe to unwrap since we know that there is an active document
							document_id: self.active_document_id.unwrap(),
						});
					}
					node_graph_open = document.is_graph_overlay_open();
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
				responses.add(NavigationMessage::CanvasPan { delta: (0., 0.).into() });
				responses.add(NodeGraphMessage::RunDocumentGraph);
				responses.add(DocumentMessage::GraphViewOverlay { open: node_graph_open });
			}
			PortfolioMessage::SubmitDocumentExport {
				file_name,
				file_type,
				scale_factor,
				bounds,
				transparent_background,
			} => {
				let document = self.active_document_id.and_then(|id| self.documents.get_mut(&id)).expect("Tried to render non-existent document");
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
					self.documents.get_mut(&document_id).expect("Tried to render non-existent document"),
					ipp.viewport_bounds.size().as_uvec2(),
					false,
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
			PortfolioMessage::UpdateVelloPreference => {
				responses.add(NodeGraphMessage::RunDocumentGraph);
				self.persistent_data.use_vello = preferences.use_vello;
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

		// Extend with actions that require an active document
		if let Some(document) = self.active_document() {
			common.extend(document.actions());

			// Extend with actions that must have a selected layer
			if document.selected_nodes.selected_layers(document.metadata()).next().is_some() {
				common.extend(actions!(PortfolioMessageDiscriminant;
					Copy,
					Cut,
				));
			}
		}

		common
	}
}

impl PortfolioMessageHandler {
	pub async fn introspect_node(&self, node_path: &[NodeId]) -> Option<Arc<dyn std::any::Any>> {
		self.executor.introspect_node(node_path).await
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
		let new_doc_title_num = doc_title_numbers.binary_search(&0).unwrap_or_else(|e| e) + 1;

		match new_doc_title_num {
			1 => DEFAULT_DOCUMENT_NAME.to_string(),
			_ => format!("{DEFAULT_DOCUMENT_NAME} {new_doc_title_num}"),
		}
	}

	// TODO: Fix how this doesn't preserve tab order upon loading new document from *File > Open*
	fn load_document(&mut self, new_document: DocumentMessageHandler, document_id: DocumentId, responses: &mut VecDeque<Message>) {
		let mut new_document = new_document;
		self.document_ids.push(document_id);
		new_document.update_layers_panel_options_bar_widgets(responses);

		new_document.node_graph_handler.update_all_click_targets(&new_document.network, Vec::new());

		self.documents.insert(document_id, new_document);

		if self.active_document().is_some() {
			responses.add(BroadcastEvent::ToolAbort);
			responses.add(ToolMessage::DeactivateTools);
		}

		//TODO: Remove this and find a way to fix the issue where creating a new document when the node graph is open causes the transform in the new document to be incorrect
		responses.add(DocumentMessage::GraphViewOverlay { open: false });
		responses.add(PortfolioMessage::UpdateOpenDocumentsList);
		responses.add(PortfolioMessage::SelectDocument { document_id });
		responses.add(PortfolioMessage::LoadDocumentResources { document_id });
		responses.add(PortfolioMessage::UpdateDocumentWidgets);
		responses.add(ToolMessage::InitTools);
		responses.add(NodeGraphMessage::Init);
		responses.add(NavigationMessage::CanvasPan { delta: (0., 0.).into() });
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

	pub fn poll_node_graph_evaluation(&mut self, responses: &mut VecDeque<Message>) -> Result<(), String> {
		let Some(active_document) = self.active_document_id.and_then(|id| self.documents.get_mut(&id)) else {
			return Err("No active document".to_string());
		};

		let result = self.executor.poll_node_graph_evaluation(active_document, responses);
		if result.is_err() {
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
		}
		result
	}
}

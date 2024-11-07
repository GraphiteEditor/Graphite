use super::document::utility_types::document_metadata::LayerNodeIdentifier;
use super::document::utility_types::network_interface::{self, InputConnector, OutputConnector};
use super::utility_types::{PanelType, PersistentData};
use crate::application::generate_uuid;
use crate::consts::DEFAULT_DOCUMENT_NAME;
use crate::messages::dialog::simple_dialogs;
use crate::messages::frontend::utility_types::FrontendDocumentDetails;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::utility_types::clipboards::{Clipboard, CopyBufferEntry, INTERNAL_CLIPBOARD_COUNT};
use crate::messages::portfolio::document::DocumentMessageData;
use crate::messages::prelude::*;
use crate::messages::tool::utility_types::{HintData, HintGroup, ToolType};
use crate::node_graph_executor::{ExportConfig, NodeGraphExecutor};

use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeId, NodeInput};
use graphene_core::text::Font;
use graphene_std::vector::style::{Fill, FillType, Gradient};
use interpreted_executor::dynamic_executor::IntrospectError;

use std::sync::Arc;
use std::vec;

pub struct PortfolioMessageData<'a> {
	pub ipp: &'a InputPreprocessorMessageHandler,
	pub preferences: &'a PreferencesMessageHandler,
	pub current_tool: &'a ToolType,
}

#[derive(Debug, Default)]
pub struct PortfolioMessageHandler {
	menu_bar_message_handler: MenuBarMessageHandler,
	pub documents: HashMap<DocumentId, DocumentMessageHandler>,
	document_ids: Vec<DocumentId>,
	active_panel: PanelType,
	pub(crate) active_document_id: Option<DocumentId>,
	copy_buffer: [Vec<CopyBufferEntry>; INTERNAL_CLIPBOARD_COUNT as usize],
	pub persistent_data: PersistentData,
	pub executor: NodeGraphExecutor,
}

impl MessageHandler<PortfolioMessage, PortfolioMessageData<'_>> for PortfolioMessageHandler {
	fn process_message(&mut self, message: PortfolioMessage, responses: &mut VecDeque<Message>, data: PortfolioMessageData) {
		let PortfolioMessageData { ipp, preferences, current_tool } = data;

		match message {
			// Sub-messages
			PortfolioMessage::MenuBar(message) => {
				let mut has_active_document = false;
				let mut rulers_visible = false;
				let mut node_graph_open = false;
				let mut has_selected_nodes = false;
				let mut has_selected_layers = false;

				if let Some(document) = self.active_document_id.and_then(|document_id| self.documents.get_mut(&document_id)) {
					has_active_document = true;
					rulers_visible = document.rulers_visible;
					node_graph_open = document.is_graph_overlay_open();
					let selected_nodes = document.network_interface.selected_nodes(&[]).unwrap();
					has_selected_nodes = selected_nodes.selected_nodes().next().is_some();
					has_selected_layers = selected_nodes.selected_visible_layers(&document.network_interface).next().is_some();
				}
				self.menu_bar_message_handler.process_message(
					message,
					responses,
					MenuBarMessageData {
						has_active_document,
						rulers_visible,
						node_graph_open,
						has_selected_nodes,
						has_selected_layers,
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
							current_tool,
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
						current_tool,
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
				let Some(active_document) = self.active_document_id.and_then(|id| self.documents.get_mut(&id)) else {
					return;
				};

				let mut copy_val = |buffer: &mut Vec<CopyBufferEntry>| {
					let ordered_last_elements = active_document.network_interface.shallowest_unique_layers(&[]);

					for layer in ordered_last_elements {
						let layer_node_id = layer.to_node();

						let mut copy_ids = HashMap::new();
						copy_ids.insert(layer_node_id, NodeId(0));

						active_document
							.network_interface
							.upstream_flow_back_from_nodes(vec![layer_node_id], &[], network_interface::FlowType::LayerChildrenUpstreamFlow)
							.enumerate()
							.for_each(|(index, node_id)| {
								copy_ids.insert(node_id, NodeId((index + 1) as u64));
							});

						buffer.push(CopyBufferEntry {
							nodes: active_document.network_interface.copy_nodes(&copy_ids, &[]).collect(),
							selected: active_document
								.network_interface
								.selected_nodes(&[])
								.unwrap()
								.selected_layers_contains(layer, active_document.metadata()),
							visible: active_document.network_interface.selected_nodes(&[]).unwrap().layer_visible(layer, &active_document.network_interface),
							locked: active_document.network_interface.selected_nodes(&[]).unwrap().layer_locked(layer, &active_document.network_interface),
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
			} => {
				let font = Font::new(font_family, font_style);

				self.persistent_data.font_cache.insert(font, preview_url, data);
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
				responses.add(FrontendMessage::TriggerImport);
			}
			PortfolioMessage::LoadDocumentResources { document_id } => {
				if let Some(document) = self.document_mut(document_id) {
					document.load_layer_resources(responses);
				}
			}
			PortfolioMessage::LoadFont { font } => {
				if !self.persistent_data.font_cache.loaded_font(&font) {
					responses.add_front(FrontendMessage::TriggerFontLoad { font });
				}
			}
			PortfolioMessage::NewDocumentWithName { name } => {
				let mut new_document = DocumentMessageHandler::default();
				new_document.name = name;
				responses.add(DocumentMessage::PTZUpdate);

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

				let document = DocumentMessageHandler::deserialize_document(&document_serialized_content).map(|mut document| {
					document.name.clone_from(&document_name);
					document
				});

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
					// This can be used, if uncommented, to upgrade demo artwork with outdated document node internals from their definitions. Delete when it's no longer needed.
					// Used for upgrading old internal networks for demo artwork nodes. Will reset all node internals for any opened file
					for node_id in &document
						.network_interface
						.network_metadata(&[])
						.unwrap()
						.persistent_metadata
						.node_metadata
						.keys()
						.cloned()
						.collect::<Vec<NodeId>>()
					{
						if let Some(reference) = document
							.network_interface
							.network_metadata(&[])
							.unwrap()
							.persistent_metadata
							.node_metadata
							.get(node_id)
							.and_then(|node| node.persistent_metadata.reference.as_ref())
						{
							let node_definition = resolve_document_node_type(reference).unwrap();
							let default_definition_node = node_definition.default_node_template();
							document.network_interface.replace_implementation(node_id, &[], default_definition_node.document_node.implementation);
							document
								.network_interface
								.replace_implementation_metadata(node_id, &[], default_definition_node.persistent_node_metadata);
						}
					}
				}

				if document
					.network_interface
					.network_metadata(&[])
					.unwrap()
					.persistent_metadata
					.node_metadata
					.iter()
					.any(|(node_id, node)| node.persistent_metadata.reference.as_ref().is_some_and(|reference| reference == "Output") && *node_id == NodeId(0))
				{
					document.network_interface.delete_nodes(vec![NodeId(0)], true, &[]);
				}

				let node_ids = document.network_interface.network(&[]).unwrap().nodes.keys().cloned().collect::<Vec<_>>();
				for node_id in &node_ids {
					let Some(node_metadata) = document.network_interface.network_metadata(&[]).unwrap().persistent_metadata.node_metadata.get(node_id) else {
						log::error!("could not get node metadata for node {node_id} in deserialize_document");
						continue;
					};

					// Upgrade Fill nodes to the format change in #1778
					// TODO: Eventually remove this (probably starting late 2024)
					let Some(ref reference) = node_metadata.persistent_metadata.reference.clone() else {
						continue;
					};

					let Some(node) = document.network_interface.network(&[]).unwrap().nodes.get(node_id) else {
						log::error!("could not get node in deserialize_document");
						continue;
					};
					let inputs_count = node.inputs.len();

					if reference == "Fill" && inputs_count == 8 {
						let node_definition = resolve_document_node_type(reference).unwrap();
						let document_node = node_definition.default_node_template().document_node;
						document.network_interface.replace_implementation(node_id, &[], document_node.implementation.clone());

						let old_inputs = document.network_interface.replace_inputs(node_id, document_node.inputs.clone(), &[]);

						document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), &[]);

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
						document
							.network_interface
							.set_input(&InputConnector::node(*node_id, 1), NodeInput::value(TaggedValue::Fill(fill.clone()), false), &[]);
						match fill {
							Fill::None => {
								document
									.network_interface
									.set_input(&InputConnector::node(*node_id, 2), NodeInput::value(TaggedValue::OptionalColor(None), false), &[]);
							}
							Fill::Solid(color) => {
								document
									.network_interface
									.set_input(&InputConnector::node(*node_id, 2), NodeInput::value(TaggedValue::OptionalColor(Some(color)), false), &[]);
							}
							Fill::Gradient(gradient) => {
								document
									.network_interface
									.set_input(&InputConnector::node(*node_id, 3), NodeInput::value(TaggedValue::Gradient(gradient), false), &[]);
							}
						}
					}

					// Upgrade Text node to include line height and character spacing, which were previously hardcoded to 1, from https://github.com/GraphiteEditor/Graphite/pull/2016
					if reference == "Text" && inputs_count == 4 {
						let node_definition = resolve_document_node_type(reference).unwrap();
						let document_node = node_definition.default_node_template().document_node;
						document.network_interface.replace_implementation(node_id, &[], document_node.implementation.clone());

						let old_inputs = document.network_interface.replace_inputs(node_id, document_node.inputs.clone(), &[]);

						document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), &[]);
						document.network_interface.set_input(&InputConnector::node(*node_id, 1), old_inputs[1].clone(), &[]);
						document.network_interface.set_input(&InputConnector::node(*node_id, 2), old_inputs[2].clone(), &[]);
						document.network_interface.set_input(&InputConnector::node(*node_id, 3), old_inputs[3].clone(), &[]);
						document
							.network_interface
							.set_input(&InputConnector::node(*node_id, 4), NodeInput::value(TaggedValue::F64(1.), false), &[]);
						document
							.network_interface
							.set_input(&InputConnector::node(*node_id, 5), NodeInput::value(TaggedValue::F64(1.), false), &[]);
					}

					// Upgrade Sine, Cosine, and Tangent nodes to include a boolean input for whether the output should be in radians, which was previously the only option but is now not the default
					if (reference == "Sine" || reference == "Cosine" || reference == "Tangent") && inputs_count == 1 {
						let node_definition = resolve_document_node_type(reference).unwrap();
						let document_node = node_definition.default_node_template().document_node;
						document.network_interface.replace_implementation(node_id, &[], document_node.implementation.clone());

						let old_inputs = document.network_interface.replace_inputs(node_id, document_node.inputs.clone(), &[]);

						document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), &[]);
						document
							.network_interface
							.set_input(&InputConnector::node(*node_id, 1), NodeInput::value(TaggedValue::Bool(true), false), &[]);
					}

					// Upgrade the Modulo node to include a boolean input for whether the output should be always positive, which was previously not an option
					if reference == "Modulo" && inputs_count == 2 {
						let node_definition = resolve_document_node_type(reference).unwrap();
						let document_node = node_definition.default_node_template().document_node;
						document.network_interface.replace_implementation(node_id, &[], document_node.implementation.clone());

						let old_inputs = document.network_interface.replace_inputs(node_id, document_node.inputs.clone(), &[]);

						document.network_interface.set_input(&InputConnector::node(*node_id, 0), old_inputs[0].clone(), &[]);
						document.network_interface.set_input(&InputConnector::node(*node_id, 1), old_inputs[1].clone(), &[]);
						document
							.network_interface
							.set_input(&InputConnector::node(*node_id, 2), NodeInput::value(TaggedValue::Bool(false), false), &[]);
					}

					// Upgrade layer implementation from https://github.com/GraphiteEditor/Graphite/pull/1946
					if reference == "Merge" || reference == "Artboard" {
						let node_definition = crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type(reference).unwrap();
						let new_merge_node = node_definition.default_node_template();
						document.network_interface.replace_implementation(node_id, &[], new_merge_node.document_node.implementation)
					}

					// Upgrade artboard name being passed as hidden value input to "To Artboard"
					if reference == "Artboard" {
						let label = document.network_interface.frontend_display_name(node_id, &[]);
						document
							.network_interface
							.set_input(&InputConnector::node(NodeId(0), 1), NodeInput::value(TaggedValue::String(label), false), &[*node_id]);
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

				// Ensure layers are positioned as stacks if they upstream siblings of another layer
				document.network_interface.load_structure();
				let all_layers = LayerNodeIdentifier::ROOT_PARENT.descendants(document.network_interface.document_metadata()).collect::<Vec<_>>();
				for layer in all_layers {
					let Some((downstream_node, input_index)) = document
						.network_interface
						.outward_wires(&[])
						.and_then(|outward_wires| outward_wires.get(&OutputConnector::node(layer.to_node(), 0)))
						.and_then(|outward_wires| outward_wires.first())
						.and_then(|input_connector| input_connector.node_id().map(|node_id| (node_id, input_connector.input_index())))
					else {
						continue;
					};
					// If the downstream node is a layer and the input is the first input and the current layer is not in a stack
					if input_index == 0 && document.network_interface.is_layer(&downstream_node, &[]) && !document.network_interface.is_stack(&layer.to_node(), &[]) {
						// Ensure the layer is horizontally aligned with the downstream layer to prevent changing the layout of old files
						let (Some(layer_position), Some(downstream_position)) =
							(document.network_interface.position(&layer.to_node(), &[]), document.network_interface.position(&downstream_node, &[]))
						else {
							log::error!("Could not get position for layer {:?} or downstream node {} when opening file", layer.to_node(), downstream_node);
							continue;
						};
						if layer_position.x == downstream_position.x {
							document.network_interface.set_stack_position_calculated_offset(&layer.to_node(), &downstream_node, &[]);
						}
					}
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
						let new_ids: HashMap<_, _> = nodes.iter().map(|(id, _)| (*id, NodeId::new())).collect();
						let layer = LayerNodeIdentifier::new_unchecked(new_ids[&NodeId(0)]);
						responses.add(NodeGraphMessage::AddNodes { nodes, new_ids });
						responses.add(NodeGraphMessage::MoveLayerToStack { layer, parent, insert_index });
					}
				};

				responses.add(DocumentMessage::DeselectAllLayers);

				for entry in self.copy_buffer[clipboard as usize].iter().rev() {
					paste(entry, responses)
				}
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			PortfolioMessage::PasteSerializedData { data } => {
				if let Some(document) = self.active_document() {
					if let Ok(data) = serde_json::from_str::<Vec<CopyBufferEntry>>(&data) {
						let parent = document.new_layer_parent(false);

						let mut added_nodes = false;

						for entry in data.into_iter().rev() {
							if !added_nodes {
								responses.add(DocumentMessage::DeselectAllLayers);
								responses.add(DocumentMessage::AddTransaction);
								added_nodes = true;
							}
							document.load_layer_resources(responses);
							let new_ids: HashMap<_, _> = entry.nodes.iter().map(|(id, _)| (*id, NodeId::new())).collect();
							let layer = LayerNodeIdentifier::new_unchecked(new_ids[&NodeId(0)]);
							responses.add(NodeGraphMessage::AddNodes { nodes: entry.nodes, new_ids });
							responses.add(NodeGraphMessage::MoveLayerToStack { layer, parent, insert_index: 0 });
						}
						responses.add(NodeGraphMessage::RunDocumentGraph);
					}
				}
			}
			PortfolioMessage::PasteImage {
				name,
				image,
				mouse,
				parent_and_insert_index,
			} => {
				let create_document = self.documents.is_empty();

				if create_document {
					responses.add(PortfolioMessage::NewDocumentWithName {
						name: name.clone().unwrap_or("Untitled Document".into()),
					});
				}

				responses.add(DocumentMessage::PasteImage {
					name,
					image,
					mouse,
					parent_and_insert_index,
				});

				if create_document {
					// Wait for the document to be rendered so the click targets can be calculated in order to determine the artboard size that will encompass the pasted image
					responses.add(Message::StartBuffer);
					responses.add(DocumentMessage::WrapContentInArtboard { place_artboard_at_origin: true });

					// TODO: Figure out how to get StartBuffer to work here so we can delete this and use `DocumentMessage::ZoomCanvasToFitAll` instead
					// Currently, it is necessary to use `FrontendMessage::TriggerDelayedZoomCanvasToFitAll` rather than `DocumentMessage::ZoomCanvasToFitAll` because the size of the viewport is not yet populated
					responses.add(Message::StartBuffer);
					responses.add(FrontendMessage::TriggerDelayedZoomCanvasToFitAll);
				}
			}
			PortfolioMessage::PasteSvg {
				name,
				svg,
				mouse,
				parent_and_insert_index,
			} => {
				let create_document = self.documents.is_empty();

				if create_document {
					responses.add(PortfolioMessage::NewDocumentWithName {
						name: name.clone().unwrap_or("Untitled Document".into()),
					});
				}

				responses.add(DocumentMessage::PasteSvg {
					name,
					svg,
					mouse,
					parent_and_insert_index,
				});

				if create_document {
					// Wait for the document to be rendered so the click targets can be calculated in order to determine the artboard size that will encompass the pasted image
					responses.add(Message::StartBuffer);
					responses.add(DocumentMessage::WrapContentInArtboard { place_artboard_at_origin: true });

					// TODO: Figure out how to get StartBuffer to work here so we can delete this and use `DocumentMessage::ZoomCanvasToFitAll` instead
					// Currently, it is necessary to use `FrontendMessage::TriggerDelayedZoomCanvasToFitAll` rather than `DocumentMessage::ZoomCanvasToFitAll` because the size of the viewport is not yet populated
					responses.add(Message::StartBuffer);
					responses.add(FrontendMessage::TriggerDelayedZoomCanvasToFitAll);
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
			PortfolioMessage::SetActivePanel { panel } => {
				self.active_panel = panel;
				responses.add(DocumentMessage::SetActivePanel { active_panel: self.active_panel });
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
				responses.add(ToolMessage::InitTools);
				responses.add(NodeGraphMessage::Init);
				responses.add(OverlaysMessage::Draw);
				responses.add(BroadcastEvent::ToolAbort);
				responses.add(BroadcastEvent::SelectionChanged);
				responses.add(NavigationMessage::CanvasPan { delta: (0., 0.).into() });
				responses.add(NodeGraphMessage::RunDocumentGraph);
				responses.add(DocumentMessage::GraphViewOverlay { open: node_graph_open });
				if node_graph_open {
					responses.add(NodeGraphMessage::UpdateGraphBarRight);
				} else {
					responses.add(PortfolioMessage::UpdateDocumentWidgets);
				}
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
			PortfolioMessage::SubmitGraphRender { document_id, ignore_hash } => {
				let result = self.executor.submit_node_graph_evaluation(
					self.documents.get_mut(&document_id).expect("Tried to render non-existent document"),
					ipp.viewport_bounds.size().as_uvec2(),
					ignore_hash,
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
			if document.network_interface.selected_nodes(&[]).unwrap().selected_layers(document.metadata()).next().is_some() {
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
	pub async fn introspect_node(&self, node_path: &[NodeId]) -> Result<Arc<dyn std::any::Any>, IntrospectError> {
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
		let new_document = new_document;
		self.document_ids.push(document_id);
		new_document.update_layers_panel_options_bar_widgets(responses);

		self.documents.insert(document_id, new_document);

		if self.active_document().is_some() {
			responses.add(BroadcastEvent::ToolAbort);
			responses.add(ToolMessage::DeactivateTools);
		} else {
			// Load the default font upon creating the first document
			let font = Font::new(graphene_core::consts::DEFAULT_FONT_FAMILY.into(), graphene_core::consts::DEFAULT_FONT_STYLE.into());
			responses.add(FrontendMessage::TriggerFontLoad { font });
		}

		// TODO: Remove this and find a way to fix the issue where creating a new document when the node graph is open causes the transform in the new document to be incorrect
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
			responses.add(Message::EndBuffer(graphene_std::renderer::RenderMetadata::default()));
			responses.add(FrontendMessage::UpdateDocumentArtwork { svg: error });
		}
		result
	}
}

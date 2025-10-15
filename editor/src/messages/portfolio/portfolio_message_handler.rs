use super::document::utility_types::document_metadata::LayerNodeIdentifier;
use super::document::utility_types::network_interface;
use super::utility_types::{PanelType, PersistentData};
use crate::application::generate_uuid;
use crate::consts::{DEFAULT_DOCUMENT_NAME, DEFAULT_STROKE_WIDTH, FILE_EXTENSION};
use crate::messages::animation::TimingInformation;
use crate::messages::debug::utility_types::MessageLoggingVerbosity;
use crate::messages::dialog::simple_dialogs;
use crate::messages::frontend::utility_types::{DocumentDetails, OpenDocument};
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::DocumentMessageContext;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::utility_types::clipboards::{Clipboard, CopyBufferEntry, INTERNAL_CLIPBOARD_COUNT};
use crate::messages::portfolio::document::utility_types::network_interface::OutputConnector;
use crate::messages::portfolio::document::utility_types::nodes::SelectedNodes;
use crate::messages::portfolio::document_migration::*;
use crate::messages::preferences::SelectionMode;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::utility_functions::make_path_editable_is_allowed;
use crate::messages::tool::utility_types::{HintData, HintGroup, ToolType};
use crate::node_graph_executor::{ExportConfig, NodeGraphExecutor};
use derivative::*;
use glam::{DAffine2, DVec2};
use graph_craft::document::NodeId;
use graphene_std::Color;
use graphene_std::renderer::Quad;
use graphene_std::subpath::BezierHandles;
use graphene_std::text::Font;
use graphene_std::vector::misc::HandleId;
use graphene_std::vector::{PointId, SegmentId, Vector, VectorModificationType};
use std::vec;

#[derive(ExtractField)]
pub struct PortfolioMessageContext<'a> {
	pub ipp: &'a InputPreprocessorMessageHandler,
	pub preferences: &'a PreferencesMessageHandler,
	pub animation: &'a AnimationMessageHandler,
	pub current_tool: &'a ToolType,
	pub message_logging_verbosity: MessageLoggingVerbosity,
	pub reset_node_definitions_on_open: bool,
	pub timing_information: TimingInformation,
}

#[derive(Debug, Derivative, ExtractField)]
#[derivative(Default)]
pub struct PortfolioMessageHandler {
	menu_bar_message_handler: MenuBarMessageHandler,
	pub documents: HashMap<DocumentId, DocumentMessageHandler>,
	document_ids: VecDeque<DocumentId>,
	active_panel: PanelType,
	pub(crate) active_document_id: Option<DocumentId>,
	copy_buffer: [Vec<CopyBufferEntry>; INTERNAL_CLIPBOARD_COUNT as usize],
	pub persistent_data: PersistentData,
	pub executor: NodeGraphExecutor,
	pub selection_mode: SelectionMode,
	device_pixel_ratio: Option<f64>,
	pub reset_node_definitions_on_open: bool,
	pub data_panel_open: bool,
	#[derivative(Default(value = "true"))]
	pub layers_panel_open: bool,
	#[derivative(Default(value = "true"))]
	pub properties_panel_open: bool,
}

#[message_handler_data]
impl MessageHandler<PortfolioMessage, PortfolioMessageContext<'_>> for PortfolioMessageHandler {
	fn process_message(&mut self, message: PortfolioMessage, responses: &mut VecDeque<Message>, context: PortfolioMessageContext) {
		let PortfolioMessageContext {
			ipp,
			preferences,
			animation,
			current_tool,
			message_logging_verbosity,
			reset_node_definitions_on_open,
			timing_information,
		} = context;

		match message {
			// Sub-messages
			PortfolioMessage::MenuBar(message) => {
				self.menu_bar_message_handler.has_active_document = false;
				self.menu_bar_message_handler.canvas_tilted = false;
				self.menu_bar_message_handler.canvas_flipped = false;
				self.menu_bar_message_handler.rulers_visible = false;
				self.menu_bar_message_handler.node_graph_open = false;
				self.menu_bar_message_handler.has_selected_nodes = false;
				self.menu_bar_message_handler.has_selected_layers = false;
				self.menu_bar_message_handler.has_selection_history = (false, false);
				self.menu_bar_message_handler.make_path_editable_is_allowed = false;
				self.menu_bar_message_handler.data_panel_open = self.data_panel_open;
				self.menu_bar_message_handler.layers_panel_open = self.layers_panel_open;
				self.menu_bar_message_handler.properties_panel_open = self.properties_panel_open;
				self.menu_bar_message_handler.message_logging_verbosity = message_logging_verbosity;
				self.menu_bar_message_handler.reset_node_definitions_on_open = reset_node_definitions_on_open;

				if let Some(document) = self.active_document_id.and_then(|document_id| self.documents.get_mut(&document_id)) {
					self.menu_bar_message_handler.has_active_document = true;
					self.menu_bar_message_handler.canvas_tilted = document.document_ptz.tilt() != 0.;
					self.menu_bar_message_handler.canvas_flipped = document.document_ptz.flip;
					self.menu_bar_message_handler.rulers_visible = document.rulers_visible;
					self.menu_bar_message_handler.node_graph_open = document.is_graph_overlay_open();
					let selected_nodes = document.network_interface.selected_nodes();
					self.menu_bar_message_handler.has_selected_nodes = selected_nodes.selected_nodes().next().is_some();
					self.menu_bar_message_handler.has_selected_layers = selected_nodes.selected_visible_layers(&document.network_interface).next().is_some();
					self.menu_bar_message_handler.has_selection_history = {
						let metadata = &document.network_interface.document_network_metadata().persistent_metadata;
						(!metadata.selection_undo_history.is_empty(), !metadata.selection_redo_history.is_empty())
					};
					self.menu_bar_message_handler.make_path_editable_is_allowed = make_path_editable_is_allowed(&mut document.network_interface).is_some();
				}

				self.menu_bar_message_handler.process_message(message, responses, ());
			}
			PortfolioMessage::Document(message) => {
				if let Some(document_id) = self.active_document_id {
					if let Some(document) = self.documents.get_mut(&document_id) {
						let document_inputs = DocumentMessageContext {
							document_id,
							ipp,
							persistent_data: &self.persistent_data,
							executor: &mut self.executor,
							current_tool,
							preferences,
							device_pixel_ratio: self.device_pixel_ratio.unwrap_or(1.),
							data_panel_open: self.data_panel_open,
							layers_panel_open: self.layers_panel_open,
							properties_panel_open: self.properties_panel_open,
						};
						document.process_message(message, responses, document_inputs)
					}
				}
			}

			// Messages
			PortfolioMessage::Init => {
				// Tell frontend to load persistent preferences
				responses.add(FrontendMessage::TriggerLoadPreferences);

				// Tell frontend to load the current document
				responses.add(FrontendMessage::TriggerLoadFirstAutoSaveDocument);

				// Display the menu bar at the top of the window
				responses.add(MenuBarMessage::SendLayout);

				// Send the information for tooltips and categories for each node/input.
				responses.add(FrontendMessage::SendUIMetadata {
					node_descriptions: document_node_definitions::collect_node_descriptions(),
					node_types: document_node_definitions::collect_node_types(),
				});

				// Tell frontend to finish loading persistent documents
				responses.add(FrontendMessage::TriggerLoadRestAutoSaveDocuments);

				responses.add(FrontendMessage::TriggerOpenLaunchDocuments);
			}
			PortfolioMessage::DocumentPassMessage { document_id, message } => {
				if let Some(document) = self.documents.get_mut(&document_id) {
					let document_inputs = DocumentMessageContext {
						document_id,
						ipp,
						persistent_data: &self.persistent_data,
						executor: &mut self.executor,
						current_tool,
						preferences,
						device_pixel_ratio: self.device_pixel_ratio.unwrap_or(1.),
						data_panel_open: self.data_panel_open,
						layers_panel_open: self.layers_panel_open,
						properties_panel_open: self.properties_panel_open,
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
				responses.add(FrontendMessage::TriggerPersistenceWriteDocument {
					document_id,
					document: document.serialize_document(),
					details: DocumentDetails {
						name: document.name.clone(),
						path: document.path.clone(),
						is_saved: document.is_saved(),
						is_auto_saved: document.is_auto_saved(),
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
					responses.add(EventMessage::ToolAbort);
					responses.add(ToolMessage::DeactivateTools);

					// Clear relevant UI layouts if there are no documents
					responses.add(PropertiesPanelMessage::Clear);
					responses.add(DocumentMessage::ClearLayersPanel);
					responses.add(DataPanelMessage::ClearLayout);
					let hint_data = HintData(vec![HintGroup(vec![])]);
					responses.add(FrontendMessage::UpdateInputHints { hint_data });
				}

				for document_id in &self.document_ids {
					responses.add(FrontendMessage::TriggerPersistenceRemoveDocument { document_id: *document_id });
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
					responses.add(DataPanelMessage::ClearLayout);
					let hint_data = HintData(vec![HintGroup(vec![])]);
					responses.add(FrontendMessage::UpdateInputHints { hint_data });
				}

				// Actually delete the document (delay to delete document is required to let the document and properties panel messages above get processed)
				responses.add(PortfolioMessage::DeleteDocument { document_id });
				responses.add(FrontendMessage::TriggerPersistenceRemoveDocument { document_id });

				// Send the new list of document tab names
				responses.add(PortfolioMessage::UpdateOpenDocumentsList);
			}
			PortfolioMessage::CloseDocumentWithConfirmation { document_id } => {
				let target_document = self.documents.get(&document_id).unwrap();
				if target_document.is_saved() {
					responses.add(EventMessage::ToolAbort);
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
					let mut ordered_last_elements = active_document.network_interface.shallowest_unique_layers(&[]).collect::<Vec<_>>();

					ordered_last_elements.sort_by_key(|layer| {
						let Some(parent) = layer.parent(active_document.metadata()) else { return usize::MAX };
						DocumentMessageHandler::get_calculated_insert_index(active_document.metadata(), &SelectedNodes(vec![layer.to_node()]), parent)
					});

					for layer in ordered_last_elements.into_iter() {
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
							selected: active_document.network_interface.selected_nodes().selected_layers_contains(layer, active_document.metadata()),
							visible: active_document.network_interface.selected_nodes().layer_visible(layer, &active_document.network_interface),
							locked: active_document.network_interface.selected_nodes().layer_locked(layer, &active_document.network_interface),
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
						*self.document_ids.back().unwrap()
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
					let node_to_inspect = self.node_to_inspect();
					if let Ok(message) = self.executor.submit_node_graph_evaluation(
						self.documents.get_mut(document_id).expect("Tried to render non-existent document"),
						*document_id,
						ipp.viewport_bounds.size().as_uvec2(),
						timing_information,
						node_to_inspect,
						true,
					) {
						responses.add_front(message);
					}
				}

				if self.active_document_mut().is_some() {
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}
			}
			PortfolioMessage::EditorPreferences => self.executor.update_editor_preferences(preferences.editor_preferences()),
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
					responses.add(EventMessage::ToolAbort);
					responses.add(NavigationMessage::CanvasPan { delta: (0., 0.).into() });
				}

				self.load_document(new_document, document_id, self.layers_panel_open, responses, false);
				responses.add(PortfolioMessage::SelectDocument { document_id });
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
				document_path,
				document_serialized_content,
			} => {
				responses.add(PortfolioMessage::OpenDocumentFileWithId {
					document_id: DocumentId(generate_uuid()),
					document_name,
					document_path,
					document_is_auto_saved: false,
					document_is_saved: true,
					document_serialized_content,
					to_front: false,
					select_after_open: true,
				});
			}
			PortfolioMessage::ToggleResetNodesToDefinitionsOnOpen => {
				self.reset_node_definitions_on_open = !self.reset_node_definitions_on_open;
				responses.add(MenuBarMessage::SendLayout);
			}
			PortfolioMessage::OpenDocumentFileWithId {
				document_id,
				document_name,
				document_path,
				document_is_auto_saved,
				document_is_saved,
				document_serialized_content,
				to_front,
				select_after_open,
			} => {
				// Upgrade the document being opened to use fresh copies of all nodes
				let reset_node_definitions_on_open = reset_node_definitions_on_open || document_migration_reset_node_definition(&document_serialized_content);
				// Upgrade the document being opened with string replacements on the original JSON
				let document_serialized_content = document_migration_string_preprocessing(document_serialized_content);

				// Deserialize the document
				let document = DocumentMessageHandler::deserialize_document(&document_serialized_content);

				// Display an error to the user if the document could not be opened
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

				// Upgrade the document's nodes to be compatible with the latest version
				document_migration_upgrades(&mut document, reset_node_definitions_on_open);

				// Ensure each node has the metadata for its inputs
				for (node_id, node, path) in document.network_interface.document_network().clone().recursive_nodes() {
					document.network_interface.validate_input_metadata(node_id, node, &path);
					document.network_interface.validate_display_name_metadata(node_id, &path);
					document.network_interface.validate_output_names(node_id, node, &path);
				}

				// Ensure layers are positioned as stacks if they are upstream siblings of another layer
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

				// Set the save state of the document based on what's given to us by the caller to this message
				document.set_auto_save_state(document_is_auto_saved);
				document.set_save_state(document_is_saved);

				let document_name_from_path = document_path.as_ref().and_then(|path| {
					if path.extension().is_some_and(|e| e == FILE_EXTENSION) {
						path.file_stem().map(|n| n.to_string_lossy().to_string())
					} else {
						None
					}
				});

				match (document_name, document_path, document_name_from_path) {
					(Some(name), _, None) => {
						document.name = name;
					}
					(_, Some(path), Some(name)) => {
						document.name = name;
						document.path = Some(path);
					}
					(_, _, Some(name)) => {
						document.name = name;
					}
					_ => {
						document.name = DEFAULT_DOCUMENT_NAME.to_string();
					}
				}

				// Load the document into the portfolio so it opens in the editor
				self.load_document(document, document_id, self.layers_panel_open, responses, to_front);

				if select_after_open {
					responses.add(PortfolioMessage::SelectDocument { document_id });
				}
			}
			PortfolioMessage::PasteIntoFolder { clipboard, parent, insert_index } => {
				let mut all_new_ids = Vec::new();
				let paste = |entry: &CopyBufferEntry, responses: &mut VecDeque<_>, all_new_ids: &mut Vec<NodeId>| {
					if self.active_document().is_some() {
						trace!("Pasting into folder {parent:?} as index: {insert_index}");
						let nodes = entry.clone().nodes;
						let new_ids: HashMap<_, _> = nodes.iter().map(|(id, _)| (*id, NodeId::new())).collect();
						let layer = LayerNodeIdentifier::new_unchecked(new_ids[&NodeId(0)]);
						all_new_ids.extend(new_ids.values().cloned());
						responses.add(NodeGraphMessage::AddNodes { nodes, new_ids: new_ids.clone() });
						responses.add(NodeGraphMessage::MoveLayerToStack { layer, parent, insert_index });
					}
				};

				responses.add(DocumentMessage::DeselectAllLayers);

				for entry in self.copy_buffer[clipboard as usize].iter().rev() {
					paste(entry, responses, &mut all_new_ids)
				}
				responses.add(NodeGraphMessage::RunDocumentGraph);
				responses.add(NodeGraphMessage::SelectedNodesSet { nodes: all_new_ids });
			}
			PortfolioMessage::PasteSerializedData { data } => {
				if let Some(document) = self.active_document() {
					let mut all_new_ids = Vec::new();
					if let Ok(data) = serde_json::from_str::<Vec<CopyBufferEntry>>(&data) {
						let parent = document.new_layer_parent(false);
						let mut layers = Vec::new();

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
							all_new_ids.extend(new_ids.values().cloned());

							responses.add(NodeGraphMessage::AddNodes { nodes: entry.nodes, new_ids });
							responses.add(NodeGraphMessage::MoveLayerToStack { layer, parent, insert_index: 0 });
							layers.push(layer);
						}

						responses.add(NodeGraphMessage::RunDocumentGraph);
						responses.add(NodeGraphMessage::SelectedNodesSet { nodes: all_new_ids });
						responses.add(DeferMessage::AfterGraphRun {
							messages: vec![PortfolioMessage::CenterPastedLayers { layers }.into()],
						});
					}
				}
			}
			// Custom paste implementation for Path tool
			PortfolioMessage::PasteSerializedVector { data } => {
				// If using Path tool then send the operation to Path tool
				if *current_tool == ToolType::Path {
					responses.add(PathToolMessage::Paste { data });
					return;
				}

				// If not using Path tool, create new layers and add paths into those
				if let Some(document) = self.active_document() {
					let Ok(data) = serde_json::from_str::<Vec<(LayerNodeIdentifier, Vector, DAffine2)>>(&data) else {
						return;
					};

					let mut layers = Vec::new();

					for (_, new_vector, transform) in data {
						let Some(node_type) = resolve_document_node_type("Path") else {
							error!("Path node does not exist");
							continue;
						};
						let nodes = vec![(NodeId(0), node_type.default_node_template())];

						let parent = document.new_layer_parent(false);

						let layer = graph_modification_utils::new_custom(NodeId::new(), nodes, parent, responses);
						layers.push(layer);

						// Adding the transform back into the layer
						responses.add(GraphOperationMessage::TransformSet {
							layer,
							transform,
							transform_in: TransformIn::Local,
							skip_rerender: false,
						});

						// Add default fill and stroke to the layer
						let fill_color = Color::WHITE;
						let stroke_color = Color::BLACK;

						let fill = graphene_std::vector::style::Fill::solid(fill_color.to_gamma_srgb());
						responses.add(GraphOperationMessage::FillSet { layer, fill });

						let stroke = graphene_std::vector::style::Stroke::new(Some(stroke_color.to_gamma_srgb()), DEFAULT_STROKE_WIDTH);
						responses.add(GraphOperationMessage::StrokeSet { layer, stroke });

						// Create new point ids and add those into the existing Vector path
						let mut points_map = HashMap::new();
						for (point, position) in new_vector.point_domain.iter() {
							let new_point_id = PointId::generate();
							points_map.insert(point, new_point_id);
							let modification_type = VectorModificationType::InsertPoint { id: new_point_id, position };
							responses.add(GraphOperationMessage::Vector { layer, modification_type });
						}

						// Create new segment ids and add the segments into the existing Vector path
						let mut segments_map = HashMap::new();
						for (segment_id, bezier, start, end) in new_vector.segment_bezier_iter() {
							let new_segment_id = SegmentId::generate();

							segments_map.insert(segment_id, new_segment_id);

							let handles = match bezier.handles {
								BezierHandles::Linear => [None, None],
								BezierHandles::Quadratic { handle } => [Some(handle - bezier.start), None],
								BezierHandles::Cubic { handle_start, handle_end } => [Some(handle_start - bezier.start), Some(handle_end - bezier.end)],
							};

							let points = [points_map[&start], points_map[&end]];
							let modification_type = VectorModificationType::InsertSegment { id: new_segment_id, points, handles };
							responses.add(GraphOperationMessage::Vector { layer, modification_type });
						}

						// Set G1 continuity
						for handles in new_vector.colinear_manipulators {
							let to_new_handle = |handle: HandleId| -> HandleId {
								HandleId {
									ty: handle.ty,
									segment: segments_map[&handle.segment],
								}
							};
							let new_handles = [to_new_handle(handles[0]), to_new_handle(handles[1])];
							let modification_type = VectorModificationType::SetG1Continuous { handles: new_handles, enabled: true };
							responses.add(GraphOperationMessage::Vector { layer, modification_type });
						}
					}

					responses.add(NodeGraphMessage::RunDocumentGraph);
					responses.add(Message::Defer(DeferMessage::AfterGraphRun {
						messages: vec![PortfolioMessage::CenterPastedLayers { layers }.into()],
					}));
				}
			}
			PortfolioMessage::CenterPastedLayers { layers } => {
				if let Some(document) = self.active_document_mut() {
					let viewport_bounds_quad_pixels = Quad::from_box([DVec2::ZERO, ipp.viewport_bounds.size()]);
					let viewport_center_pixels = viewport_bounds_quad_pixels.center(); // In viewport pixel coordinates

					let doc_to_viewport_transform = document.metadata().document_to_viewport;
					let viewport_to_doc_transform = doc_to_viewport_transform.inverse();

					let viewport_quad_doc_space = viewport_to_doc_transform * viewport_bounds_quad_pixels;

					let mut top_level_items_to_center: Vec<LayerNodeIdentifier> = Vec::new();
					let mut artboards_in_selection: Vec<LayerNodeIdentifier> = Vec::new();

					for &layer_id in &layers {
						if document.network_interface.is_artboard(&layer_id.to_node(), &document.node_graph_handler.network) {
							artboards_in_selection.push(layer_id);
						}
					}

					for &layer_id in &layers {
						let is_child_of_selected_artboard = artboards_in_selection.iter().any(|&artboard_id| {
							if layer_id == artboard_id {
								return false;
							}
							layer_id.ancestors(document.metadata()).any(|ancestor| ancestor == artboard_id)
						});

						if !is_child_of_selected_artboard {
							top_level_items_to_center.push(layer_id);
						}
					}

					if top_level_items_to_center.is_empty() {
						return;
					}

					let mut combined_min_doc = DVec2::MAX;
					let mut combined_max_doc = DVec2::MIN;
					let mut has_any_bounds = false;

					for &item_id in &top_level_items_to_center {
						if let Some(bounds_doc) = document.metadata().bounding_box_document(item_id) {
							combined_min_doc = combined_min_doc.min(bounds_doc[0]);
							combined_max_doc = combined_max_doc.max(bounds_doc[1]);
							has_any_bounds = true;
						}
					}

					if !has_any_bounds {
						return;
					}

					let combined_bounds_doc_quad = Quad::from_box([combined_min_doc, combined_max_doc]);

					if combined_bounds_doc_quad.intersects(viewport_quad_doc_space) {
						return;
					}

					let combined_center_doc = combined_bounds_doc_quad.center();
					let combined_center_viewport_pixels = doc_to_viewport_transform.transform_point2(combined_center_doc);
					let translation_viewport_pixels_rounded = (viewport_center_pixels - combined_center_viewport_pixels).round();

					let final_translation_offset_doc = viewport_to_doc_transform.transform_vector2(translation_viewport_pixels_rounded);

					if final_translation_offset_doc.abs_diff_eq(glam::DVec2::ZERO, 1e-9) {
						return;
					}

					responses.add(DocumentMessage::AddTransaction);

					for &item_id in &top_level_items_to_center {
						if document.network_interface.is_artboard(&item_id.to_node(), &document.node_graph_handler.network) {
							if let Some(bounds_doc) = document.metadata().bounding_box_document(item_id) {
								let current_artboard_origin_doc = bounds_doc[0];
								let dimensions_doc = bounds_doc[1] - bounds_doc[0];
								let new_artboard_origin_doc = current_artboard_origin_doc + final_translation_offset_doc;

								responses.add(GraphOperationMessage::ResizeArtboard {
									layer: item_id,
									location: new_artboard_origin_doc.round().as_ivec2(),
									dimensions: dimensions_doc.round().as_ivec2(),
								});
							}
						} else {
							let current_abs_doc_transform = document.metadata().transform_to_document(item_id);

							let new_abs_doc_transform = DAffine2 {
								matrix2: current_abs_doc_transform.matrix2,
								translation: current_abs_doc_transform.translation + final_translation_offset_doc,
							};

							let transform = doc_to_viewport_transform * new_abs_doc_transform;

							responses.add(GraphOperationMessage::TransformSet {
								layer: item_id,
								transform,
								transform_in: TransformIn::Viewport,
								skip_rerender: false,
							});
						}
					}

					responses.add(NodeGraphMessage::RunDocumentGraph);
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
						name: name.clone().unwrap_or(DEFAULT_DOCUMENT_NAME.into()),
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
					responses.add(DeferMessage::AfterGraphRun {
						messages: vec![DocumentMessage::WrapContentInArtboard { place_artboard_at_origin: true }.into()],
					});
					responses.add(DeferMessage::AfterNavigationReady {
						messages: vec![DocumentMessage::ZoomCanvasToFitAll.into()],
					});
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
						name: name.clone().unwrap_or(DEFAULT_DOCUMENT_NAME.into()),
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
					responses.add(DeferMessage::AfterGraphRun {
						messages: vec![DocumentMessage::WrapContentInArtboard { place_artboard_at_origin: true }.into()],
					});

					responses.add(DeferMessage::AfterNavigationReady {
						messages: vec![DocumentMessage::ZoomCanvasToFitAll.into()],
					});
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
			PortfolioMessage::SetDevicePixelRatio { ratio } => {
				self.device_pixel_ratio = Some(ratio);
				responses.add(OverlaysMessage::Draw);
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
				responses.add(FrontendMessage::TriggerSaveActiveDocument { document_id });
				responses.add(ToolMessage::InitTools);
				responses.add(NodeGraphMessage::Init);
				responses.add(OverlaysMessage::Draw);
				responses.add(EventMessage::ToolAbort);
				responses.add(EventMessage::SelectionChanged);
				responses.add(NavigationMessage::CanvasPan { delta: (0., 0.).into() });
				responses.add(NodeGraphMessage::RunDocumentGraph);
				responses.add(DocumentMessage::GraphViewOverlay { open: node_graph_open });
				if node_graph_open {
					responses.add(NodeGraphMessage::UpdateGraphBarRight);
					responses.add(NodeGraphMessage::UnloadWires);
					responses.add(NodeGraphMessage::SendWires)
				} else {
					responses.add(PortfolioMessage::UpdateDocumentWidgets);
				}

				let Some(document) = self.documents.get_mut(&document_id) else {
					warn!("Tried to read non existent document");
					return;
				};
				if !document.is_loaded {
					document.is_loaded = true;
					responses.add(PortfolioMessage::LoadDocumentResources { document_id });
					responses.add(PortfolioMessage::UpdateDocumentWidgets);
					responses.add(PropertiesPanelMessage::Clear);
				}
			}
			PortfolioMessage::SubmitDocumentExport {
				name,
				file_type,
				scale_factor,
				bounds,
				transparent_background,
			} => {
				let document = self.active_document_id.and_then(|id| self.documents.get_mut(&id)).expect("Tried to render non-existent document");
				let export_config = ExportConfig {
					name,
					file_type,
					scale_factor,
					bounds,
					transparent_background,
					..Default::default()
				};
				let result = self.executor.submit_document_export(document, self.active_document_id.unwrap(), export_config);

				if let Err(description) = result {
					responses.add(DialogMessage::DisplayDialogError {
						title: "Unable to export document".to_string(),
						description,
					});
				}
			}
			PortfolioMessage::SubmitActiveGraphRender => {
				if let Some(document_id) = self.active_document_id {
					responses.add(PortfolioMessage::SubmitGraphRender { document_id, ignore_hash: false });
				}
			}
			PortfolioMessage::SubmitGraphRender { document_id, ignore_hash } => {
				let node_to_inspect = self.node_to_inspect();
				let Some(document) = self.documents.get_mut(&document_id) else {
					log::error!("Tried to render non-existent document");
					return;
				};
				let viewport_resolution = ipp.viewport_bounds.size().as_uvec2();

				let result = self
					.executor
					.submit_node_graph_evaluation(document, document_id, viewport_resolution, timing_information, node_to_inspect, ignore_hash);

				match result {
					Err(description) => {
						responses.add(DialogMessage::DisplayDialogError {
							title: "Unable to update node graph".to_string(),
							description,
						});
					}
					Ok(message) => responses.add_front(message),
				}
			}
			PortfolioMessage::ToggleDataPanelOpen => {
				self.data_panel_open = !self.data_panel_open;
				responses.add(MenuBarMessage::SendLayout);

				// Run the graph to grab the data
				if self.data_panel_open {
					// When opening, we make the frontend show the panel first so it can start receiving its message subscriptions for the data it will display
					responses.add(FrontendMessage::UpdateDataPanelState { open: self.data_panel_open });

					responses.add(NodeGraphMessage::RunDocumentGraph);
				} else {
					// If we don't clear the panel, the layout diffing system will assume widgets still exist when it attempts to update the data panel next time it is opened
					responses.add(DataPanelMessage::ClearLayout);

					// When closing, we make the frontend hide the panel last so it can finish receiving its message subscriptions before it is destroyed
					responses.add(FrontendMessage::UpdateDataPanelState { open: self.data_panel_open });
				}
			}
			PortfolioMessage::TogglePropertiesPanelOpen => {
				self.properties_panel_open = !self.properties_panel_open;
				responses.add(MenuBarMessage::SendLayout);

				responses.add(FrontendMessage::UpdatePropertiesPanelState { open: self.properties_panel_open });

				// Run the graph to grab the data
				if self.properties_panel_open {
					responses.add(NodeGraphMessage::RunDocumentGraph);
				}

				responses.add(PropertiesPanelMessage::Refresh);
			}
			PortfolioMessage::ToggleLayersPanelOpen => {
				self.layers_panel_open = !self.layers_panel_open;
				responses.add(MenuBarMessage::SendLayout);

				// Run the graph to grab the data
				if self.layers_panel_open {
					// When opening, we make the frontend show the panel first so it can start receiving its message subscriptions for the data it will display
					responses.add(FrontendMessage::UpdateLayersPanelState { open: self.layers_panel_open });

					responses.add(NodeGraphMessage::RunDocumentGraph);
					responses.add(DeferMessage::AfterGraphRun {
						messages: vec![NodeGraphMessage::UpdateLayerPanel.into(), DocumentMessage::DocumentStructureChanged.into()],
					});
				} else {
					// If we don't clear the panel, the layout diffing system will assume widgets still exist when it attempts to update the layers panel next time it is opened
					responses.add(DocumentMessage::ClearLayersPanel);

					// When closing, we make the frontend hide the panel last so it can finish receiving its message subscriptions before it is destroyed
					responses.add(FrontendMessage::UpdateLayersPanelState { open: self.layers_panel_open });
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
					document.update_document_widgets(responses, animation.is_playing(), timing_information.animation_time);
				}
			}
			PortfolioMessage::UpdateOpenDocumentsList => {
				// Send the list of document tab names
				let open_documents = self
					.document_ids
					.iter()
					.filter_map(|id| {
						self.documents.get(id).map(|document| OpenDocument {
							id: *id,
							details: DocumentDetails {
								name: document.name.clone(),
								path: document.path.clone(),
								is_saved: document.is_saved(),
								is_auto_saved: document.is_auto_saved(),
							},
						})
					})
					.collect::<Vec<_>>();
				responses.add(FrontendMessage::UpdateOpenDocumentsList { open_documents });
			}
			PortfolioMessage::UpdateVelloPreference => {
				let active = if cfg!(target_family = "wasm") { false } else { preferences.use_vello };
				responses.add(FrontendMessage::UpdateViewportHolePunch { active });
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
			ToggleDataPanelOpen,
		);

		// Extend with actions that require an active document
		if let Some(document) = self.active_document() {
			common.extend(document.actions());

			// Extend with actions that must have a selected layer
			if document.network_interface.selected_nodes().selected_layers(document.metadata()).next().is_some() {
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
	pub fn with_executor(executor: crate::node_graph_executor::NodeGraphExecutor) -> Self {
		Self { executor, ..Default::default() }
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

	fn load_document(&mut self, mut new_document: DocumentMessageHandler, document_id: DocumentId, layers_panel_open: bool, responses: &mut VecDeque<Message>, to_front: bool) {
		if to_front {
			self.document_ids.push_front(document_id);
		} else {
			self.document_ids.push_back(document_id);
		}
		new_document.update_layers_panel_control_bar_widgets(layers_panel_open, responses);
		new_document.update_layers_panel_bottom_bar_widgets(layers_panel_open, responses);

		self.documents.insert(document_id, new_document);

		if self.active_document().is_some() {
			responses.add(EventMessage::ToolAbort);
			responses.add(ToolMessage::DeactivateTools);
		} else {
			// Load the default font upon creating the first document
			let font = Font::new(graphene_std::consts::DEFAULT_FONT_FAMILY.into(), graphene_std::consts::DEFAULT_FONT_STYLE.into());
			responses.add(FrontendMessage::TriggerFontLoad { font });
		}

		// TODO: Remove this and find a way to fix the issue where creating a new document when the node graph is open causes the transform in the new document to be incorrect
		responses.add(DocumentMessage::GraphViewOverlay { open: false });
		responses.add(PortfolioMessage::UpdateOpenDocumentsList);
	}

	/// Returns an iterator over the open documents in order.
	pub fn ordered_document_iterator(&self) -> impl Iterator<Item = &DocumentMessageHandler> {
		self.document_ids.iter().map(|id| self.documents.get(id).expect("Document id was not found in the document hashmap"))
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
				<rect x="50%" y="50%" width="460" height="100" transform="translate(-230 -50)" rx="4" fill="var(--color-warning-yellow)" />
				<text x="50%" y="50%" dominant-baseline="middle" text-anchor="middle" font-size="18" fill="var(--color-2-mildblack)">
					<tspan x="50%" dy="-24" font-weight="bold">The document cannot render in its current state.</tspan>
					<tspan x="50%" dy="24">Undo to go back, if available, or check for error details</tspan>
					<tspan x="50%" dy="24">by clicking the <tspan font-style="italic">Node Graph</tspan> button up at the top right.</tspan>
				/text>"#
				// It's a mystery why the `/text>` tag above needs to be missing its `<`, but when it exists it prints the `<` character in the text. However this works with it removed.
				.to_string();
			responses.add(FrontendMessage::UpdateDocumentArtwork { svg: error });
		}
		result
	}

	/// Get the ID of the selected node that should be used as the current source for the Data panel.
	pub fn node_to_inspect(&self) -> Option<NodeId> {
		// Skip if the Data panel is not open
		if !self.data_panel_open {
			return None;
		}

		let document = self.documents.get(&self.active_document_id?)?;
		let selected_nodes = document.network_interface.selected_nodes().0;

		// Skip if there is not exactly one selected node
		if selected_nodes.len() != 1 {
			return None;
		}

		selected_nodes.first().copied()
	}
}

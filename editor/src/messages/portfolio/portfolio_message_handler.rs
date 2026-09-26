use super::document::utility_types::document_metadata::LayerNodeIdentifier;
use super::persistent_state::{PersistentStateMessage, PersistentStateMessageContext, PersistentStateMessageHandler};
use super::utility_types::PanelType;
use crate::application::{Editor, generate_uuid};
use crate::consts::{DEFAULT_DOCUMENT_NAME, FILE_EXTENSION, GDD_FILE_EXTENSION};
use crate::messages::animation::TimingInformation;
use crate::messages::dialog::simple_dialogs;
use crate::messages::frontend::utility_types::{DocumentInfo, PersistedState, SessionStatus};
use crate::messages::input_mapper::utility_types::keyboard::Key;
use crate::messages::input_mapper::utility_types::macros::{action_shortcut, action_shortcut_manual};
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::DocumentMessageContext;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions;
use crate::messages::portfolio::document::utility_types::network_interface::OutputConnector;
use crate::messages::portfolio::document_migration::*;
use crate::messages::portfolio::document_storage_io::{build_or_open_working_copy, compare_storage_against_runtime, open_gdd_document};
use crate::messages::preferences::SelectionMode;
use crate::messages::prelude::*;
use crate::messages::tool::utility_types::{HintData, ToolType};
use crate::messages::viewport::ToPhysical;
use crate::node_graph_executor::{ExportConfig, NodeGraphExecutor};
use glam::{DAffine2, DVec2};
use graph_craft::application_io::resource::{DataSource, ResourceHash};
use graph_craft::document::NodeId;
use graphene_std::renderer::Quad;
use std::path::PathBuf;
use std::sync::Arc;
use std::vec;

#[derive(ExtractField)]
pub struct PortfolioMessageContext<'a> {
	pub ipp: &'a InputPreprocessorMessageHandler,
	pub preferences: &'a PreferencesMessageHandler,
	pub animation: &'a AnimationMessageHandler,
	pub current_tool: &'a ToolType,
	pub reset_node_definitions_on_open: bool,
	pub timing_information: TimingInformation,
	pub viewport: &'a ViewportMessageHandler,
	pub resource_storage: &'a ResourceStorageMessageHandler,
}

#[derive(Debug, Default, ExtractField)]
pub struct PortfolioMessageHandler {
	pub documents: HashMap<DocumentId, DocumentMessageHandler>,
	unloaded_documents: HashMap<DocumentId, DocumentInfo>,
	/// Background eager loads whose trailing `SelectDocument` should be suppressed to keep focus on the user's active doc.
	// TODO: Eventually remove this document upgrade code
	pending_eager_loads: HashSet<DocumentId>,
	document_ids: VecDeque<DocumentId>,
	pub(crate) active_document_id: Option<DocumentId>,
	failed_documents: FailedDocumentsMessageHandler,
	persistent_state: PersistentStateMessageHandler,
	pub fonts: FontsMessageHandler,
	ingest: IngestMessageHandler,
	sync: SyncMessageHandler,
	pub executor: NodeGraphExecutor,
	pub selection_mode: SelectionMode,
	pub reset_node_definitions_on_open: bool,
	pub workspace: WorkspaceMessageHandler,
	working_copy_root: Option<PathBuf>,
	/// Number of documents not fully loaded. While nonzero, resource GC is skipped.
	pending_opens: usize,
}

#[message_handler_data]
impl MessageHandler<PortfolioMessage, PortfolioMessageContext<'_>> for PortfolioMessageHandler {
	fn process_message(&mut self, message: PortfolioMessage, responses: &mut VecDeque<Message>, context: PortfolioMessageContext) {
		let PortfolioMessageContext {
			ipp,
			preferences,
			animation,
			current_tool,
			reset_node_definitions_on_open,
			timing_information,
			viewport,
			resource_storage,
		} = context;

		match message {
			// Sub-messages
			PortfolioMessage::Document(message) => {
				if let Some(document_id) = self.active_document_id
					&& let Some(document) = self.documents.get_mut(&document_id)
				{
					let document_inputs = DocumentMessageContext {
						document_id,
						ipp,
						fonts: &self.fonts,
						executor: &mut self.executor,
						current_tool,
						preferences,
						viewport,
						resource_storage,
						data_panel_open: self.workspace.panel_layout.is_panel_visible(PanelType::Data) && !self.workspace.panel_layout.focus_document,
						layers_panel_open: self.workspace.panel_layout.is_panel_visible(PanelType::Layers) && !self.workspace.panel_layout.focus_document,
						properties_panel_open: self.workspace.panel_layout.is_panel_visible(PanelType::Properties) && !self.workspace.panel_layout.focus_document,
					};
					document.process_message(message, responses, document_inputs)
				}
			}
			PortfolioMessage::PersistentState(message) => {
				let context = PersistentStateMessageContext {
					persisted_state: self.persisted_state_snapshot(),
				};
				self.persistent_state.process_message(message, responses, context);
			}
			PortfolioMessage::FailedDocuments(message) => {
				let context = FailedDocumentsMessageContext { document_ids: &mut self.document_ids };
				self.failed_documents.process_message(message, responses, context);
			}
			PortfolioMessage::Fonts(message) => {
				let context = FontsMessageContext { resource_storage };
				self.fonts.process_message(message, responses, context);
			}
			PortfolioMessage::Ingest(message) => {
				let context = IngestMessageContext {
					document_open: self.active_document().is_some(),
				};
				self.ingest.process_message(message, responses, context);
			}
			PortfolioMessage::Sync(message) => {
				let context = SyncMessageContext {
					documents: &mut self.documents,
					active_document_id: self.active_document_id,
					resource_storage,
					preferences,
					ipp,
					viewport,
				};
				self.sync.process_message(message, responses, context);
			}
			PortfolioMessage::Workspace(message) => {
				let context = WorkspaceMessageContext {
					has_active_document: self.active_document_id.is_some(),
					has_no_documents: self.document_ids.is_empty(),
				};
				self.workspace.process_message(message, responses, context);
			}

			// Messages
			PortfolioMessage::Init => {
				responses.add(PersistentStateMessage::ReadState);

				// Initialize the frontend with environment information
				responses.add(FrontendMessage::UpdatePlatform {
					platform: Editor::environment().into(),
				});

				// Tell frontend to load persistent preferences
				responses.add(FrontendMessage::TriggerLoadPreferences);

				// Before loading any documents, initially prepare the welcome screen buttons layout
				responses.add(PortfolioMessage::RequestWelcomeScreenButtonsLayout);

				// Tell frontend to load documents passed in as launch arguments
				responses.add(FrontendMessage::TriggerOpenLaunchDocuments);

				// Display the menu bar at the top of the window
				responses.add(MenuBarMessage::SendLayout);

				// Send the initial workspace panel layout to the frontend
				responses.add(WorkspaceMessage::UpdatePanelsLayout);

				// Request status bar info layout
				responses.add(PortfolioMessage::RequestStatusBarInfoLayout);

				// Send shortcuts for widgets created in the frontend which need shortcut tooltips
				responses.add(FrontendMessage::SendShortcutFullscreen {
					shortcut: action_shortcut_manual!(Key::F11),
					shortcut_mac: action_shortcut_manual!(Key::Control, Key::Command, Key::KeyF),
				});
				responses.add(FrontendMessage::SendShortcutAltClick {
					shortcut: action_shortcut_manual!(Key::Alt, Key::MouseLeft),
				});
				responses.add(FrontendMessage::SendShortcutShiftClick {
					shortcut: action_shortcut_manual!(Key::Shift, Key::MouseLeft),
				});

				// Send the information for tooltips and categories for each node/input.
				responses.add(FrontendMessage::SendUIMetadata {
					node_descriptions: document_node_definitions::collect_node_descriptions(),
					node_types: document_node_definitions::collect_node_types(),
				});
			}
			PortfolioMessage::DocumentPassMessage { document_id, message } => {
				if let Some(document) = self.documents.get_mut(&document_id) {
					let document_inputs = DocumentMessageContext {
						document_id,
						ipp,
						fonts: &self.fonts,
						executor: &mut self.executor,
						current_tool,
						preferences,
						viewport,
						resource_storage,
						data_panel_open: self.workspace.panel_layout.is_panel_visible(PanelType::Data) && !self.workspace.panel_layout.focus_document,
						layers_panel_open: self.workspace.panel_layout.is_panel_visible(PanelType::Layers) && !self.workspace.panel_layout.focus_document,
						properties_panel_open: self.workspace.panel_layout.is_panel_visible(PanelType::Properties) && !self.workspace.panel_layout.focus_document,
					};
					document.process_message(message, responses, document_inputs)
				}
			}
			PortfolioMessage::AutoSaveActiveDocument => {
				if let Some(document_id) = self.active_document_id
					&& let Some(document) = self.active_document_mut()
				{
					document.set_auto_save_state(true);
					responses.add(PortfolioMessage::AutoSaveDocument { document_id });
				}
			}
			PortfolioMessage::AutoSaveAllDocuments => {
				for document_id in self.document_ids.iter() {
					if let Some(document) = self.documents.get_mut(document_id)
						&& !document.is_auto_saved()
					{
						document.set_auto_save_state(true);
						responses.add(PortfolioMessage::AutoSaveDocument { document_id: *document_id });
					}
				}

				responses.add(PortfolioMessage::GarbageCollectResources);
			}
			PortfolioMessage::AutoSaveDocument { document_id } => {
				let validate = preferences.validate_storage_round_trip;
				let Some(document) = self.documents.get_mut(&document_id) else { return };

				document.commit_storage_snapshot(&resource_storage.resources_mut(), validate);

				responses.add(PersistentStateMessage::WriteDocument {
					document_id,
					document: document.serialize_document(),
				});
				responses.add(PersistentStateMessage::WriteState);
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
					HintData::clear_layout(responses);
				}

				for document_id in &self.document_ids {
					responses.add(PersistentStateMessage::DeleteDocument { document_id: *document_id });
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
				if self.document_ids.len() == 1 && self.document_ids[0] == document_id {
					// Clear UI layouts that assume the existence of a document
					responses.add(PropertiesPanelMessage::Clear);
					responses.add(DocumentMessage::ClearLayersPanel);
					responses.add(DataPanelMessage::ClearLayout);
					HintData::clear_layout(responses);
				}

				// Actually delete the document (delay to delete document is required to let the document and properties panel messages above get processed)
				responses.add(PortfolioMessage::DeleteDocument { document_id });
				responses.add(PersistentStateMessage::DeleteDocument { document_id });

				// Send the new list of document tab names
				responses.add(PortfolioMessage::UpdateOpenDocumentsList);
			}
			PortfolioMessage::CloseDocumentWithConfirmation { document_id } => {
				let Some(target_document) = self.document(document_id) else {
					responses.add(EventMessage::ToolAbort);
					responses.add(PortfolioMessage::CloseDocument { document_id });
					return;
				};
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
			PortfolioMessage::DeleteDocument { document_id } => {
				let document_index = self.document_index(document_id);
				self.documents.remove(&document_id);
				self.unloaded_documents.remove(&document_id);
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
			PortfolioMessage::DocumentStorageMounted { document_id, reopened, mounted } => {
				let Some(document) = self.documents.get_mut(&document_id) else {
					// Document was closed before its working copy finished mounting.
					return;
				};
				let Some(mounted) = mounted else {
					log::error!("DocumentStorageMounted for {document_id:?} arrived without its payload");
					return;
				};
				let (gdd, declarations) = *mounted;
				document.set_storage(gdd, declarations, resource_storage.resources_mut().storage());
				if !reopened {
					document.commit_storage_snapshot(&resource_storage.resources_mut(), preferences.validate_storage_round_trip);
					document.retire_storage_interaction();
				}
				responses.add(SyncMessage::StorageMounted { document_id });
			}
			PortfolioMessage::DestroyAllDocuments => {
				// Empty the list of internal document data
				self.documents.clear();
				self.unloaded_documents.clear();
				self.document_ids.clear();
				self.active_document_id = None;
				responses.add(MenuBarMessage::SendLayout);
				responses.add(PersistentStateMessage::WriteState);
			}
			PortfolioMessage::EditorPreferences => self.executor.update_editor_preferences(preferences.editor_preferences()),
			PortfolioMessage::GarbageCollectResources => {
				if !self.persistent_state.loaded() {
					// We don't know what can be safely garbage collected
					return;
				}
				if self.pending_opens > 0 {
					// A document not fully loaded, skip garbage collection.
					return;
				}

				let mut used_resources = HashSet::new();
				for (id, info) in self.unloaded_documents.iter() {
					if let Some(resources) = &info.resources {
						used_resources.extend(resources.iter());
					} else {
						responses.add(PersistentStateMessage::ReadDocument { document_id: *id });
						return;
					}
				}
				for document in self.documents.values_mut() {
					document.garbage_collect_resources();
					used_resources.extend(document.resources.registry.resolved().filter_map(|info| info.hash.cloned()));

					if let Some(storage) = document.storage() {
						used_resources.extend(storage.all_referenced_resource_hashes());
					}
					// A step taken back on undo names resources nothing else refers to until it is redone.
					used_resources.extend(document.retracted_resource_hashes());
				}
				used_resources.extend(self.fonts.used_resources());
				responses.add(ResourceStorageMessage::GarbageCollect {
					used: Vec::from_iter(used_resources).into_boxed_slice(),
				});
			}
			PortfolioMessage::ResolveResources => {
				for document_id in self.document_ids.iter().copied().collect::<Vec<_>>() {
					responses.add(PortfolioMessage::ResolveDocumentResources { document_id });
				}
			}
			PortfolioMessage::ResolveDocumentResources { document_id } => {
				responses.add(PortfolioMessage::DocumentPassMessage {
					document_id,
					message: DocumentMessage::Resource(ResourceMessage::ResolveAll),
				});
			}
			PortfolioMessage::LoadPersistedState { state } => {
				if let Some(layout) = state.workspace_layout {
					self.workspace.panel_layout = layout;
					// A workspace saved before the Session panel existed gets it as a tab beside Properties, behind it; one
					// where it was closed keeps it closed.
					let panel_layout = &mut self.workspace.panel_layout;
					if !panel_layout.is_panel_present(PanelType::Session) && !panel_layout.was_closed(PanelType::Session) {
						match panel_layout.find_panel(PanelType::Properties).and_then(|group_id| panel_layout.panel_group_mut(group_id)) {
							Some(group) => group.tabs.push(PanelType::Session),
							None => panel_layout.restore_panel(PanelType::Session),
						}
					}
					responses.add(WorkspaceMessage::UpdatePanelsLayout);

					// Refill panels whose content was lost when the layout load remounted their frontend components
					for group_id in self.workspace.panel_layout.root.all_group_ids() {
						if let Some(panel_type) = self.workspace.panel_layout.panel_group(group_id).and_then(|g| g.active_panel_type()) {
							WorkspaceMessageHandler::refresh_panel_content(panel_type, self.active_document_id.is_some(), self.document_ids.is_empty(), responses);
						}
					}
				}

				let PersistedState {
					documents,
					current_document,
					workspace_layout: _,
				} = state;

				// TODO: Eventually remove this document upgrade code
				let mut newly_unloaded_ids = Vec::new();

				for info in documents {
					if !self.document_ids.contains(&info.id) {
						self.document_ids.push_back(info.id);
					}
					if !self.documents.contains_key(&info.id) && !self.unloaded_documents.contains_key(&info.id) {
						// TODO: Eventually remove this document upgrade code
						newly_unloaded_ids.push(info.id);

						self.unloaded_documents.insert(info.id, info);
					}
				}

				responses.add(PortfolioMessage::UpdateOpenDocumentsList);

				let select_document_id = current_document.filter(|id| self.document_ids.contains(id)).or_else(|| self.document_ids.front().copied());

				// Eagerly load every autosaved doc on startup so deserialization failures can be reported in one batched dialog at the end.
				// The active doc's read is deferred to the `SelectDocument` below, but is still counted.
				// TODO: Eventually remove this document upgrade code
				self.failed_documents.expect_autosave_loads(newly_unloaded_ids.len());

				// TODO: Eventually remove this document upgrade code
				for document_id in &newly_unloaded_ids {
					if Some(*document_id) != select_document_id {
						self.pending_eager_loads.insert(*document_id);
						responses.add(PersistentStateMessage::ReadDocument { document_id: *document_id });
					}
				}

				if let Some(document_id) = select_document_id {
					responses.add(PortfolioMessage::SelectDocument { document_id });
				}
				// TODO: Eventually remove this document upgrade code
				else if self.failed_documents.has_failures_to_report() {
					responses.add(FailedDocumentsMessage::ShowFailedToLoadDocumentsDialog);
				}
			}
			PortfolioMessage::LoadDocumentContent {
				document_id,
				document_serialized_content,
			} => {
				let Some(info) = self.unloaded_documents.remove(&document_id) else {
					log::error!("Tried to load content for non existent document");
					return;
				};

				responses.add(PortfolioMessage::LoadDocument {
					document_id,
					document_name: Some(info.name),
					document_path: info.path,
					document_is_auto_saved: true,
					document_is_saved: info.is_saved,
					document_serialized_content,
				});

				// Suppress auto-select for startup eager loads to keep focus on the user's active doc
				// TODO: Eventually remove this document upgrade code
				// TODO: (But keep the inner logic unconditionally, just remove the condition)
				if !self.pending_eager_loads.remove(&document_id) {
					responses.add(PortfolioMessage::SelectDocument { document_id });
				}
			}
			PortfolioMessage::NewDocumentWithName { name } => {
				let mut new_document = DocumentMessageHandler::default();
				new_document.name = self.resolve_document_name(name, None);

				responses.add(DocumentMessage::PTZUpdate);

				let document_id = DocumentId(generate_uuid());
				if self.active_document().is_some() {
					responses.add(EventMessage::ToolAbort);
					responses.add(NavigationMessage::CanvasPan { delta: (0., 0.).into() });
				}

				self.load_document(new_document, document_id, resource_storage, preferences, responses);
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
			PortfolioMessage::OpenDocumentFile {
				document_name,
				document_path,
				document_serialized_content,
			} => {
				let document_id = DocumentId(generate_uuid());
				responses.add(PortfolioMessage::LoadDocument {
					document_id,
					document_name,
					document_path,
					document_is_auto_saved: false,
					document_is_saved: true,
					document_serialized_content,
				});
				responses.add(PortfolioMessage::SelectDocument { document_id });
			}
			PortfolioMessage::OpenGddDocument {
				document_name,
				document_path,
				content,
			} => {
				let document_id = DocumentId(generate_uuid());

				// Suppress resource GC until the open completes
				self.pending_opens += 1;

				responses.add(open_gdd_document(
					self.working_copy_root.clone(),
					document_id,
					document_name,
					document_path,
					content,
					resource_storage.resources_mut(),
					preferences.validate_storage_round_trip,
				));
			}
			PortfolioMessage::GddDocumentLoaded {
				document_id,
				document_name,
				document_path,
				document,
			} => {
				self.pending_opens = self.pending_opens.saturating_sub(1);

				let Some(mut document) = document.map(|boxed| *boxed) else {
					let name = document_name
						.filter(|name| !name.trim().is_empty())
						.or_else(|| document_path.as_ref().and_then(|path| path.file_stem()).map(|stem| stem.to_string_lossy().into_owned()))
						.unwrap_or_default();
					simple_dialogs::FailedToOpenDocumentDialog { document_name: name }.send_dialog_to_frontend(responses);
					return;
				};
				document.finalize_storage_load();
				document.set_save_state(true);

				let name = document_name
					.filter(|name| !name.trim().is_empty())
					.or_else(|| document_path.as_ref().and_then(|path| path.file_stem()).map(|stem| stem.to_string_lossy().into_owned()))
					.unwrap_or_else(|| DEFAULT_DOCUMENT_NAME.to_string());
				document.name = self.resolve_document_name(name, None);
				document.path = document_path;

				// The working copy is already mounted (we opened the .gdd), so skip the async re-mount.
				self.load_document(document, document_id, resource_storage, preferences, responses);
				responses.add(PortfolioMessage::SelectDocument { document_id });
			}
			PortfolioMessage::ToggleResetNodesToDefinitionsOnOpen => {
				self.reset_node_definitions_on_open = !self.reset_node_definitions_on_open;
				responses.add(MenuBarMessage::SendLayout);
			}
			PortfolioMessage::LoadDocument {
				document_id,
				document_name,
				document_path,
				document_is_auto_saved,
				document_is_saved,
				document_serialized_content,
			} => {
				// Upgrade the document being opened to use fresh copies of all nodes
				let reset_node_definitions_on_open = reset_node_definitions_on_open || document_migration_reset_node_definition(&document_serialized_content);
				// Upgrade the document being opened with string replacements on the original JSON
				let document_serialized_content = document_migration_string_preprocessing(document_serialized_content);
				// Upgrade resources from being referend by hash to beeing referened by ID
				let (document_serialized_content, resource_hash_to_id_migration_map) = document_migration_replace_resources_referenced_by_hash(document_serialized_content);

				// Deserialize the document
				let document = DocumentMessageHandler::deserialize_document(&document_serialized_content);

				// Display an error to the user if the document could not be opened
				let mut document = match document {
					Ok(document) => document,
					Err(e) => {
						log::error!("{e}");
						// TODO: Eventually remove this document upgrade code
						// TODO: (Only the `if` branch, the `else` branch's manual-open dialog stays)
						if document_is_auto_saved {
							let name = document_name.unwrap_or_default();
							let info = DocumentInfo {
								id: document_id,
								name,
								resources: None,
								path: document_path,
								session: None,
								is_saved: document_is_saved,
							};
							self.document_ids.retain(|id| id != &document_id);
							self.failed_documents.record_failure(document_id, info, document_serialized_content);

							if self.active_document_id == Some(document_id) {
								self.active_document_id = None;
								if let Some(next_id) = self.document_ids.front().copied() {
									responses.add(PortfolioMessage::SelectDocument { document_id: next_id });
								}
							}

							responses.add(PortfolioMessage::UpdateOpenDocumentsList);
							self.failed_documents.tick_autosave_load_progress(responses, true);
						} else {
							let name = document_name
								.filter(|n| !n.trim().is_empty())
								.or_else(|| document_path.as_ref().and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().into_owned()))
								.unwrap_or_default();
							let dialog = simple_dialogs::FailedToOpenDocumentDialog { document_name: name };
							dialog.send_dialog_to_frontend(responses);
						}

						return;
					}
				};

				// Upgrade the document's nodes to be compatible with the latest version
				document_migration_upgrades(&mut document, reset_node_definitions_on_open);

				// Load the document's embedded resources into the resource storage
				std::mem::take(&mut document.resources.embedded).into_iter().for_each(|(hash, resource)| {
					let data: Arc<[u8]> = Arc::from(resource.as_ref());
					if ResourceHash::from(data.as_ref()) != hash {
						log::error!("Resource hash mismatch for resource with hash {hash}");
						return;
					}
					responses.add(ResourceStorageMessage::Store { data });

					// TODO: Eventually remove this document upgrade code
					// Register any resources that were previously referenced by hash
					if let Some(id) = resource_hash_to_id_migration_map.get(&hash)
						&& !document.resources.registry.contains(id)
					{
						document.resources.registry.resolve(id, hash);
						document.resources.registry.push_source_back(id, DataSource::Embedded);
					}
				});

				// Ensure each node has the metadata for its inputs
				for (node_id, node, path) in document.network_interface.document_network().clone().recursive_nodes() {
					document.network_interface.validate_input_metadata(node_id, node, &path);
					document.network_interface.validate_output_names(node_id, node, &path);
				}
				// Ensure layers are positioned as stacks if they are upstream siblings of another layer
				document.network_interface.load_structure();
				let all_layers = LayerNodeIdentifier::ROOT_PARENT.descendants(document.network_interface.document_metadata()).collect::<Vec<_>>();
				for layer in all_layers {
					let Some((downstream_node, input_index)) = document
						.network_interface
						.outward_wires(&[])
						.and_then(|outward_wires| outward_wires.get(&OutputConnector::primary_output(layer.to_node())))
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

				// Everything above is how the document arrives rather than an edit to it, so what those fix-ups
				// recorded must not ride along in the first real commit. They still have to reach storage, so
				// the next commit converts the whole document instead of staging the batch that follows them.
				document.network_interface.discard_deltas();
				document.require_whole_document_stage();

				// Set the save state of the document based on what's given to us by the caller to this message
				document.set_auto_save_state(document_is_auto_saved);
				document.set_save_state(document_is_saved);

				let document_name_from_path = document_path.as_ref().and_then(|path| {
					if path.extension().is_some_and(|extension| extension == FILE_EXTENSION || extension == GDD_FILE_EXTENSION) {
						path.file_stem().map(|stem| stem.to_string_lossy().to_string())
					} else {
						None
					}
				});

				let candidate_name = match (document_name, document_path, document_name_from_path) {
					(Some(name), _, None) => name,
					(_, Some(path), Some(name)) => {
						document.path = Some(path);
						name
					}
					(_, _, Some(name)) => name,
					_ => String::new(),
				};
				document.name = self.resolve_document_name(candidate_name, None);

				// Load the document into the portfolio so it opens in the editor
				self.load_document(document, document_id, resource_storage, preferences, responses);

				responses.add(AppWindowMessage::Focus);

				// TODO: Eventually remove this document upgrade code
				if document_is_auto_saved {
					self.failed_documents.tick_autosave_load_progress(responses, false);
				}
			}
			PortfolioMessage::CenterLayers { layers } => {
				if let Some(document) = self.active_document_mut() {
					let viewport_bounds_quad_pixels = Quad::from_box([DVec2::ZERO, viewport.size().into_dvec2()]); // In viewport pixel coordinates
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
									location: new_artboard_origin_doc.round(),
									dimensions: dimensions_doc.round(),
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
			PortfolioMessage::PrevDocument => {
				if let Some(active_document_id) = self.active_document_id {
					let len = self.document_ids.len();
					let current_index = self.document_index(active_document_id);
					let prev_index = (current_index + len - 1) % len;
					let prev_id = self.document_ids[prev_index];
					responses.add(PortfolioMessage::SelectDocument { document_id: prev_id });
				}
			}
			PortfolioMessage::ReorderDocument { document_id, new_index } => {
				let new_index = new_index.min(self.document_ids.len().saturating_sub(1));
				let Some(current_index) = self.document_ids.iter().position(|&id| id == document_id) else {
					return;
				};

				if new_index != current_index {
					self.document_ids.remove(current_index);
					self.document_ids.insert(new_index, document_id);

					responses.add(PortfolioMessage::UpdateOpenDocumentsList);

					// Re-send the active document so the frontend recalculates the active tab index after reordering
					if let Some(active_document_id) = self.active_document_id {
						responses.add(FrontendMessage::UpdateActiveDocument { document_id: active_document_id });
					}
				}
			}
			PortfolioMessage::RequestWelcomeScreenButtonsLayout => {
				let donate = "https://graphite.art/donate/";

				let table = LayoutGroup::table(
					vec![
						vec![
							TextButton::new("New Document")
								.icon("File")
								.flush(true)
								.on_commit(|_| DialogMessage::RequestNewDocumentDialog.into())
								.widget_instance(),
							ShortcutLabel::new(action_shortcut!(DialogMessageDiscriminant::RequestNewDocumentDialog)).widget_instance(),
						],
						vec![
							TextButton::new("Open Document").icon("Folder").flush(true).on_commit(|_| IngestMessage::Open.into()).widget_instance(),
							ShortcutLabel::new(action_shortcut!(IngestMessageDiscriminant::Open)).widget_instance(),
						],
						vec![
							TextButton::new("Open Demo Artwork")
								.icon("Image")
								.flush(true)
								.on_commit(|_| DialogMessage::RequestDemoArtworkDialog.into())
								.widget_instance(),
						],
						vec![
							TextButton::new("Support the Development Fund")
								.icon("Heart")
								.flush(true)
								.on_commit(move |_| FrontendMessage::TriggerVisitLink { url: donate.to_string() }.into())
								.widget_instance(),
						],
					],
					true,
				);

				responses.add(LayoutMessage::DestroyLayout {
					layout_target: LayoutTarget::WelcomeScreenButtons,
				});
				responses.add(LayoutMessage::SendLayout {
					layout: Layout(vec![table]),
					layout_target: LayoutTarget::WelcomeScreenButtons,
				});
			}
			PortfolioMessage::RequestStatusBarInfoLayout => {
				#[cfg(not(target_family = "wasm"))]
				let widgets = vec![TextLabel::new("Graphite 1.0.0-RC6").disabled(true).widget_instance()]; // TODO: After the RCs, call this "Graphite (beta) x.y.z"
				#[cfg(target_family = "wasm")]
				let widgets = vec![];

				let row = LayoutGroup::row(widgets);

				responses.add(LayoutMessage::SendLayout {
					layout: Layout(vec![row]),
					layout_target: LayoutTarget::StatusBarInfo,
				});
			}
			PortfolioMessage::RenameDocument { new_name } => {
				let resolved_name = self.resolve_document_name(new_name, self.active_document_id);
				responses.add(DocumentMessage::RenameDocument { new_name: resolved_name });
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

				if self.unloaded_documents.contains_key(&document_id) {
					let already_selected = self.active_document_id == Some(document_id);

					self.active_document_id = Some(document_id);
					responses.add(MenuBarMessage::SendLayout);
					responses.add(PortfolioMessage::UpdateOpenDocumentsList);
					responses.add(FrontendMessage::UpdateActiveDocument { document_id });

					if !already_selected {
						responses.add(PersistentStateMessage::ReadDocument { document_id });
					}

					return;
				}

				if !self.documents.contains_key(&document_id) {
					warn!("Tried to read non existent document");
					return;
				}

				// Set the new active document ID
				self.active_document_id = Some(document_id);

				responses.add(MenuBarMessage::SendLayout);
				responses.add(PortfolioMessage::UpdateOpenDocumentsList);
				responses.add(FrontendMessage::UpdateActiveDocument { document_id });
				responses.add(ToolMessage::InitTools);
				responses.add(SyncMessage::RefreshPanel);
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
					responses.add(PortfolioMessage::ResolveDocumentResources { document_id });
					responses.add(PortfolioMessage::UpdateDocumentWidgets);
					responses.add(PropertiesPanelMessage::Clear);
				}
			}
			PortfolioMessage::SubmitDocumentExport {
				name,
				file_type,
				scale_factor,
				bounds,
				artboard_name,
				artboard_count,
			} => {
				let document_id = self.active_document_id.expect("Tried to render non-existent document");
				let document = self.documents.get_mut(&document_id).expect("Tried to render non-existent document");
				let export_config = ExportConfig {
					name,
					file_type,
					scale_factor,
					bounds,
					artboard_name,
					artboard_count,
					..Default::default()
				};
				let result = self.executor.submit_document_export(document, document_id, export_config);

				if let Err(description) = result {
					responses.add(DialogMessage::DisplayDialogError {
						title: "Unable to export document".to_string(),
						description,
					});
				}
			}
			PortfolioMessage::SaveRasterizedExport { name, file_type, width, height, data } => match file_type.encode(width, height, data) {
				Ok(content) => responses.add(FrontendMessage::TriggerSaveFile {
					name,
					folder: None,
					filters: vec![file_type.file_filter()],
					content: content.into(),
				}),
				Err(description) => responses.add(DialogMessage::DisplayDialogError {
					title: "Unable to export document".to_string(),
					description,
				}),
			},
			PortfolioMessage::SubmitActiveGraphRender => {
				if let Some(document_id) = self.active_document_id {
					responses.add(PortfolioMessage::SubmitGraphRender { document_id, ignore_hash: false });
				}
			}
			PortfolioMessage::SubmitGraphRender { document_id, ignore_hash } => {
				let node_to_inspect = self.node_to_inspect();

				let Some(document) = self.documents.get_mut(&document_id) else {
					log::error!("Tried to render non-existent document {:?}", document_id);
					return;
				};

				// Skip rendering while any resource is still unresolved — the preprocessor would otherwise fail with
				// `ResourceNotFound`. `ResourceMessage::Resolved` queues `RunDocumentGraph` once each id resolves,
				// so the render fires automatically once the registry is complete.
				if document.resources.registry.unresolved().next().is_some() {
					return;
				}

				let document_to_viewport = document
					.navigation_handler
					.calculate_offset_transform(viewport.center_in_viewport_space().into(), &document.document_ptz);
				let pointer_position = document_to_viewport.inverse().transform_point2(ipp.mouse.position);

				let scale = viewport.scale();
				// Use exact physical dimensions from browser (via ResizeObserver's devicePixelContentBoxSize)
				let physical_resolution = viewport.size().to_physical().into_dvec2().round().as_uvec2();

				// TODO: Eventually remove this document upgrade code
				// A freshly-opened document with legacy gradients (whether newly decomposed or persisted from a save made before every bake landed) runs a
				// one-time measurement pre-pass instead of rendering, until every gradient's transform is baked into absolute space
				if !document.pending_gradient_bbox_bake.is_empty() && self.executor.drive_gradient_migration(document, document_id, physical_resolution, scale, responses) {
					return;
				}

				// TODO: Remove this when we do the SVG rendering with a separate library on desktop, thus avoiding a need for the hole punch.
				// TODO: See #3796. There is a second instance of this todo comment and code block (be sure to remove both).
				#[cfg(not(target_family = "wasm"))]
				responses.add_front(FrontendMessage::UpdateViewportHolePunch {
					active: document.render_mode != graphene_std::vector::style::RenderMode::SvgPreview,
				});

				let result = self
					.executor
					.submit_node_graph_evaluation(document, document_id, physical_resolution, scale, timing_information, node_to_inspect, ignore_hash, pointer_position);

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
			PortfolioMessage::SubmitEyedropperPreviewRender => {
				use crate::consts::EYEDROPPER_PREVIEW_AREA_RESOLUTION;

				let Some(document_id) = self.active_document_id else { return };
				let Some(document) = self.documents.get_mut(&document_id) else { return };

				let resolution = glam::UVec2::splat(EYEDROPPER_PREVIEW_AREA_RESOLUTION);
				let scale = viewport.scale();

				let preview_offset_in_viewport = ipp.mouse.position - (glam::DVec2::splat(EYEDROPPER_PREVIEW_AREA_RESOLUTION as f64 / 2.));
				let preview_offset_in_viewport = DAffine2::from_translation(preview_offset_in_viewport);

				let document_to_viewport = document.metadata().document_to_viewport;

				let preview_transform = preview_offset_in_viewport.inverse() * document_to_viewport;
				let pointer_position = document_to_viewport.inverse().transform_point2(ipp.mouse.position);

				let result = self
					.executor
					.submit_eyedropper_preview(document, document_id, preview_transform, pointer_position, resolution, scale, timing_information);

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
				let open_documents = self.document_ids.iter().filter_map(|id| self.document_details(*id)).collect::<Vec<_>>();

				let no_open_documents = open_documents.is_empty();

				responses.add(FrontendMessage::UpdateOpenDocumentsList { open_documents });
				responses.add(PersistentStateMessage::WriteState);

				if no_open_documents {
					responses.add(PortfolioMessage::RequestWelcomeScreenButtonsLayout);
				}
			}
		}
	}

	fn actions(&self) -> ActionList {
		let mut common = self.workspace.actions();
		common.extend(actions!(IngestMessageDiscriminant; Open));

		// Extend with actions that require an active document
		if let Some(document) = self.active_document() {
			common.extend(document.actions());
			common.extend(actions!(PortfolioMessageDiscriminant;
				CloseActiveDocumentWithConfirmation,
				CloseAllDocuments,
				CloseAllDocumentsWithConfirmation,
				ToggleRulers,
				NextDocument,
				PrevDocument,
			));
			common.extend(actions!(IngestMessageDiscriminant; Import));
		}

		common
	}
}

impl PortfolioMessageHandler {
	pub fn with_executor(executor: crate::node_graph_executor::NodeGraphExecutor) -> Self {
		Self { executor, ..Default::default() }
	}

	pub fn set_working_copy_root(&mut self, root: Option<PathBuf>) {
		self.working_copy_root = root;
	}

	pub fn document(&self, document_id: DocumentId) -> Option<&DocumentMessageHandler> {
		self.documents.get(&document_id)
	}

	pub fn document_mut(&mut self, document_id: DocumentId) -> Option<&mut DocumentMessageHandler> {
		self.documents.get_mut(&document_id)
	}

	pub fn active_document(&self) -> Option<&DocumentMessageHandler> {
		self.active_document_id.and_then(|id| self.document(id))
	}

	pub fn active_document_mut(&mut self) -> Option<&mut DocumentMessageHandler> {
		self.active_document_id.and_then(|id| self.document_mut(id))
	}

	pub fn active_document_id(&self) -> Option<DocumentId> {
		self.active_document_id
	}

	pub fn unsaved_document_names(&self) -> Vec<String> {
		self.document_ids
			.iter()
			.filter_map(|id| self.document_details(*id))
			.filter(|details| !details.is_saved)
			.map(|details| details.name)
			.collect()
	}

	pub fn persisted_state_snapshot(&self) -> PersistedState {
		let mut documents = self.document_ids.iter().filter_map(|id| self.document_details(*id)).collect::<Vec<_>>();

		// Keep failed-to-load docs referenced in `state.documents` so their autosave files survive `garbage_collect_document_files`
		// TODO: Eventually remove this document upgrade code
		for info in self.failed_documents.failed_document_infos() {
			documents.push(info.clone());
		}

		PersistedState {
			documents,
			current_document: self.active_document_id,
			workspace_layout: Some(self.workspace.panel_layout.clone()),
		}
	}

	/// Resolves a proposed document name: if it's empty or only whitespace, falls back to the next
	/// available "Untitled Document {N}" via [`Self::generate_new_document_name`]. Otherwise trims surrounding
	/// whitespace and returns it. `exclude` is forwarded so a renaming document can skip its own current
	/// name when computing the fallback (preventing self-collision).
	pub fn resolve_document_name(&self, name: String, exclude: Option<DocumentId>) -> String {
		let trimmed = name.trim();
		if trimmed.is_empty() { self.generate_new_document_name(exclude) } else { trimmed.to_string() }
	}

	/// `exclude` lets a renaming caller skip its own current name so a document can rename back to
	/// its existing slot rather than colliding with itself and getting bumped to the next number.
	pub fn generate_new_document_name(&self, exclude: Option<DocumentId>) -> String {
		let untitled = DEFAULT_DOCUMENT_NAME;

		// Collect the numbers already used by existing default-named documents, skipping the excluded one
		let taken_numbers = self
			.document_ids
			.iter()
			.filter(|&&id| exclude != Some(id))
			.filter_map(|&id| self.document_details(id))
			.filter_map(|doc| {
				let rest = doc.name.strip_prefix(untitled)?.trim();
				if rest.is_empty() { Some(1) } else { rest.parse::<usize>().ok() }
			})
			.collect::<HashSet<usize>>();

		// Pick the lowest number not already in use (a match always exists since the range is unbounded)
		let new_number = (1..).find(|number| !taken_numbers.contains(number)).unwrap_or(1);

		// Return "Untitled Document" for the first, then "Untitled Document {N}" for subsequent ones
		if new_number == 1 { untitled.to_string() } else { format!("{untitled} {new_number}") }
	}

	fn load_document(
		&mut self,
		mut new_document: DocumentMessageHandler,
		document_id: DocumentId,
		resource_storage: &ResourceStorageMessageHandler,
		preferences: &PreferencesMessageHandler,
		responses: &mut VecDeque<Message>,
	) {
		let is_new_document = !self.document_ids.contains(&document_id);
		if is_new_document {
			self.document_ids.push_back(document_id);
		}
		self.unloaded_documents.remove(&document_id);
		new_document.update_layers_panel_control_bar_widgets(
			self.workspace.panel_layout.is_panel_visible(PanelType::Layers) && !self.workspace.panel_layout.focus_document,
			responses,
		);
		new_document.update_layers_panel_bottom_bar_widgets(
			self.workspace.panel_layout.is_panel_visible(PanelType::Layers) && !self.workspace.panel_layout.focus_document,
			responses,
		);

		self.documents.insert(document_id, new_document);

		if self.active_document().is_some() {
			responses.add(EventMessage::ToolAbort);
			responses.add(ToolMessage::DeactivateTools);
		}

		// TODO: Remove this and find a way to fix the issue where creating a new document when the node graph is open causes the transform in the new document to be incorrect
		responses.add(DocumentMessage::GraphViewOverlay { open: false });
		if is_new_document {
			responses.add(PortfolioMessage::UpdateOpenDocumentsList);
		}

		// Mount the per-document `Gdd` working copy asynchronously.
		//
		// Only mount for legacy loads: a `.gdd` open already mounted its working copy
		// and remounting would overwrite it and drop that history state.
		if self.documents.get(&document_id).and_then(|document| document.storage()).is_none() {
			let legacy_network = self
				.documents
				.get(&document_id)
				.map(|document| document.network_interface.document_network().clone())
				.unwrap_or_default();

			responses.add(Self::mount_document_storage(
				self.working_copy_root.clone(),
				document_id,
				document_graph_storage::UserId(preferences.user_id),
				legacy_network,
				resource_storage.resources(),
				preferences.validate_storage_round_trip,
			));
		}
	}

	/// With a configured root the working copy lives at `<root>/<id_hex>`; without one it is in-memory.
	///
	/// On a *reopen* (existing working copy), the freshly-read `.gdd` is converted back to a runtime
	/// network and compared against `legacy_network` before
	/// the working copy attaches. It validates the `.gdd` *read* path.
	fn mount_document_storage(
		working_copy_root: Option<std::path::PathBuf>,
		document_id: DocumentId,
		user: document_graph_storage::UserId,
		legacy_network: graph_craft::document::NodeNetwork,
		byte_store: Box<dyn graph_craft::application_io::resource::LoadResource>,
		validate: bool,
	) -> Message {
		let path = working_copy_root.map(|root| root.join(format!("{:016x}", document_id.0)));
		let editor_version = crate::application::GRAPHITE_GIT_COMMIT_HASH.to_string();
		let peer = document_graph_storage::PeerId(generate_uuid());

		let future = async move {
			let (gdd, reopened) = match build_or_open_working_copy(path.as_deref(), peer, user, document_id.0, editor_version).await {
				Ok(result) => result,
				Err(error) => {
					log::error!("Failed to mount document storage for {document_id:?}: {error}");
					return Message::NoOp;
				}
			};

			let declarations = gdd.declarations(byte_store.as_ref()).await;

			if validate && reopened {
				compare_storage_against_runtime(&gdd, &legacy_network, &declarations, document_id);
			}

			Message::Portfolio(PortfolioMessage::DocumentStorageMounted {
				document_id,
				reopened,
				mounted: Some(Box::new((gdd, declarations))),
			})
		};
		future.into()
	}

	/// Returns an iterator over the open documents in order.
	pub fn ordered_document_iterator(&self) -> impl Iterator<Item = &DocumentMessageHandler> {
		self.document_ids.iter().filter_map(|id| self.document(*id))
	}

	fn document_index(&self, document_id: DocumentId) -> usize {
		self.document_ids.iter().position(|id| id == &document_id).expect("Active document is missing from document ids")
	}

	pub fn poll_node_graph_evaluation(&mut self, responses: &mut VecDeque<Message>) -> Result<(), String> {
		let Some(document_id) = self.active_document_id else {
			return Err("No active document".to_string());
		};
		let Some(active_document) = self.documents.get_mut(&document_id) else {
			return Err("No active document".to_string());
		};

		let result = self.executor.poll_node_graph_evaluation(active_document, document_id, responses);
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

	fn document_details(&self, document_id: DocumentId) -> Option<DocumentInfo> {
		if let Some(document) = self.documents.get(&document_id) {
			Some(DocumentInfo {
				id: document_id,
				name: document.name.clone(),
				path: document.path.clone(),
				is_saved: document.is_saved(),
				resources: Some(document.resources.registry.resolved().filter_map(|info| info.hash.cloned()).collect::<Vec<_>>().into_boxed_slice()),
				session: document.storage().and_then(|gdd| match (gdd.role().is_some(), gdd.is_shared()) {
					(true, _) => Some(SessionStatus::Connected),
					(false, true) => Some(SessionStatus::Disconnected),
					(false, false) => None,
				}),
			})
		} else {
			self.unloaded_documents.get(&document_id).cloned()
		}
	}

	/// Returns the full path from the root network to the selected node that should drive the Data panel.
	/// The last element is the node itself; preceding elements identify the nested subnetwork it lives in
	/// so the Data panel can introspect nodes inside subgraphs. An empty `Vec` signals "nothing to inspect".
	pub fn node_to_inspect(&self) -> Vec<NodeId> {
		// Skip if the Data panel is not open
		if !self.workspace.panel_layout.is_panel_visible(PanelType::Data) || self.workspace.panel_layout.focus_document {
			return Vec::new();
		}

		let Some(document) = self.active_document_id.and_then(|id| self.document(id)) else {
			return Vec::new();
		};
		let network_path = document.selection_network_path();
		let Some(selected_nodes) = document.network_interface.selected_nodes_in_nested_network(network_path) else {
			return Vec::new();
		};

		// Skip if there is not exactly one selected node
		let [node_id] = selected_nodes.0.as_slice() else {
			return Vec::new();
		};

		let mut path = network_path.to_vec();
		path.push(*node_id);
		path
	}
}

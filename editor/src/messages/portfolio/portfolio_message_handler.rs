use super::utility_types::PersistentData;
use crate::application::generate_uuid;
use crate::consts::{DEFAULT_DOCUMENT_NAME, GRAPHITE_DOCUMENT_VERSION};
use crate::messages::dialog::simple_dialogs;
use crate::messages::frontend::utility_types::{FrontendDocumentDetails, FrontendImageData};
use crate::messages::layout::utility_types::layout_widget::PropertyHolder;
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::portfolio::document::node_graph::resolve_document_node_type;
use crate::messages::portfolio::document::utility_types::clipboards::{Clipboard, CopyBufferEntry, INTERNAL_CLIPBOARD_COUNT};
use crate::messages::portfolio::document::utility_types::misc::DocumentRenderMode;
use crate::messages::portfolio::utility_types::ImaginateServerStatus;
use crate::messages::prelude::*;

use graph_craft::document::DocumentNodeImplementation;
use graph_craft::document::NodeId;
use graph_craft::document::{NodeInput, NodeNetwork};
use graphene::document::pick_safe_imaginate_resolution;
use graphene::layers::layer_info::{LayerDataType, LayerDataTypeDiscriminant};
use graphene::layers::text_layer::Font;
use graphene::{LayerId, Operation as DocumentOperation};
use graphene_core::raster::Image;

use glam::DVec2;
use std::borrow::Cow;

#[derive(Debug, Clone, Default)]
pub struct PortfolioMessageHandler {
	menu_bar_message_handler: MenuBarMessageHandler,
	documents: HashMap<u64, DocumentMessageHandler>,
	document_ids: Vec<u64>,
	active_document_id: Option<u64>,
	copy_buffer: [Vec<CopyBufferEntry>; INTERNAL_CLIPBOARD_COUNT as usize],
	pub persistent_data: PersistentData,
}

impl MessageHandler<PortfolioMessage, (&InputPreprocessorMessageHandler, &PreferencesMessageHandler)> for PortfolioMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: PortfolioMessage, (ipp, preferences): (&InputPreprocessorMessageHandler, &PreferencesMessageHandler), responses: &mut VecDeque<Message>) {
		#[remain::sorted]
		match message {
			// Sub-messages
			#[remain::unsorted]
			PortfolioMessage::MenuBar(message) => self.menu_bar_message_handler.process_message(message, (), responses),
			#[remain::unsorted]
			PortfolioMessage::Document(message) => {
				if let Some(document_id) = self.active_document_id {
					if let Some(document) = self.documents.get_mut(&document_id) {
						document.process_message(message, (document_id, ipp, &self.persistent_data, preferences), responses)
					}
				}
			}

			// Messages
			#[remain::unsorted]
			PortfolioMessage::DocumentPassMessage { document_id, message } => {
				if let Some(document) = self.documents.get_mut(&document_id) {
					document.process_message(message, (document_id, ipp, &self.persistent_data, preferences), responses)
				}
			}
			PortfolioMessage::AutoSaveActiveDocument => {
				if let Some(document_id) = self.active_document_id {
					if let Some(document) = self.active_document_mut() {
						document.set_auto_save_state(true);
					}
					responses.push_back(PortfolioMessage::AutoSaveDocument { document_id }.into());
				}
			}
			PortfolioMessage::AutoSaveDocument { document_id } => {
				let document = self.documents.get(&document_id).unwrap();
				responses.push_back(
					FrontendMessage::TriggerIndexedDbWriteDocument {
						document: document.serialize_document(),
						details: FrontendDocumentDetails {
							is_auto_saved: document.is_auto_saved(),
							is_saved: document.is_saved(),
							id: document_id,
							name: document.name.clone(),
						},
						version: GRAPHITE_DOCUMENT_VERSION.to_string(),
					}
					.into(),
				)
			}
			PortfolioMessage::CloseActiveDocumentWithConfirmation => {
				if let Some(document_id) = self.active_document_id {
					responses.push_back(PortfolioMessage::CloseDocumentWithConfirmation { document_id }.into());
				}
			}
			PortfolioMessage::CloseAllDocuments => {
				if self.active_document_id.is_some() {
					responses.push_back(PropertiesPanelMessage::Deactivate.into());
					responses.push_back(BroadcastEvent::ToolAbort.into());
					responses.push_back(ToolMessage::DeactivateTools.into());
				}

				for document_id in &self.document_ids {
					responses.push_back(FrontendMessage::TriggerIndexedDbRemoveDocument { document_id: *document_id }.into());
				}

				responses.push_back(PortfolioMessage::DestroyAllDocuments.into());
				responses.push_back(PortfolioMessage::UpdateOpenDocumentsList.into());
			}
			PortfolioMessage::CloseDocument { document_id } => {
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
					responses.push_back(PortfolioMessage::SelectDocument { document_id }.into());
				}

				// Send the new list of document tab names
				responses.push_back(PortfolioMessage::UpdateOpenDocumentsList.into());
				responses.push_back(FrontendMessage::TriggerIndexedDbRemoveDocument { document_id }.into());
				responses.push_back(DocumentMessage::RenderDocument.into());
				responses.push_back(DocumentMessage::DocumentStructureChanged.into());
				if let Some(document) = self.active_document() {
					for layer in document.layer_metadata.keys() {
						responses.push_back(DocumentMessage::LayerChanged { affected_layer_path: layer.clone() }.into());
					}
				}
			}
			PortfolioMessage::CloseDocumentWithConfirmation { document_id } => {
				let target_document = self.documents.get(&document_id).unwrap();
				if target_document.is_saved() {
					responses.push_back(BroadcastEvent::ToolAbort.into());
					responses.push_back(PortfolioMessage::CloseDocument { document_id }.into());
				} else {
					let dialog = simple_dialogs::CloseDocumentDialog {
						document_name: target_document.name.clone(),
						document_id,
					};
					dialog.register_properties(responses, LayoutTarget::DialogDetails);
					responses.push_back(FrontendMessage::DisplayDialog { icon: "File".to_string() }.into());

					// Select the document being closed
					responses.push_back(PortfolioMessage::SelectDocument { document_id }.into());
				}
			}
			PortfolioMessage::Copy { clipboard } => {
				// We can't use `self.active_document()` because it counts as an immutable borrow of the entirety of `self`
				if let Some(active_document) = self.active_document_id.and_then(|id| self.documents.get(&id)) {
					let copy_val = |buffer: &mut Vec<CopyBufferEntry>| {
						for layer_path in active_document.selected_layers_without_children() {
							match (active_document.graphene_document.layer(layer_path).map(|t| t.clone()), *active_document.layer_metadata(layer_path)) {
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

						responses.push_back(FrontendMessage::TriggerTextCopy { copy_text }.into());
					} else {
						let copy_buffer = &mut self.copy_buffer;
						copy_buffer[clipboard as usize].clear();
						copy_val(&mut copy_buffer[clipboard as usize]);
					}
				}
			}
			PortfolioMessage::Cut { clipboard } => {
				responses.push_back(PortfolioMessage::Copy { clipboard }.into());
				responses.push_back(DocumentMessage::DeleteSelectedLayers.into());
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
				self.persistent_data.font_cache.insert(Font::new(font_family, font_style), preview_url, data, is_default);

				if let Some(document) = self.active_document_mut() {
					document.graphene_document.mark_all_layers_of_type_as_dirty(LayerDataTypeDiscriminant::Text);
					responses.push_back(DocumentMessage::RenderDocument.into());
					responses.push_back(BroadcastEvent::DocumentIsDirty.into());
				}
			}
			PortfolioMessage::ImaginateCheckServerStatus => {
				self.persistent_data.imaginate_server_status = ImaginateServerStatus::Checking;
				responses.push_back(
					FrontendMessage::TriggerImaginateCheckServerStatus {
						hostname: preferences.imaginate_server_hostname.clone(),
					}
					.into(),
				);
				responses.push_back(PropertiesPanelMessage::ResendActiveProperties.into());
			}
			PortfolioMessage::ImaginateSetGeneratingStatus { .. } => {
				//let message = DocumentOperation::ImaginateSetGeneratingStatus { path, percent, status }.into();
				//responses.push_back(PortfolioMessage::DocumentPassMessage { document_id, message }.into());
			}
			PortfolioMessage::ImaginateSetImageData { .. } => {
				//let message = DocumentOperation::ImaginateSetImageData { layer_path, image_data }.into();
				//responses.push_back(PortfolioMessage::DocumentPassMessage { document_id, message }.into());
			}
			PortfolioMessage::ImaginateSetServerStatus { status } => {
				self.persistent_data.imaginate_server_status = status;
				responses.push_back(PropertiesPanelMessage::ResendActiveProperties.into());
			}
			PortfolioMessage::Import => {
				// This portfolio message wraps the frontend message so it can be listed as an action, which isn't possible for frontend messages
				if self.active_document().is_some() {
					responses.push_back(FrontendMessage::TriggerImport.into());
				}
			}
			PortfolioMessage::LoadDocumentResources { document_id } => {
				if let Some(document) = self.document_mut(document_id) {
					document.load_layer_resources(responses, &document.graphene_document.root.data, Vec::new(), document_id);
				}
			}
			PortfolioMessage::LoadFont { font, is_default } => {
				if !self.persistent_data.font_cache.loaded_font(&font) {
					responses.push_front(FrontendMessage::TriggerFontLoad { font, is_default }.into());
				}
			}
			PortfolioMessage::NewDocumentWithName { name } => {
				let new_document = DocumentMessageHandler::with_name(name, ipp);
				let document_id = generate_uuid();
				if self.active_document().is_some() {
					responses.push_back(BroadcastEvent::ToolAbort.into());
					responses.push_back(NavigationMessage::TranslateCanvas { delta: (0., 0.).into() }.into());
				}

				self.load_document(new_document, document_id, responses);
			}
			PortfolioMessage::NextDocument => {
				if let Some(active_document_id) = self.active_document_id {
					let current_index = self.document_index(active_document_id);
					let next_index = (current_index + 1) % self.document_ids.len();
					let next_id = self.document_ids[next_index];

					responses.push_back(PortfolioMessage::SelectDocument { document_id: next_id }.into());
				}
			}
			PortfolioMessage::OpenDocument => {
				// This portfolio message wraps the frontend message so it can be listed as an action, which isn't possible for frontend messages
				responses.push_back(FrontendMessage::TriggerOpenDocument.into());
			}
			PortfolioMessage::OpenDocumentFile {
				document_name,
				document_serialized_content,
			} => {
				responses.push_back(
					PortfolioMessage::OpenDocumentFileWithId {
						document_id: generate_uuid(),
						document_name,
						document_is_auto_saved: false,
						document_is_saved: true,
						document_serialized_content,
					}
					.into(),
				);
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
					Err(e) => responses.push_back(
						DialogMessage::DisplayDialogError {
							title: "Failed to open document".to_string(),
							description: e.to_string(),
						}
						.into(),
					),
				}
			}
			// TODO: Paste message is unused, delete it?
			PortfolioMessage::Paste { clipboard } => {
				let shallowest_common_folder = self.active_document().map(|document| {
					document
						.graphene_document
						.shallowest_common_folder(document.selected_layers())
						.expect("While pasting, the selected layers did not exist while attempting to find the appropriate folder path for insertion")
				});

				if let Some(folder) = shallowest_common_folder {
					responses.push_back(DocumentMessage::DeselectAllLayers.into());
					responses.push_back(DocumentMessage::StartTransaction.into());
					responses.push_back(
						PortfolioMessage::PasteIntoFolder {
							clipboard,
							folder_path: folder.to_vec(),
							insert_index: -1,
						}
						.into(),
					);
					responses.push_back(DocumentMessage::CommitTransaction.into());
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

						responses.push_front(
							DocumentMessage::UpdateLayerMetadata {
								layer_path: destination_path.clone(),
								layer_metadata: entry.layer_metadata,
							}
							.into(),
						);
						document.load_layer_resources(responses, &entry.layer.data, destination_path.clone(), self.active_document_id.unwrap());
						responses.push_front(
							DocumentOperation::InsertLayer {
								layer: Box::new(entry.layer.clone()),
								destination_path,
								insert_index,
							}
							.into(),
						);
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
							.graphene_document
							.shallowest_common_folder(document.selected_layers())
							.expect("While pasting from serialized, the selected layers did not exist while attempting to find the appropriate folder path for insertion");
						responses.push_back(DocumentMessage::DeselectAllLayers.into());
						responses.push_back(DocumentMessage::StartTransaction.into());

						for entry in data.iter().rev() {
							let destination_path = [shallowest_common_folder.to_vec(), vec![generate_uuid()]].concat();

							responses.push_front(
								DocumentMessage::UpdateLayerMetadata {
									layer_path: destination_path.clone(),
									layer_metadata: entry.layer_metadata,
								}
								.into(),
							);
							document.load_layer_resources(responses, &entry.layer.data, destination_path.clone(), self.active_document_id.unwrap());
							responses.push_front(
								DocumentOperation::InsertLayer {
									layer: Box::new(entry.layer.clone()),
									destination_path,
									insert_index: -1,
								}
								.into(),
							);
						}

						responses.push_back(DocumentMessage::CommitTransaction.into());
					}
				}
			}
			PortfolioMessage::PrevDocument => {
				if let Some(active_document_id) = self.active_document_id {
					let len = self.document_ids.len();
					let current_index = self.document_index(active_document_id);
					let prev_index = (current_index + len - 1) % len;
					let prev_id = self.document_ids[prev_index];
					responses.push_back(PortfolioMessage::SelectDocument { document_id: prev_id }.into());
				}
			}
			PortfolioMessage::ProcessNodeGraphFrame {
				document_id,
				layer_path,
				image_data,
				size,
				imaginate_node,
			} => {
				if let Err(description) = self.evaluate_node_graph(document_id, layer_path, image_data, size, imaginate_node, preferences, responses) {
					responses.push_back(
						DialogMessage::DisplayDialogError {
							title: "Unable to update node graph".to_string(),
							description,
						}
						.into(),
					);
				}
			}
			PortfolioMessage::SelectDocument { document_id } => {
				if let Some(document) = self.active_document() {
					if !document.is_auto_saved() {
						responses.push_back(
							PortfolioMessage::AutoSaveDocument {
								// Safe to unwrap since we know that there is an active document
								document_id: self.active_document_id.unwrap(),
							}
							.into(),
						);
					}
				}

				if self.active_document().is_some() {
					responses.push_back(BroadcastEvent::ToolAbort.into());
				}

				// TODO: Remove this message in favor of having tools have specific data per document instance
				responses.push_back(PortfolioMessage::SetActiveDocument { document_id }.into());
				responses.push_back(PortfolioMessage::UpdateOpenDocumentsList.into());
				responses.push_back(FrontendMessage::UpdateActiveDocument { document_id }.into());
				responses.push_back(DocumentMessage::RenderDocument.into());
				responses.push_back(DocumentMessage::DocumentStructureChanged.into());
				for layer in self.documents.get(&document_id).unwrap().layer_metadata.keys() {
					responses.push_back(DocumentMessage::LayerChanged { affected_layer_path: layer.clone() }.into());
				}
				responses.push_back(BroadcastEvent::SelectionChanged.into());
				responses.push_back(BroadcastEvent::DocumentIsDirty.into());
				responses.push_back(PortfolioMessage::UpdateDocumentWidgets.into());
				responses.push_back(NavigationMessage::TranslateCanvas { delta: (0., 0.).into() }.into());
			}
			PortfolioMessage::SetActiveDocument { document_id } => self.active_document_id = Some(document_id),
			PortfolioMessage::SetImageBlobUrl {
				document_id,
				layer_path,
				blob_url,
				resolution,
			} => {
				let message = DocumentMessage::SetImageBlobUrl {
					layer_path,
					blob_url,
					resolution,
					document_id,
				};
				responses.push_back(PortfolioMessage::DocumentPassMessage { document_id, message }.into());
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
				responses.push_back(FrontendMessage::UpdateOpenDocumentsList { open_documents }.into());
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

	// TODO Fix how this doesn't preserve tab order upon loading new document from *File > Load*
	fn load_document(&mut self, new_document: DocumentMessageHandler, document_id: u64, responses: &mut VecDeque<Message>) {
		self.document_ids.push(document_id);

		responses.extend(
			new_document
				.layer_metadata
				.keys()
				.filter_map(|path| new_document.layer_panel_entry_from_path(path, &self.persistent_data.font_cache))
				.map(|entry| FrontendMessage::UpdateDocumentLayerDetails { data: entry }.into())
				.collect::<Vec<_>>(),
		);
		new_document.update_layer_tree_options_bar_widgets(responses, &self.persistent_data.font_cache);

		self.documents.insert(document_id, new_document);

		if self.active_document().is_some() {
			responses.push_back(PropertiesPanelMessage::Deactivate.into());
			responses.push_back(BroadcastEvent::ToolAbort.into());
			responses.push_back(ToolMessage::DeactivateTools.into());
		}

		responses.push_back(PortfolioMessage::UpdateOpenDocumentsList.into());
		responses.push_back(PortfolioMessage::SelectDocument { document_id }.into());
		responses.push_back(PortfolioMessage::LoadDocumentResources { document_id }.into());
		responses.push_back(PortfolioMessage::UpdateDocumentWidgets.into());
		responses.push_back(ToolMessage::InitTools.into());
		responses.push_back(PropertiesPanelMessage::Init.into());
		responses.push_back(NavigationMessage::TranslateCanvas { delta: (0., 0.).into() }.into());
		responses.push_back(DocumentMessage::DocumentStructureChanged.into());
	}

	/// Returns an iterator over the open documents in order.
	pub fn ordered_document_iterator(&self) -> impl Iterator<Item = &DocumentMessageHandler> {
		self.document_ids.iter().map(|id| self.documents.get(id).expect("document id was not found in the document hashmap"))
	}

	fn document_index(&self, document_id: u64) -> usize {
		self.document_ids.iter().position(|id| id == &document_id).expect("Active document is missing from document ids")
	}

	fn compute_input<T: dyn_any::StaticType>(old_network: &NodeNetwork, node_path: &[NodeId], mut input_index: usize, image: Cow<Image>) -> Result<T, String> {
		let mut network = old_network.clone();
		// Adjust the output of the graph so we find the relevant output
		'outer: for end in (0..node_path.len()).rev() {
			let mut inner_network = &mut network;
			for index in 0..=end {
				let node_id = node_path[index];
				inner_network.output = node_id;

				if index != end {
					if let DocumentNodeImplementation::Network(n) = &mut inner_network.nodes.get_mut(&node_id).ok_or_else(|| "Invalid node path".to_string())?.implementation {
						inner_network = n;
					} else {
						return Err("Path incomplete".to_string());
					}
				}
			}
			match &inner_network.nodes.get(&node_path[end]).unwrap().inputs[input_index] {
				// If the input is from a parent network then adjust the input index and continue iteration
				NodeInput::Network => {
					error!("Input is network!!!");
					input_index = inner_network
						.inputs
						.iter()
						.enumerate()
						.filter(|&(_index, &id)| id == node_path[end])
						.nth(input_index)
						.ok_or_else(|| "Invalid network input".to_string())?
						.0;
				}
				// If the input is just a value, return that value
				NodeInput::Value { tagged_value, .. } => {
					return dyn_any::downcast::<T>(tagged_value.clone().to_value().up_box())
						.map(|v| *v)
						.ok_or_else(|| "Incorrectly typed value".to_string())
				}
				// If the input is from a node, set the node to be the output (so that is what is evaluated)
				NodeInput::Node(n) => {
					inner_network.output = *n;
					break 'outer;
				}
			}
		}

		let stack = borrow_stack::FixedSizeStack::new(256);
		for node_id in old_network.nodes.keys() {
			network.flatten(*node_id);
		}

		let mut proto_network = network.into_proto_network();
		proto_network.reorder_ids();

		info!("proto_network with reordered ids: {proto_network:#?}");

		assert_ne!(proto_network.nodes.len(), 0, "No protonodes exist?");
		for (_id, node) in proto_network.nodes {
			info!("Inserting proto node {:?}", node);
			interpreted_executor::node_registry::push_node(node, &stack);
		}

		use borrow_stack::BorrowStack;
		use dyn_any::IntoDynAny;
		use graphene_core::Node;

		let boxed = unsafe { stack.get().last().unwrap().eval(image.into_owned().into_dyn()) };

		dyn_any::downcast::<T>(boxed).map(|v| *v).ok_or_else(|| "Incorrectly typed output".to_string())
	}

	fn encode_img(image: Image, resize: bool, format: image::ImageOutputFormat) -> Result<(Vec<u8>, (u32, u32)), String> {
		use image::{ImageBuffer, Rgba};
		use std::io::Cursor;

		let mut image_data: Vec<u8> = Vec::new();
		let [image_width, image_height] = [image.width, image.height];
		let size_estimate = (image_width * image_height * 4) as usize;

		let mut result_bytes = Vec::with_capacity(size_estimate);
		result_bytes.extend(image.data.into_iter().flat_map(|colour| colour.to_rgba8()));
		let mut output: ImageBuffer<Rgba<u8>, _> = image::ImageBuffer::from_raw(image_width, image_height, result_bytes).ok_or_else(|| "Invalid image size".to_string())?;
		if resize {
			let (new_width, new_height) = pick_safe_imaginate_resolution((image_width as f64, image_height as f64));
			output = image::imageops::resize(&output, new_width as u32, new_height as u32, image::imageops::Triangle);
		}
		let size = output.dimensions();
		output.write_to(&mut Cursor::new(&mut image_data), format).map_err(|e| e.to_string())?;
		Ok::<_, String>((image_data, size))
	}

	fn evaluate_node_graph(
		&mut self,
		document_id: u64,
		layer_path: Vec<LayerId>,
		image_data: Vec<u8>,
		size: (u32, u32),
		imaginate_node: Option<Vec<NodeId>>,
		preferences: &PreferencesMessageHandler,
		responses: &mut VecDeque<Message>,
	) -> Result<(), String> {
		let data = image_data.chunks_exact(4).map(|v| graphene_core::raster::color::Color::from_rgba8(v[0], v[1], v[2], v[3])).collect();
		let (width, height) = size;
		let image = graphene_core::raster::Image { width, height, data };

		let document = self.documents.get_mut(&document_id).ok_or_else(|| "Invalid document".to_string())?;
		let layer = document.graphene_document.layer(&layer_path).map_err(|e| format!("No layer: {e:?}"))?;
		let node_graph_frame = match &layer.data {
			LayerDataType::NodeGraphFrame(frame) => Ok(frame),
			_ => Err("Invalid layer type".to_string()),
		}?;
		let network = node_graph_frame.network.clone();

		// Execute the node graph

		if let Some(imaginate_node) = imaginate_node {
			use graph_craft::imaginate_input::*;

			let node_type = resolve_document_node_type("Imaginate").expect("Imaginate in node library");
			let get = |name: &str| node_type.inputs.iter().position(|input| input.name == name).unwrap_or_else(|| panic!("Input {name} not found"));

			let resolution: glam::DVec2 = Self::compute_input(&network, &imaginate_node, get("Resolution"), Cow::Borrowed(&image))?;

			let transform = document.graphene_document.root.transform.inverse() * document.graphene_document.multiply_transforms(&layer_path).unwrap();
			let parameters = ImaginateGenerationParameters {
				seed: Self::compute_input::<f64>(&network, &imaginate_node, get("Seed"), Cow::Borrowed(&image))? as u64,
				resolution: resolution.as_uvec2().into(),
				samples: Self::compute_input::<f64>(&network, &imaginate_node, get("Samples"), Cow::Borrowed(&image))? as u32,
				sampling_method: Self::compute_input::<ImaginateSamplingMethod>(&network, &imaginate_node, get("Sampling Method"), Cow::Borrowed(&image))?
					.api_value()
					.to_string(),
				text_guidance: Self::compute_input(&network, &imaginate_node, get("Text Guidance"), Cow::Borrowed(&image))?,
				text_prompt: Self::compute_input(&network, &imaginate_node, get("Text Prompt"), Cow::Borrowed(&image))?,
				negative_prompt: Self::compute_input(&network, &imaginate_node, get("Neg. Prompt"), Cow::Borrowed(&image))?,
				image_creativity: Some(Self::compute_input(&network, &imaginate_node, get("Image Creativity"), Cow::Borrowed(&image))?),
				restore_faces: Self::compute_input(&network, &imaginate_node, get("Improve Faces"), Cow::Borrowed(&image))?,
				tiling: Self::compute_input(&network, &imaginate_node, get("Tiling"), Cow::Borrowed(&image))?,
			};
			let use_base_image = Self::compute_input::<bool>(&network, &imaginate_node, get("Use Base Image"), Cow::Borrowed(&image))?;

			let base_image = if use_base_image {
				let image: Image = Self::compute_input(&network, &imaginate_node, get("Base Image"), Cow::Borrowed(&image))?;
				if image.width > 0 {
					let (image_data, size) = Self::encode_img(image, false, image::ImageOutputFormat::Png)?;
					let size = DVec2::new(size.0 as f64, size.1 as f64);
					let mime = "image/png".to_string();
					Some(ImaginateBaseImage { image_data, size, mime })
				} else {
					None
				}
			} else {
				None
			};

			let mask_image =
				if base_image.is_some() {
					let mask: Option<Vec<LayerId>> = Self::compute_input(&network, &imaginate_node, get("Masking Layer"), Cow::Borrowed(&image))?;

					// Calculate the size of the region to be exported
					let size = DVec2::new(transform.transform_vector2(DVec2::new(1., 0.)).length(), transform.transform_vector2(DVec2::new(0., 1.)).length());

					let old_transforms = document.remove_document_transform();

					let mask_is_some = mask.is_some();
					let mask_image = mask.filter(|mask_layer_path| document.graphene_document.layer(mask_layer_path).is_ok()).map(|mask_layer_path| {
						let svg = document.render_document(
							size,
							transform.inverse(),
							&self.persistent_data,
							DocumentRenderMode::LayerCutout(&mask_layer_path, graphene::color::Color::WHITE),
						);

						ImaginateMaskImage { svg, size }
					});

					if mask_is_some && mask_image.is_none() {
						return Err("Imagination masking layer is missing.\nIt may have been deleted or moved. Please drag a new layer reference\ninto the 'Masking Layer' parameter input, then generate again.".to_string());
					}

					document.restore_document_transform(old_transforms);
					mask_image
				} else {
					None
				};

			responses.push_back(
				FrontendMessage::TriggerImaginateGenerate {
					parameters,
					base_image,
					mask_image,
					mask_paint_mode: if Self::compute_input::<bool>(&network, &imaginate_node, get("Inpaint"), Cow::Borrowed(&image))? {
						ImaginateMaskPaintMode::Inpaint
					} else {
						ImaginateMaskPaintMode::Outpaint
					},
					mask_blur_px: Self::compute_input::<f64>(&network, &imaginate_node, get("Mask Blur"), Cow::Borrowed(&image))? as u32,
					imaginate_mask_starting_fill: Self::compute_input(&network, &imaginate_node, get("Mask Starting Fill"), Cow::Borrowed(&image))?,
					hostname: preferences.imaginate_server_hostname.clone(),
					refresh_frequency: preferences.imaginate_refresh_frequency,
					document_id,
					layer_path,
				}
				.into(),
			);
		} else {
			let image: Image = Self::compute_input(&network, &[1], 0, Cow::Owned(image))?;
			let (image_data, _size) = Self::encode_img(image, false, image::ImageOutputFormat::Bmp)?;

			responses.push_back(
				DocumentOperation::SetNodeGraphFrameImageData {
					layer_path: layer_path.clone(),
					image_data: image_data.clone(),
				}
				.into(),
			);
			let mime = "image/bmp".to_string();
			let image_data = std::sync::Arc::new(image_data);
			responses.push_back(
				FrontendMessage::UpdateImageData {
					document_id,
					image_data: vec![FrontendImageData { path: layer_path, image_data, mime }],
				}
				.into(),
			);
		}

		Ok(())
	}
}

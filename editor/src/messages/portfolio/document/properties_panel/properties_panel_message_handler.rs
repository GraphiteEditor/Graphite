use super::utility_functions::{register_artboard_layer_properties, register_artwork_layer_properties};
use super::utility_types::PropertiesPanelMessageHandlerData;
use crate::messages::layout::utility_types::layout_widget::{Layout, WidgetLayout};
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::portfolio::document::properties_panel::utility_functions::apply_transform_operation;
use crate::messages::portfolio::document::utility_types::misc::TargetDocument;
use crate::messages::portfolio::utility_types::PersistentData;
use crate::messages::prelude::*;

use document_legacy::layers::layer_info::LayerDataTypeDiscriminant;
use document_legacy::{LayerId, Operation};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PropertiesPanelMessageHandler {
	active_selection: Option<(Vec<LayerId>, TargetDocument)>,
}

impl<'a> MessageHandler<PropertiesPanelMessage, (&PersistentData, PropertiesPanelMessageHandlerData<'a>)> for PropertiesPanelMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: PropertiesPanelMessage, (persistent_data, data): (&PersistentData, PropertiesPanelMessageHandlerData), responses: &mut VecDeque<Message>) {
		let PropertiesPanelMessageHandlerData {
			artwork_document,
			artboard_document,
			selected_layers,
			node_graph_message_handler,
		} = data;
		let get_document = |document_selector: TargetDocument| match document_selector {
			TargetDocument::Artboard => artboard_document,
			TargetDocument::Artwork => artwork_document,
		};
		use PropertiesPanelMessage::*;
		match message {
			SetActiveLayers { paths, document } => {
				if paths.len() != 1 {
					// TODO: Allow for multiple selected layers
					responses.push_back(PropertiesPanelMessage::ClearSelection.into());
					responses.push_back(NodeGraphMessage::CloseNodeGraph.into());
				} else {
					let path = paths.into_iter().next().unwrap();
					if Some((path.clone(), document)) != self.active_selection {
						// Update the node graph frame visibility
						if get_document(document)
							.layer(&path)
							.ok()
							.filter(|layer| LayerDataTypeDiscriminant::from(&layer.data) == LayerDataTypeDiscriminant::NodeGraphFrame)
							.is_some()
						{
							responses.push_back(NodeGraphMessage::OpenNodeGraph { layer_path: path.clone() }.into());
						} else {
							responses.push_back(NodeGraphMessage::CloseNodeGraph.into());
						}

						self.active_selection = Some((path, document));
						responses.push_back(PropertiesPanelMessage::ResendActiveProperties.into());
					}
				}
			}
			ClearSelection => {
				responses.push_back(
					LayoutMessage::SendLayout {
						layout: Layout::WidgetLayout(WidgetLayout::new(vec![])),
						layout_target: LayoutTarget::PropertiesOptions,
					}
					.into(),
				);
				responses.push_back(
					LayoutMessage::SendLayout {
						layout: Layout::WidgetLayout(WidgetLayout::new(vec![])),
						layout_target: LayoutTarget::PropertiesSections,
					}
					.into(),
				);
				responses.push_back(NodeGraphMessage::CloseNodeGraph.into());
				self.active_selection = None;
			}
			Deactivate => responses.push_back(
				BroadcastMessage::UnsubscribeEvent {
					on: BroadcastEvent::SelectionChanged,
					message: Box::new(PropertiesPanelMessage::UpdateSelectedDocumentProperties.into()),
				}
				.into(),
			),
			Init => responses.push_back(
				BroadcastMessage::SubscribeEvent {
					on: BroadcastEvent::SelectionChanged,
					send: Box::new(PropertiesPanelMessage::UpdateSelectedDocumentProperties.into()),
				}
				.into(),
			),
			ModifyFont { font_family, font_style, size } => {
				let (path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");

				self.create_document_operation(Operation::ModifyFont { path, font_family, font_style, size }, true, responses);
				responses.push_back(ResendActiveProperties.into());
			}
			ModifyTransform { value, transform_op } => {
				let (path, target_document) = self.active_selection.as_ref().expect("Received update for properties panel with no active layer");
				let layer = get_document(*target_document).layer(path).unwrap();

				let transform = apply_transform_operation(layer, transform_op, value, &persistent_data.font_cache);

				self.create_document_operation(Operation::SetLayerTransform { path: path.clone(), transform }, true, responses);
			}
			ModifyName { name } => {
				let (path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				self.create_document_operation(Operation::SetLayerName { path, name }, true, responses);
			}
			ModifyPreserveAspect { preserve_aspect } => {
				let (layer_path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				self.create_document_operation(Operation::SetLayerPreserveAspect { layer_path, preserve_aspect }, true, responses);
			}
			ModifyFill { fill } => {
				let (path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				self.create_document_operation(Operation::SetLayerFill { path, fill }, true, responses);
			}
			ModifyStroke { stroke } => {
				let (path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				self.create_document_operation(Operation::SetLayerStroke { path, stroke }, true, responses);
			}
			ModifyText { new_text } => {
				let (path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				self.create_document_operation(Operation::SetTextContent { path, new_text }, true, responses);
			}
			SetPivot { new_position } => {
				let (layer_path, _) = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				let position: Option<glam::DVec2> = new_position.into();
				let pivot = position.unwrap().into();

				responses.push_back(DocumentMessage::StartTransaction.into());
				responses.push_back(Operation::SetPivot { layer_path, pivot }.into());
			}
			CheckSelectedWasUpdated { path } => {
				if self.matches_selected(&path) {
					responses.push_back(PropertiesPanelMessage::ResendActiveProperties.into())
				}
			}
			CheckSelectedWasDeleted { path } => {
				if self.matches_selected(&path) {
					self.active_selection = None;
					responses.push_back(
						LayoutMessage::SendLayout {
							layout_target: LayoutTarget::PropertiesOptions,
							layout: Layout::WidgetLayout(WidgetLayout::default()),
						}
						.into(),
					);
					responses.push_back(
						LayoutMessage::SendLayout {
							layout_target: LayoutTarget::PropertiesSections,
							layout: Layout::WidgetLayout(WidgetLayout::default()),
						}
						.into(),
					);
					responses.push_back(NodeGraphMessage::CloseNodeGraph.into());
				}
			}
			ResendActiveProperties => {
				if let Some((path, target_document)) = self.active_selection.clone() {
					let document = get_document(target_document);
					let layer = document.layer(&path).unwrap();
					match target_document {
						TargetDocument::Artboard => register_artboard_layer_properties(layer, responses, persistent_data),
						TargetDocument::Artwork => register_artwork_layer_properties(document, path, layer, responses, persistent_data, node_graph_message_handler),
					}
				}
			}
			UpdateSelectedDocumentProperties => responses.push_back(
				PropertiesPanelMessage::SetActiveLayers {
					paths: selected_layers.map(|path| path.to_vec()).collect(),
					document: TargetDocument::Artwork,
				}
				.into(),
			),
		}
	}

	fn actions(&self) -> ActionList {
		actions!(PropertiesMessageDiscriminant;)
	}
}

impl PropertiesPanelMessageHandler {
	fn matches_selected(&self, path: &[LayerId]) -> bool {
		let last_active_path_id = self.active_selection.as_ref().and_then(|(v, _)| v.last().copied());
		let last_modified = path.last().copied();
		matches!((last_active_path_id, last_modified), (Some(active_last), Some(modified_last)) if active_last == modified_last)
	}

	fn create_document_operation(&self, operation: Operation, commit_history: bool, responses: &mut VecDeque<Message>) {
		let (_, target_document) = self.active_selection.as_ref().unwrap();
		match *target_document {
			TargetDocument::Artboard => {
				// Commit history is not respected as the artboard document is not saved in the history system.

				// Dispatch the relevant operation to the artboard document
				responses.push_back(ArtboardMessage::DispatchOperation(Box::new(operation)).into())
			}
			TargetDocument::Artwork => {
				// Commit to history before the modification
				if commit_history {
					responses.push_back(DocumentMessage::StartTransaction.into());
				}

				// Dispatch the relevant operation to the main document
				responses.push_back(DocumentMessage::DispatchOperation(Box::new(operation)).into());
			}
		}
	}
}

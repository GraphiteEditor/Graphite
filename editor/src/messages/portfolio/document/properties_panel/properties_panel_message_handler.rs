use super::utility_functions::{register_artwork_layer_properties, register_document_graph_properties};
use super::utility_types::PropertiesPanelMessageHandlerData;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::properties_panel::utility_functions::apply_transform_operation;
use crate::messages::portfolio::utility_types::PersistentData;
use crate::messages::prelude::*;

use document_legacy::layers::layer_info::LayerDataTypeDiscriminant;
use document_legacy::layers::style::{RenderData, ViewMode};
use document_legacy::{LayerId, Operation};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PropertiesPanelMessageHandler {
	active_selection: Option<Vec<LayerId>>, // TODO: Delete this if it's indeed dead code?
}

impl<'a> MessageHandler<PropertiesPanelMessage, (&PersistentData, PropertiesPanelMessageHandlerData<'a>)> for PropertiesPanelMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: PropertiesPanelMessage, responses: &mut VecDeque<Message>, (persistent_data, data): (&PersistentData, PropertiesPanelMessageHandlerData)) {
		use PropertiesPanelMessage::*;

		let PropertiesPanelMessageHandlerData {
			document_name,
			artwork_document,
			selected_layers,
			node_graph_message_handler,
			executor,
		} = data;
		let render_data = RenderData::new(&persistent_data.font_cache, ViewMode::Normal, None);

		match message {
			SetActiveLayers { paths } => {
				if paths.len() != 1 {
					// TODO: Allow for multiple selected layers
					responses.add(PropertiesPanelMessage::ClearSelection);
					responses.add(NodeGraphMessage::CloseNodeGraph);
				} else {
					let path = paths.into_iter().next().unwrap();
					if self.active_selection.as_ref() != Some(&path) {
						// Update the layer visibility
						if artwork_document
							.layer(&path)
							.ok()
							.filter(|layer| LayerDataTypeDiscriminant::from(&layer.data) == LayerDataTypeDiscriminant::Layer)
							.is_some()
						{
							responses.add(NodeGraphMessage::OpenNodeGraph { layer_path: path.clone() });
						} else {
							responses.add(NodeGraphMessage::CloseNodeGraph);
						}

						self.active_selection = Some(path);
						responses.add(PropertiesPanelMessage::ResendActiveProperties);
					}
				}
			}
			ClearSelection => {
				// This causes the Properties panel to change, so this needs to happen before the following lines clear the Properties panel
				responses.add(NodeGraphMessage::CloseNodeGraph);

				responses.add(LayoutMessage::SendLayout {
					layout: Layout::WidgetLayout(WidgetLayout::new(vec![])),
					layout_target: LayoutTarget::PropertiesOptions,
				});
				responses.add(LayoutMessage::SendLayout {
					layout: Layout::WidgetLayout(WidgetLayout::new(vec![])),
					layout_target: LayoutTarget::PropertiesSections,
				});
				self.active_selection = None;
			}
			Deactivate => responses.add(BroadcastMessage::UnsubscribeEvent {
				on: BroadcastEvent::SelectionChanged,
				message: Box::new(PropertiesPanelMessage::UpdateSelectedDocumentProperties.into()),
			}),
			Init => responses.add(BroadcastMessage::SubscribeEvent {
				on: BroadcastEvent::SelectionChanged,
				send: Box::new(PropertiesPanelMessage::UpdateSelectedDocumentProperties.into()),
			}),
			ModifyTransform { value, transform_op } => {
				let path = self.active_selection.as_ref().expect("Received update for properties panel with no active layer");
				let layer = artwork_document.layer(path).unwrap();

				let transform = apply_transform_operation(layer, transform_op, value, &render_data);

				self.create_document_operation(Operation::SetLayerTransform { path: path.clone(), transform }, true, responses);
			}
			ModifyPreserveAspect { preserve_aspect } => {
				let layer_path = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				self.create_document_operation(Operation::SetLayerPreserveAspect { layer_path, preserve_aspect }, true, responses);
			}
			SetPivot { new_position } => {
				let layer = self.active_selection.clone().expect("Received update for properties panel with no active layer");
				let position: Option<glam::DVec2> = new_position.into();
				let pivot = position.unwrap();

				responses.add(DocumentMessage::StartTransaction);
				responses.add(GraphOperationMessage::TransformSetPivot { layer, pivot });
			}
			CheckSelectedWasUpdated { path } => {
				if self.matches_selected(&path) {
					responses.add(PropertiesPanelMessage::ResendActiveProperties)
				}
			}
			CheckSelectedWasDeleted { path } => {
				if self.matches_selected(&path) {
					self.active_selection = None;
					responses.add(LayoutMessage::SendLayout {
						layout_target: LayoutTarget::PropertiesOptions,
						layout: Layout::WidgetLayout(WidgetLayout::default()),
					});
					responses.add(LayoutMessage::SendLayout {
						layout_target: LayoutTarget::PropertiesSections,
						layout: Layout::WidgetLayout(WidgetLayout::default()),
					});
					responses.add(NodeGraphMessage::CloseNodeGraph);
				}
			}
			ResendActiveProperties => {
				if let Some(path) = self.active_selection.clone() {
					// TODO: Remove this conditional now that the document graph is the only form of graph? (Also any other related code.)
					let layer = artwork_document.layer(&path).unwrap();
					register_artwork_layer_properties(artwork_document, path, layer, responses, persistent_data, node_graph_message_handler, executor);
				} else {
					let context = crate::messages::portfolio::document::node_graph::NodePropertiesContext {
						persistent_data,
						document: artwork_document,
						responses,
						nested_path: &node_graph_message_handler.network,
						layer_path: &[],
						executor,
						network: &artwork_document.document_network,
					};
					register_document_graph_properties(context, node_graph_message_handler, document_name);
				}
			}
			UpdateSelectedDocumentProperties => responses.add(PropertiesPanelMessage::SetActiveLayers {
				paths: selected_layers.map(|path| path.to_vec()).collect(),
			}),
		}
	}

	fn actions(&self) -> ActionList {
		actions!(PropertiesMessageDiscriminant;)
	}
}

impl PropertiesPanelMessageHandler {
	fn matches_selected(&self, path: &[LayerId]) -> bool {
		let last_active_path_id = self.active_selection.as_ref().and_then(|v| v.last().copied());
		let last_modified = path.last().copied();
		matches!((last_active_path_id, last_modified), (Some(active_last), Some(modified_last)) if active_last == modified_last)
	}

	fn create_document_operation(&self, operation: Operation, commit_history: bool, responses: &mut VecDeque<Message>) {
		// Commit to history before the modification
		if commit_history {
			responses.add(DocumentMessage::StartTransaction);
		}

		// Dispatch the relevant operation to the main document
		responses.add(DocumentMessage::DispatchOperation(Box::new(operation)));
	}
}

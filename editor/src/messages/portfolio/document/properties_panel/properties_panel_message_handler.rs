use super::utility_types::PropertiesPanelMessageHandlerData;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::node_graph::NodePropertiesContext;
use crate::messages::portfolio::utility_types::PersistentData;
use crate::messages::prelude::*;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PropertiesPanelMessageHandler;

impl<'a> MessageHandler<PropertiesPanelMessage, (&PersistentData, PropertiesPanelMessageHandlerData<'a>)> for PropertiesPanelMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: PropertiesPanelMessage, responses: &mut VecDeque<Message>, (persistent_data, data): (&PersistentData, PropertiesPanelMessageHandlerData)) {
		use PropertiesPanelMessage::*;

		let PropertiesPanelMessageHandlerData {
			node_graph_message_handler,
			executor,
			document_network,
			document_metadata,
			document_name,
			..
		} = data;

		match message {
			Clear => {
				responses.add(LayoutMessage::SendLayout {
					layout: Layout::WidgetLayout(WidgetLayout::new(vec![])),
					layout_target: LayoutTarget::PropertiesOptions,
				});
				responses.add(LayoutMessage::SendLayout {
					layout: Layout::WidgetLayout(WidgetLayout::new(vec![])),
					layout_target: LayoutTarget::PropertiesSections,
				});
			}
			Refresh => {
				let mut context = NodePropertiesContext {
					persistent_data,
					responses,
					nested_path: &node_graph_message_handler.network,
					layer_path: &[],
					executor,
					network: document_network,
					metadata: document_metadata,
				};

				let properties_sections = node_graph_message_handler.collate_properties(&mut context);

				let options_bar = vec![LayoutGroup::Row {
					widgets: vec![
						IconLabel::new("File").tooltip("Document").widget_holder(),
						Separator::new(SeparatorType::Unrelated).widget_holder(),
						TextInput::new(document_name)
							.on_update(|text_input| DocumentMessage::RenameDocument { new_name: text_input.value.clone() }.into())
							.widget_holder(),
						Separator::new(SeparatorType::Related).widget_holder(),
						PopoverButton::new("Additional Options", "Coming soon").widget_holder(),
					],
				}];

				context.responses.add(LayoutMessage::SendLayout {
					layout: Layout::WidgetLayout(WidgetLayout::new(options_bar)),
					layout_target: LayoutTarget::PropertiesOptions,
				});
				context.responses.add(LayoutMessage::SendLayout {
					layout: Layout::WidgetLayout(WidgetLayout::new(properties_sections)),
					layout_target: LayoutTarget::PropertiesSections,
				});
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(PropertiesMessageDiscriminant;)
	}
}

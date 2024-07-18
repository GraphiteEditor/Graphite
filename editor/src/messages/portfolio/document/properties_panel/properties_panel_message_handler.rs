use super::utility_types::PropertiesPanelMessageHandlerData;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::node_graph::document_node_types::NodePropertiesContext;
use crate::messages::portfolio::utility_types::PersistentData;
use crate::messages::prelude::*;

#[derive(Debug, Clone, Default)]
pub struct PropertiesPanelMessageHandler {}

impl<'a> MessageHandler<PropertiesPanelMessage, (&PersistentData, PropertiesPanelMessageHandlerData<'a>)> for PropertiesPanelMessageHandler {
	fn process_message(&mut self, message: PropertiesPanelMessage, responses: &mut VecDeque<Message>, (persistent_data, data): (&PersistentData, PropertiesPanelMessageHandlerData)) {
		let PropertiesPanelMessageHandlerData {
			network_interface,
			selection_path,
			document_name,
			executor,
		} = data;

		match message {
			PropertiesPanelMessage::Clear => {
				responses.add(LayoutMessage::SendLayout {
					layout: Layout::WidgetLayout(WidgetLayout::new(vec![])),
					layout_target: LayoutTarget::PropertiesOptions,
				});
				responses.add(LayoutMessage::SendLayout {
					layout: Layout::WidgetLayout(WidgetLayout::new(vec![])),
					layout_target: LayoutTarget::PropertiesSections,
				});
			}
			PropertiesPanelMessage::Refresh => {
				let mut context = NodePropertiesContext {
					persistent_data,
					responses,
					executor,
					network_interface,
					selection_network_path: selection_path,
				};

				let properties_sections = NodeGraphMessageHandler::collate_properties(&mut context);

				let options_bar = vec![LayoutGroup::Row {
					widgets: vec![
						IconLabel::new("File").tooltip("Document name").widget_holder(),
						Separator::new(SeparatorType::Related).widget_holder(),
						TextInput::new(document_name)
							.on_update(|text_input| DocumentMessage::RenameDocument { new_name: text_input.value.clone() }.into())
							.widget_holder(),
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

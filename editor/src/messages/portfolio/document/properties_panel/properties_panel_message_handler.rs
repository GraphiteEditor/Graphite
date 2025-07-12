use graphene_std::uuid::NodeId;

use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::node_graph::document_node_definitions::NodePropertiesContext;
use crate::messages::portfolio::utility_types::PersistentData;
use crate::messages::prelude::*;

use crate::messages::portfolio::document::utility_types::network_interface::NodeNetworkInterface;
pub struct PropertiesPanelMessageHandlerData<'a> {
	pub network_interface: &'a mut NodeNetworkInterface,
	pub selection_network_path: &'a [NodeId],
	pub document_name: &'a str,
}

#[derive(Debug, Clone, Default, ExtractField)]
pub struct PropertiesPanelMessageHandler {}

#[message_handler_data]
impl MessageHandler<PropertiesPanelMessage, (&PersistentData, PropertiesPanelMessageHandlerData<'_>)> for PropertiesPanelMessageHandler {
	fn process_message(&mut self, message: PropertiesPanelMessage, responses: &mut VecDeque<Message>, (persistent_data, data): (&PersistentData, PropertiesPanelMessageHandlerData)) {
		let PropertiesPanelMessageHandlerData {
			network_interface,
			selection_network_path,
			document_name,
		} = data;

		match message {
			PropertiesPanelMessage::Clear => {
				responses.add(LayoutMessage::SendLayout {
					layout: Layout::WidgetLayout(WidgetLayout::new(vec![])),
					layout_target: LayoutTarget::PropertiesSections,
				});
			}
			PropertiesPanelMessage::Refresh => {
				let mut context = NodePropertiesContext {
					persistent_data,
					responses,
					network_interface,
					selection_network_path,
					document_name,
				};
				let properties_sections = NodeGraphMessageHandler::collate_properties(&mut context);

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

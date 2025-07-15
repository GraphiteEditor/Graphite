use graphene_std::uuid::NodeId;

use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::node_graph::document_node_definitions::NodePropertiesContext;
use crate::messages::portfolio::document::utility_types::network_interface::NodeNetworkInterface;
use crate::messages::portfolio::utility_types::PersistentData;
use crate::messages::prelude::*;
use crate::node_graph_executor::NodeGraphExecutor;

#[derive(ExtractField)]
pub struct PropertiesPanelMessageContext<'a> {
	pub network_interface: &'a mut NodeNetworkInterface,
	pub selection_network_path: &'a [NodeId],
	pub document_name: &'a str,
	pub executor: &'a mut NodeGraphExecutor,
	pub persistent_data: &'a PersistentData,
}

#[derive(Debug, Clone, Default, ExtractField)]
pub struct PropertiesPanelMessageHandler {}

#[message_handler_data]
impl MessageHandler<PropertiesPanelMessage, PropertiesPanelMessageContext<'_>> for PropertiesPanelMessageHandler {
	fn process_message(&mut self, message: PropertiesPanelMessage, responses: &mut VecDeque<Message>, context: PropertiesPanelMessageContext) {
		let PropertiesPanelMessageContext {
			network_interface,
			selection_network_path,
			document_name,
			executor,
			persistent_data,
		} = context;

		match message {
			PropertiesPanelMessage::Clear => {
				responses.add(LayoutMessage::SendLayout {
					layout: Layout::WidgetLayout(WidgetLayout::new(vec![])),
					layout_target: LayoutTarget::PropertiesSections,
				});
			}
			PropertiesPanelMessage::Refresh => {
				let mut node_properties_context = NodePropertiesContext {
					persistent_data,
					responses,
					network_interface,
					selection_network_path,
					document_name,
					executor,
				};
				let properties_sections = NodeGraphMessageHandler::collate_properties(&mut node_properties_context);

				node_properties_context.responses.add(LayoutMessage::SendLayout {
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

use graphene_std::uuid::NodeId;

use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::node_graph::document_node_definitions::NodePropertiesContext;
use crate::messages::portfolio::document::utility_types::network_interface::NodeNetworkInterface;
use crate::messages::prelude::*;
use crate::node_graph_executor::NodeGraphExecutor;

#[derive(ExtractField)]
pub struct PropertiesPanelMessageContext<'a> {
	pub executor: &'a mut NodeGraphExecutor,
	pub network_interface: &'a mut NodeNetworkInterface,
	pub resources: &'a ResourceMessageHandler,
	pub selection_network_path: &'a [NodeId],
	pub document_name: &'a str,
	pub fonts: &'a FontsMessageHandler,
	pub properties_panel_open: bool,
	pub properties_panel_collapsed_sections: &'a [NodeId],
}

#[derive(Debug, Clone, Default, ExtractField)]
pub struct PropertiesPanelMessageHandler {}

#[message_handler_data]
impl MessageHandler<PropertiesPanelMessage, PropertiesPanelMessageContext<'_>> for PropertiesPanelMessageHandler {
	fn process_message(&mut self, message: PropertiesPanelMessage, responses: &mut VecDeque<Message>, context: PropertiesPanelMessageContext) {
		let PropertiesPanelMessageContext {
			executor,
			network_interface,
			resources,
			selection_network_path,
			document_name,
			fonts,
			properties_panel_open,
			properties_panel_collapsed_sections,
		} = context;

		match message {
			PropertiesPanelMessage::Clear => {
				responses.add(LayoutMessage::SendLayout {
					layout: Layout::default(),
					layout_target: LayoutTarget::PropertiesPanel,
				});
			}
			PropertiesPanelMessage::Refresh => {
				if !properties_panel_open {
					responses.add(PropertiesPanelMessage::Clear);
					return;
				}

				let mut node_properties_context = NodePropertiesContext {
					responses,
					executor,
					network_interface,
					resources,
					selection_network_path,
					document_name,
					fonts,
					properties_panel_collapsed_sections,
				};
				let layout = Layout(NodeGraphMessageHandler::collate_properties(&mut node_properties_context));

				node_properties_context.responses.add(LayoutMessage::SendLayout {
					layout,
					layout_target: LayoutTarget::PropertiesPanel,
				});
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(PropertiesMessageDiscriminant;)
	}
}

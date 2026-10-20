use graphene_std::uuid::NodeId;

use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::node_graph::document_node_definitions::NodeInspectorContext;
use crate::messages::portfolio::document::utility_types::network_interface::NodeNetworkInterface;
use crate::messages::prelude::*;
use crate::node_graph_executor::NodeGraphExecutor;

#[derive(ExtractField)]
pub struct InspectorMessageContext<'a> {
	pub executor: &'a mut NodeGraphExecutor,
	pub document_id: DocumentId,
	pub network_interface: &'a mut NodeNetworkInterface,
	pub resources: &'a ResourceMessageHandler,
	pub selection_network_path: &'a [NodeId],
	pub fonts: &'a FontsMessageHandler,
	pub inspector_open: bool,
	pub inspector_collapsed_sections: &'a [NodeId],
	pub inspector_expanded_input_chains: &'a [(NodeId, Option<usize>)],
}

#[derive(Debug, Clone, Default, ExtractField)]
pub struct InspectorMessageHandler {}

#[message_handler_data]
impl MessageHandler<InspectorMessage, InspectorMessageContext<'_>> for InspectorMessageHandler {
	fn process_message(&mut self, message: InspectorMessage, responses: &mut VecDeque<Message>, context: InspectorMessageContext) {
		let InspectorMessageContext {
			executor,
			document_id,
			network_interface,
			resources,
			selection_network_path,
			fonts,
			inspector_open,
			inspector_collapsed_sections,
			inspector_expanded_input_chains,
		} = context;

		match message {
			InspectorMessage::Clear => {
				responses.add(LayoutMessage::SendLayout {
					layout: Layout::default(),
					layout_target: LayoutTarget::Inspector,
				});
			}
			InspectorMessage::Refresh => {
				if !inspector_open {
					responses.add(InspectorMessage::Clear);
					return;
				}

				let mut node_inspector_context = NodeInspectorContext {
					responses,
					executor,
					document_id,
					network_interface,
					resources,
					selection_network_path,
					fonts,
					inspector_collapsed_sections,
					inspector_expanded_input_chains,
				};
				let layout = Layout(NodeGraphMessageHandler::collate_inspector_sections(&mut node_inspector_context));

				node_inspector_context.responses.add(LayoutMessage::SendLayout {
					layout,
					layout_target: LayoutTarget::Inspector,
				});
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(InspectorMessageDiscriminant;)
	}
}

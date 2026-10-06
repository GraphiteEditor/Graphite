use graphene_std::uuid::NodeId;

use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::node_graph::document_node_definitions::NodePropertiesContext;
use crate::messages::portfolio::document::utility_types::network_interface::NodeNetworkInterface;
use crate::messages::prelude::*;
use crate::node_graph_executor::NodeGraphExecutor;

#[derive(ExtractField)]
pub struct PropertiesPanelMessageContext<'a> {
	pub executor: &'a mut NodeGraphExecutor,
	pub document_id: DocumentId,
	pub network_interface: &'a mut NodeNetworkInterface,
	pub resources: &'a ResourceMessageHandler,
	pub selection_network_path: &'a [NodeId],
	pub document_name: &'a str,
	pub fonts: &'a FontsMessageHandler,
	pub properties_panel_open: bool,
	pub properties_panel_collapsed_sections: &'a [NodeId],
}

#[derive(Debug, Clone, Default, ExtractField)]
pub struct PropertiesPanelMessageHandler {
	/// The node IDs whose sections the last render showed, so bulk toggles know what is currently visible.
	pub shown_section_node_ids: Vec<NodeId>,
}

#[message_handler_data]
impl MessageHandler<PropertiesPanelMessage, PropertiesPanelMessageContext<'_>> for PropertiesPanelMessageHandler {
	fn process_message(&mut self, message: PropertiesPanelMessage, responses: &mut VecDeque<Message>, context: PropertiesPanelMessageContext) {
		let PropertiesPanelMessageContext {
			executor,
			document_id,
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
					document_id,
					network_interface,
					resources,
					selection_network_path,
					document_name,
					fonts,
					properties_panel_collapsed_sections,
				};
				let layout = Layout(NodeGraphMessageHandler::collate_properties(&mut node_properties_context));
				self.shown_section_node_ids = collect_section_node_ids(&layout.0);

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

/// Gathers the node IDs of every section in a Properties panel layout, recursing into nested sections.
fn collect_section_node_ids(groups: &[LayoutGroup]) -> Vec<NodeId> {
	let mut node_ids = Vec::new();
	for group in groups {
		if let LayoutGroup::Section(section) = group {
			node_ids.push(NodeId(section.id));
			node_ids.extend(collect_section_node_ids(&section.layout.0));
		}
	}
	node_ids
}

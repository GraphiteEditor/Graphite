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
	pub properties_panel_open: bool,
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
			properties_panel_open,
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
					persistent_data,
					responses,
					network_interface,
					selection_network_path,
					document_name,
					executor,
				};
				let layout = Layout(NodeGraphMessageHandler::collate_properties(&mut node_properties_context));

				node_properties_context.responses.add(LayoutMessage::SendLayout {
					layout,
					layout_target: LayoutTarget::PropertiesPanel,
				});
			}
			PropertiesPanelMessage::SetAllSectionsExpanded { expanded } => {
				let mut layout = {
					let mut node_properties_context = NodePropertiesContext {
						persistent_data,
						responses,
						network_interface,
						selection_network_path,
						document_name,
						executor,
					};
					Layout(NodeGraphMessageHandler::collate_properties(&mut node_properties_context))
				};

				responses.add(DocumentMessage::AddTransaction);
				let node_ids = Self::update_all_section_expansion_recursive(&mut layout.0, expanded, responses);
				if !node_ids.is_empty() {
					responses.add(NodeGraphMessage::SetLockedOrVisibilitySideEffects { node_ids });
				}

				responses.add(LayoutMessage::SendLayout {
					layout,
					layout_target: LayoutTarget::PropertiesPanel,
				});
			}
			PropertiesPanelMessage::SetSectionExpanded { node_id, expanded } => {
				let node_id = NodeId(node_id);
				responses.add(DocumentMessage::AddTransaction);
				responses.add(NodeGraphMessage::SetCollapsed { node_id, collapsed: !expanded });
				responses.add(NodeGraphMessage::SetLockedOrVisibilitySideEffects { node_ids: vec![node_id] });
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(PropertiesMessageDiscriminant;)
	}
}

impl PropertiesPanelMessageHandler {
	fn update_all_section_expansion_recursive(layout: &mut [LayoutGroup], expanded: bool, responses: &mut VecDeque<Message>) -> Vec<NodeId> {
		let mut node_ids = Vec::new();
		for group in layout {
			if let LayoutGroup::Section {
				id, layout, expanded: group_expanded, ..
			} = group
			{
				*group_expanded = expanded;
				let node_id = NodeId(*id);
				node_ids.push(node_id);
				responses.add(NodeGraphMessage::SetCollapsed { node_id, collapsed: !expanded });
				node_ids.extend(Self::update_all_section_expansion_recursive(&mut layout.0, expanded, responses));
			}
		}
		node_ids
	}
}

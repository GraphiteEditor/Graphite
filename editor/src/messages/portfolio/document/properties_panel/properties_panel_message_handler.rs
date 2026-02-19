use std::collections::HashMap;

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
pub struct PropertiesPanelMessageHandler {
	pub section_expanded: HashMap<u64, bool>,
}

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
					section_expanded: &self.section_expanded,
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
						section_expanded: &self.section_expanded,
					};
					Layout(NodeGraphMessageHandler::collate_properties(&mut node_properties_context))
				};

				Self::update_all_section_expansion_recursive(&mut layout.0, expanded, &mut self.section_expanded);

				responses.add(LayoutMessage::SendLayout {
					layout,
					layout_target: LayoutTarget::PropertiesPanel,
				});
			}
			PropertiesPanelMessage::SetSectionExpanded { node_id, expanded } => {
				self.section_expanded.insert(node_id, expanded);
				responses.add(PropertiesPanelMessage::Refresh);
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(PropertiesMessageDiscriminant;)
	}
}

impl PropertiesPanelMessageHandler {
	fn update_all_section_expansion_recursive(layout: &mut [LayoutGroup], expanded: bool, section_expanded: &mut HashMap<u64, bool>) {
		for group in layout {
			if let LayoutGroup::Section {
				id, layout, expanded: group_expanded, ..
			} = group
			{
				*group_expanded = expanded;
				section_expanded.insert(*id, expanded);

				Self::update_all_section_expansion_recursive(&mut layout.0, expanded, section_expanded);
			}
		}
	}
}

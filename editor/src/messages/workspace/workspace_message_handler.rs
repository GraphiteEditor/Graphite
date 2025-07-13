use crate::messages::prelude::*;

#[derive(Debug, Clone, Default, ExtractField)]
pub struct WorkspaceMessageHandler {
	node_graph_visible: bool,
}

#[message_handler_data]
impl MessageHandler<WorkspaceMessage, ()> for WorkspaceMessageHandler {
	fn process_message(&mut self, message: WorkspaceMessage, _responses: &mut VecDeque<Message>, _: ()) {
		match message {
			// Messages
			WorkspaceMessage::NodeGraphToggleVisibility => {
				self.node_graph_visible = !self.node_graph_visible;
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(WorkspaceMessageDiscriminant;
			NodeGraphToggleVisibility,
		)
	}
}

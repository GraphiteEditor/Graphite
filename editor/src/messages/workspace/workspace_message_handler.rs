use crate::messages::prelude::*;

#[derive(Debug, Clone, Default)]
pub struct WorkspaceMessageHandler {
	node_graph_visible: bool,
}

impl MessageHandler<WorkspaceMessage, ()> for WorkspaceMessageHandler {
	fn process_message(&mut self, message: WorkspaceMessage, _responses: &mut VecDeque<Message>, _data: ()) {
		use WorkspaceMessage::*;

		match message {
			// Messages
			NodeGraphToggleVisibility => {
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

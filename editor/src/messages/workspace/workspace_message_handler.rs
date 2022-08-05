use crate::messages::prelude::*;

#[derive(Debug, Clone, Default)]
pub struct WorkspaceMessageHandler {
	node_graph_visible: bool,
}

impl MessageHandler<WorkspaceMessage, &InputPreprocessorMessageHandler> for WorkspaceMessageHandler {
	#[remain::check]
	fn process_message(&mut self, message: WorkspaceMessage, _ipp: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		use WorkspaceMessage::*;

		#[remain::sorted]
		match message {
			// Messages
			NodeGraphToggleVisibility => {
				self.node_graph_visible = !self.node_graph_visible;
				responses.push_back(FrontendMessage::UpdateNodeGraphVisibility { visible: self.node_graph_visible }.into());
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(WorkspaceMessageDiscriminant;
			NodeGraphToggleVisibility,
		)
	}
}

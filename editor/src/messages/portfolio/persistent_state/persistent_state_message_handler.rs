use crate::messages::frontend::utility_types::PersistedState;
use crate::messages::portfolio::utility_types::WorkspacePanelLayout;
use crate::messages::prelude::*;

#[derive(Default, Debug, Clone, ExtractField)]
pub struct PersistentStateMessageHandler {
	loaded: bool,
}

#[derive(ExtractField)]
pub struct PersistentStateMessageContext {
	pub persisted_state: PersistedState,
}

#[message_handler_data]
impl MessageHandler<PersistentStateMessage, PersistentStateMessageContext> for PersistentStateMessageHandler {
	fn process_message(&mut self, message: PersistentStateMessage, responses: &mut VecDeque<Message>, context: PersistentStateMessageContext) {
		let PersistentStateMessageContext { persisted_state: state } = context;

		match message {
			PersistentStateMessage::ReadState => {
				responses.add(FrontendMessage::TriggerPersistenceReadState);
			}
			PersistentStateMessage::WriteState => {
				if !self.loaded && (state.documents.is_empty() && (state.workspace_layout == Some(WorkspacePanelLayout::default()) || state.workspace_layout.is_none())) {
					return;
				}
				self.loaded = true;
				responses.add(FrontendMessage::TriggerPersistenceWriteState { state });
			}
			PersistentStateMessage::LoadState { state } => {
				self.loaded = true;
				responses.add(PortfolioMessage::LoadPersistedState { state });
			}
			PersistentStateMessage::ReadDocument { document_id } => {
				responses.add(FrontendMessage::TriggerPersistenceReadDocument { document_id });
			}
			PersistentStateMessage::WriteDocument { document_id, document } => {
				responses.add(FrontendMessage::TriggerPersistenceWriteDocument { document_id, document });
			}
			PersistentStateMessage::DeleteDocument { document_id } => {
				responses.add(FrontendMessage::TriggerPersistenceDeleteDocument { document_id });
			}
			PersistentStateMessage::LoadDocument { document_id, document } => {
				responses.add(PortfolioMessage::LoadDocumentContent {
					document_id,
					document_serialized_content: document,
				});
			}
		}
	}

	advertise_actions!(PersistentStateMessageDiscriminant;);
}

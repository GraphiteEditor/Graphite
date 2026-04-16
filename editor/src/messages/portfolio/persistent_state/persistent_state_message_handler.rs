use super::persistent_state_message::PersistentStateMessage;
use crate::messages::frontend::utility_types::PersistedState;
use crate::messages::prelude::*;

#[derive(Default, Debug, Clone, ExtractField)]
pub struct PersistentStateMessageHandler {}

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
				responses.add(FrontendMessage::TriggerPersistenceWriteState { state });
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
		}
	}

	advertise_actions!(PersistentStateMessageDiscriminant;);
}

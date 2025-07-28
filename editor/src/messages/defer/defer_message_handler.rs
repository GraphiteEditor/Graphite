use crate::messages::prelude::*;

#[derive(Debug, Default, ExtractField)]
pub struct DeferMessageHandler {
	after_graph_run: Vec<(u64, Message)>,
	after_viewport_resize: Vec<Message>,
	current_graph_submission_id: u64,
}

#[message_handler_data]
impl MessageHandler<DeferMessage, ()> for DeferMessageHandler {
	fn process_message(&mut self, message: DeferMessage, responses: &mut VecDeque<Message>, _: ()) {
		match message {
			DeferMessage::AfterGraphRun { mut messages } => {
				self.after_graph_run.extend(messages.drain(..).map(|m| (self.current_graph_submission_id, m)));
			}
			DeferMessage::AfterViewportReady { messages } => {
				self.after_viewport_resize.extend_from_slice(&messages);
			}
			DeferMessage::TriggerGraphRun(execution_id) => {
				self.current_graph_submission_id = execution_id;
				for message in self.after_graph_run.extract_if(.., |x| x.0 < self.current_graph_submission_id) {
					responses.push_front(message.1);
				}
			}
			DeferMessage::TriggerViewportReady => {
				for message in self.after_viewport_resize.drain(..) {
					responses.push_front(message);
				}
			}
		}
	}

	advertise_actions!(DeferMessageDiscriminant;
	);
}

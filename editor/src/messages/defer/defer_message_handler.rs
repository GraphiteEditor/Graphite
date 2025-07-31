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
			DeferMessage::AfterNavigationReady { messages } => {
				self.after_viewport_resize.extend_from_slice(&messages);
			}
			DeferMessage::SetGraphSubmissionIndex(execution_id) => {
				self.current_graph_submission_id = execution_id + 1;
			}
			DeferMessage::TriggerGraphRun(execution_id) => {
				if self.after_graph_run.is_empty() {
					return;
				}
				// Find the index of the last message we can process
				let num_elements_to_remove = self.after_graph_run.binary_search_by_key(&(execution_id + 1), |x| x.0).unwrap_or_else(|pos| pos - 1);
				let elements = self.after_graph_run.drain(0..=num_elements_to_remove);
				for (_, message) in elements.rev() {
					responses.push_front(message);
				}
			}
			DeferMessage::TriggerNavigationReady => {
				for message in self.after_viewport_resize.drain(..).rev() {
					responses.push_front(message);
				}
			}
		}
	}

	advertise_actions!(DeferMessageDiscriminant;
	);
}

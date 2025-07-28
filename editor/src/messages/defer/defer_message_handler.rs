use crate::messages::{input_mapper::utility_types::input_mouse::ViewportBounds, prelude::*};

#[derive(ExtractField)]
pub struct DeferMessageContext {
	viewport_bounds: ViewportBounds,
}

#[derive(Debug, Default, ExtractField)]
pub struct DeferMessageHandler {
	after_graph_run: Vec<(u64, Message)>,
	after_viewport_resize: Vec<Message>,
	current_graph_submission_id: u64,
}

#[message_handler_data]
impl MessageHandler<DeferMessage, DeferMessageContext> for DeferMessageHandler {
	fn process_message(&mut self, message: DeferMessage, responses: &mut VecDeque<Message>, context: DeferMessageContext) {
		match message {
			DeferMessage::AfterGraphRun { mut messages } => {
				self.after_graph_run.extend(messages.drain(..).map(|m| (self.current_graph_submission_id, m)));
			}
			DeferMessage::AfterViewportReady { messages } => {
				if context.viewport_bounds == ViewportBounds::default() {
					self.after_viewport_resize.extend_from_slice(&messages);
				} else {
					for message in messages {
						responses.push_front(message);
					}
				}
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

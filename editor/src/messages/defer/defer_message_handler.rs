use crate::messages::prelude::*;

#[derive(Debug, Default, ExtractField)]
pub struct DeferMessageHandler {
	after_graph_run: Vec<Message>,
	after_viewport_resize: Vec<Message>,
}

#[message_handler_data]
impl MessageHandler<DeferMessage, ()> for DeferMessageHandler {
	fn process_message(&mut self, message: DeferMessage, responses: &mut VecDeque<Message>, _: ()) {
		match message {
			DeferMessage::AfterGraphRun { messages } => {
				self.after_graph_run.extend_from_slice(&messages);
			}
			DeferMessage::AfterViewportResize { messages } => {
				self.after_viewport_resize.extend_from_slice(&messages);
			}
			DeferMessage::TriggerGraphRun => {
				for message in self.after_graph_run.drain(..) {
					responses.push_front(message);
				}
			}
			DeferMessage::TriggerViewportResize => {
				for message in self.after_viewport_resize.drain(..) {
					responses.push_front(message);
				}
			}
		}
	}

	advertise_actions!(DeferMessageDiscriminant;
	);
}

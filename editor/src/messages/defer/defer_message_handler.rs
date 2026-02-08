use crate::messages::prelude::*;

#[derive(ExtractField)]
pub struct DeferMessageContext<'a> {
	pub portfolio: &'a PortfolioMessageHandler,
}

#[derive(Debug, Default, ExtractField)]
pub struct DeferMessageHandler {
	after_graph_run: HashMap<DocumentId, Vec<(u64, Message)>>,
	after_viewport_resize: Vec<Message>,
	current_graph_submission_id: u64,
}

#[message_handler_data]
impl MessageHandler<DeferMessage, DeferMessageContext<'_>> for DeferMessageHandler {
	fn process_message(&mut self, message: DeferMessage, responses: &mut VecDeque<Message>, context: DeferMessageContext) {
		match message {
			DeferMessage::AfterGraphRun { mut messages } => {
				let after_graph_run = self.after_graph_run.entry(context.portfolio.active_document_id.unwrap_or(DocumentId(0))).or_default();
				after_graph_run.extend(messages.drain(..).map(|m| (self.current_graph_submission_id, m)));
				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			DeferMessage::AfterNavigationReady { messages } => {
				self.after_viewport_resize.extend_from_slice(&messages);
			}
			DeferMessage::SetGraphSubmissionIndex { execution_id } => {
				self.current_graph_submission_id = execution_id + 1;
			}
			DeferMessage::TriggerGraphRun { execution_id, document_id } => {
				let after_graph_run = self.after_graph_run.entry(document_id).or_default();
				if after_graph_run.is_empty() {
					return;
				}
				// Find the index of the last message we can process
				let split = after_graph_run.partition_point(|&(id, _)| id <= execution_id);
				let elements = after_graph_run.drain(..split);
				for (_, message) in elements.rev() {
					responses.add_front(message);
				}
				for (&document_id, messages) in self.after_graph_run.iter() {
					if !messages.is_empty() {
						responses.add(PortfolioMessage::SubmitGraphRender { document_id, ignore_hash: false });
					}
				}
			}
			DeferMessage::TriggerNavigationReady => {
				for message in self.after_viewport_resize.drain(..).rev() {
					responses.add_front(message);
				}
			}
		}
	}

	advertise_actions!(DeferMessageDiscriminant;
	);
}

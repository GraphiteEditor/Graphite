use crate::{
	dispatcher::{Action, ActionHandler, InputPreprocessor, Response},
	tools::ToolActionHandlerData,
};
use document_core::Operation;

#[derive(Default)]
pub struct Path;

impl<'a> ActionHandler<ToolActionHandlerData<'a>> for Path {
	fn process_action(&mut self, data: ToolActionHandlerData<'a>, input_preprocessor: &InputPreprocessor, action: &Action, responses: &mut Vec<Response>, operations: &mut Vec<Operation>) -> bool {
		self.fsm_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, &mut responses, &mut operations);

		false
	}
}

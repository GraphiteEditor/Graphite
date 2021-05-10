use crate::tools::Fsm;
use crate::SvgDocument;
use crate::{
	dispatcher::{Action, ActionHandler, InputPreprocessor, Response},
	tools::{DocumentToolData, ToolActionHandlerData},
};
use document_core::Operation;

#[derive(Default)]
pub struct Select {
	fsm_state: SelectToolFsmState,
	data: SelectToolData,
}

impl<'a> ActionHandler<ToolActionHandlerData<'a>> for Select {
	fn process_action(&mut self, data: ToolActionHandlerData<'a>, input_preprocessor: &InputPreprocessor, action: &Action, responses: &mut Vec<Response>, operations: &mut Vec<Operation>) -> bool {
		let (consumed, state) = self.fsm_state.transition(action, data.0, data.1, &mut self.data, input_preprocessor, responses, operations);
		self.fsm_state = state;
		consumed
	}
	actions!();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectToolFsmState {
	Ready,
	LmbDown,
	TransformSelected,
}

impl Default for SelectToolFsmState {
	fn default() -> Self {
		SelectToolFsmState::Ready
	}
}

#[derive(Default)]
struct SelectToolData;

impl Fsm for SelectToolFsmState {
	type ToolData = SelectToolData;

	fn transition(
		self,
		event: &Action,
		_document: &SvgDocument,
		_tool_data: &DocumentToolData,
		_data: &mut Self::ToolData,
		input: &InputPreprocessor,
		_responses: &mut Vec<Response>,
		_operations: &mut Vec<Operation>,
	) -> (bool, Self) {
		match (self, event) {
			(SelectToolFsmState::Ready, Action::LmbDown) => (true, SelectToolFsmState::LmbDown),

			(SelectToolFsmState::LmbDown, Action::LmbUp) => (true, SelectToolFsmState::Ready),

			(SelectToolFsmState::LmbDown, Action::MouseMove) => (true, SelectToolFsmState::TransformSelected),

			(SelectToolFsmState::TransformSelected, Action::MouseMove) => (true, self),

			(SelectToolFsmState::TransformSelected, Action::LmbUp) => (true, SelectToolFsmState::Ready),

			_ => (false, self),
		}
	}
}

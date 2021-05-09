use crate::events::{Event, ToolResponse};
use crate::tools::Fsm;
use crate::Document;
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
		self.fsm_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, &mut responses, &mut operations);

		false
	}
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

	fn transition(self, event: &Event, _document: &Document, _tool_data: &DocumentToolData, _data: &mut Self::ToolData, _responses: &mut Vec<ToolResponse>, _operations: &mut Vec<Operation>) -> Self {
		match (self, event) {
			(SelectToolFsmState::Ready, Event::LmbDown(_mouse_state)) => SelectToolFsmState::LmbDown,

			(SelectToolFsmState::LmbDown, Event::LmbUp(_mouse_state)) => SelectToolFsmState::Ready,

			(SelectToolFsmState::LmbDown, Event::MouseMove(_mouse_state)) => SelectToolFsmState::TransformSelected,

			(SelectToolFsmState::TransformSelected, Event::MouseMove(_mouse_state)) => self,

			(SelectToolFsmState::TransformSelected, Event::LmbUp(_mouse_state)) => SelectToolFsmState::Ready,

			_ => self,
		}
	}
}

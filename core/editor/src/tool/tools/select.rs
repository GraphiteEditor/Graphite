use crate::input::InputPreprocessor;
use crate::tool::{DocumentToolData, Fsm, ToolActionHandlerData};
use crate::{message_prelude::*, SvgDocument};

#[derive(Default)]
pub struct Select {
	fsm_state: SelectToolFsmState,
	data: SelectToolData,
}

#[impl_message(Message, ToolMessage, Select)]
#[derive(PartialEq, Clone, Debug)]
pub enum SelectMessage {
	MouseMove,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Select {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		self.fsm_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, data.2, responses);
	}
	actions_fn!();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectToolFsmState {
	Ready,
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
		event: ToolMessage,
		_document: &SvgDocument,
		_tool_data: &DocumentToolData,
		_data: &mut Self::ToolData,
		_input: &InputPreprocessor,
		_responses: &mut VecDeque<Message>,
	) -> Self {
		use SelectMessage::*;
		use SelectToolFsmState::*;
		if let ToolMessage::Select(event) = event {
			match (self, event) {
				(Ready, MouseMove) => self,
			}
		} else {
			self
		}
	}
}

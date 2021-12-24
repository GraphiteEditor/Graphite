use crate::message_prelude::*;
use crate::tool::ToolActionHandlerData;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct Navigate;

#[impl_message(Message, ToolMessage, Navigate)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum NavigateMessage {
	MouseMove,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Navigate {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		todo!("{}::handle_input {:?} {:?} {:?} ", module_path!(), action, data, responses);
	}

	advertise_actions!();
}

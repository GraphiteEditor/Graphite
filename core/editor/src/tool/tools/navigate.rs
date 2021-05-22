use crate::message_prelude::*;
use crate::tool::ToolActionHandlerData;

#[derive(Default)]
pub struct Navigate;

#[impl_message(Message, ToolMessage, Navigate)]
#[derive(PartialEq, Clone, Debug)]
pub enum NavigateMessage {
	MouseMove,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Navigate {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		todo!("{}::handle_input {:?} {:?} {:?} ", module_path!(), action, data, responses);
	}
	advertise_actions!();
}

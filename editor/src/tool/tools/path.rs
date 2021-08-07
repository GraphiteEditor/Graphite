use crate::message_prelude::*;
use crate::tool::ToolActionHandlerData;

#[derive(Default)]
pub struct Path;

#[impl_message(Message, ToolMessage, Path)]
#[derive(PartialEq, Clone, Debug, Hash)]
pub enum PathMessage {
	MouseMove,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Path {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		todo!("{}::handle_input {:?} {:?} {:?} ", module_path!(), action, data, responses);
	}
	advertise_actions!();
}

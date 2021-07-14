use crate::message_prelude::*;
use crate::tool::ToolActionHandlerData;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct Path;

#[impl_message(Message, ToolMessage, Path)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum PathMessage {
	MouseMove,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Path {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		todo!("{}::handle_input {:?} {:?} {:?} ", module_path!(), action, data, responses);
	}
	advertise_actions!();
}

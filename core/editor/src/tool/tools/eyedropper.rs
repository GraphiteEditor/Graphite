use crate::message_prelude::*;
use crate::tool::ToolActionHandlerData;

#[derive(Default)]
pub struct Eyedropper;

#[impl_message(Message, ToolMessage, Eyedropper)]
#[derive(PartialEq, Clone, Debug)]
pub enum EyedropperMessage {
	MouseMove,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Eyedropper {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		todo!("{}::handle_input {:?} {:?} {:?} ", module_path!(), action, data, responses);
	}
	advertise_actions!();
}

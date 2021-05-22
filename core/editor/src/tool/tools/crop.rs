use crate::message_prelude::*;
use crate::tool::ToolActionHandlerData;

#[derive(Default)]
pub struct Crop;

#[impl_message(Message, ToolMessage, Crop)]
#[derive(PartialEq, Clone, Debug)]
pub enum CropMessage {
	MouseMove,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Crop {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		todo!("{}::handle_input {:?} {:?} {:?} ", module_path!(), action, data, responses);
	}
	advertise_actions!();
}

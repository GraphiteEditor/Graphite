use crate::message_prelude::*;
use crate::viewport_tools::tool::ToolActionHandlerData;

use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct Crop;

#[remain::sorted]
#[impl_message(Message, ToolMessage, Crop)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum CropMessage {
	MouseMove,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Crop {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		todo!("{}::handle_input {:?} {:?} {:?} ", module_path!(), action, data, responses);
	}

	advertise_actions!();
}

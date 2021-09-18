use crate::input::InputPreprocessor;
use crate::tool::{DocumentToolData, Fsm, ToolActionHandlerData, ToolOptions, ToolType};
use crate::{document::DocumentMessageHandler, message_prelude::*};
use glam::DAffine2;
use graphene::{layers::style, Operation};
use log::info;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct Text;

#[impl_message(Message, ToolMessage, Text)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum TextMessage {
	PlaceText,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Text {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		let path = vec![generate_uuid()];
		responses.extend([
			Operation::AddText {
				path: path.clone(),
				insert_index: -1,
				style: style::PathStyle::new(None, Some(style::Fill::new(data.1.primary_color))),
			}
			.into(),
			Operation::SetLayerTransformInViewport {
				path,
				transform: DAffine2::from_translation(data.2.mouse.position).to_cols_array(),
			}
			.into(),
		]);
	}
	fn actions(&self) -> ActionList {
		actions!(TextMessageDiscriminant; PlaceText)
	}
}

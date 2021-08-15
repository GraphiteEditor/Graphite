use crate::consts::SELECTION_TOLERANCE;
use crate::message_prelude::*;
use crate::tool::{ToolActionHandlerData, ToolMessage};
use glam::DVec2;
use graphene::layers::LayerDataType;
use graphene::Quad;

#[derive(Default)]
pub struct Eyedropper;

#[impl_message(Message, ToolMessage, Eyedropper)]
#[derive(PartialEq, Clone, Debug, Hash)]
pub enum EyedropperMessage {
	LeftMouseDown,
	RightMouseDown,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Eyedropper {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		let mouse_pos = data.2.mouse.position;
		let tolerance = DVec2::splat(SELECTION_TOLERANCE);
		let quad = Quad::from_box([mouse_pos - tolerance, mouse_pos + tolerance]);

		if let Some(path) = data.0.document.intersects_quad_root(quad).last() {
			if let Ok(layer) = data.0.document.layer(path) {
				if let LayerDataType::Shape(s) = &layer.data {
					s.style.fill().and_then(|fill| {
						fill.color().map(|color| match action {
							ToolMessage::Eyedropper(EyedropperMessage::LeftMouseDown) => responses.push_back(ToolMessage::SelectPrimaryColor(color).into()),
							ToolMessage::Eyedropper(EyedropperMessage::RightMouseDown) => responses.push_back(ToolMessage::SelectSecondaryColor(color).into()),
							_ => {}
						})
					});
				}
			}
		}
	}
	advertise_actions!(EyedropperMessageDiscriminant; LeftMouseDown, RightMouseDown);
}

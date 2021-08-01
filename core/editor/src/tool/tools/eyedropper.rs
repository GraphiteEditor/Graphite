use crate::consts::SELECTION_TOLERANCE;
use crate::message_prelude::*;
use crate::tool::{ToolActionHandlerData, ToolMessage};
use document_core::layers::LayerDataType;
use glam::DVec2;

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
		let (x, y) = (mouse_pos.x as f64, mouse_pos.y as f64);
		let (point_1, point_2) = (
			DVec2::new(x - SELECTION_TOLERANCE, y - SELECTION_TOLERANCE),
			DVec2::new(x + SELECTION_TOLERANCE, y + SELECTION_TOLERANCE),
		);

		let quad = [
			DVec2::new(point_1.x, point_1.y),
			DVec2::new(point_2.x, point_1.y),
			DVec2::new(point_2.x, point_2.y),
			DVec2::new(point_1.x, point_2.y),
		];

		if let Some(path) = data.0.document.intersects_quad_root(quad).last() {
			if let Ok(layer) = data.0.document.layer(path) {
				if let LayerDataType::Shape(s) = &layer.data {
					s.style.fill().map(|fill| {
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

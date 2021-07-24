use crate::consts::SELECTION_TOLERANCE;
use crate::frontend::FrontendMessage;
use crate::message_prelude::*;
use crate::tool::{ToolActionHandlerData, ToolMessage};
use glam::DVec2;

#[derive(Default)]
pub struct Eyedropper;

#[impl_message(Message, ToolMessage, Eyedropper)]
#[derive(PartialEq, Clone, Debug)]
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
				if let Some(fill) = layer.style.fill() {
					if let Some(color) = fill.color() {
						let (primary, secondary) = match action {
							ToolMessage::Eyedropper(EyedropperMessage::LeftMouseDown) => (color, data.1.secondary_color),
							ToolMessage::Eyedropper(EyedropperMessage::RightMouseDown) => (data.1.primary_color, color),
							_ => (data.1.primary_color, data.1.secondary_color),
						};
						responses.push_back(FrontendMessage::UpdateWorkingColors { primary, secondary }.into());
					}
				}
			}
		}
	}
	advertise_actions!(EyedropperMessageDiscriminant; LeftMouseDown, RightMouseDown);
}

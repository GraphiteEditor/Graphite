use crate::consts::SELECTION_TOLERANCE;
use crate::message_prelude::*;
use crate::tool::ToolActionHandlerData;
use document_core::Operation;
use glam::DVec2;

#[derive(Default)]
pub struct Fill;

#[impl_message(Message, ToolMessage, Fill)]
#[derive(PartialEq, Clone, Debug)]
pub enum FillMessage {
	MouseDown,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Fill {
	fn process_action(&mut self, _action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
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
			responses.push_back(
				Operation::FillLayer {
					path: path.to_vec(),
					color: data.1.primary_color,
				}
				.into(),
			);
		}
	}
	advertise_actions!(FillMessageDiscriminant; MouseDown);
}

use crate::message_prelude::*;
use crate::tool::ToolActionHandlerData;
use document_core::Operation;

#[derive(Default)]
pub struct Fill;

#[impl_message(Message, ToolMessage, Fill)]
#[derive(PartialEq, Clone, Debug)]
pub enum FillMessage {
	MouseDown,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Fill {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		let mouse_pos = data.2.mouse.position;
		let quad = [
			glam::DVec2::new(mouse_pos.x as f64 - 1., mouse_pos.y as f64 - 1.),
			glam::DVec2::new(mouse_pos.x as f64 + 1., mouse_pos.y as f64 - 1.),
			glam::DVec2::new(mouse_pos.x as f64 - 1., mouse_pos.y as f64 + 1.),
			glam::DVec2::new(mouse_pos.x as f64 + 1., mouse_pos.y as f64 + 1.),
		];
		if let Some(path) = data.0.intersects_quad_root(quad).get(0) {
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

use crate::consts::SELECTION_TOLERANCE;
use crate::message_prelude::*;
use crate::tool::ToolActionHandlerData;
use glam::DVec2;
use graphene::{Operation, Quad};

#[derive(Default)]
pub struct Fill;

#[impl_message(Message, ToolMessage, Fill)]
#[derive(PartialEq, Clone, Debug, Hash)]
pub enum FillMessage {
	MouseDown,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Fill {
	fn process_action(&mut self, _action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		let mouse_pos = data.2.mouse.position;
		let tolerance = DVec2::splat(SELECTION_TOLERANCE);
		let quad = Quad::from_box([mouse_pos - tolerance, mouse_pos + tolerance]);

		if let Some(path) = data.0.document.intersects_quad_root(quad).last() {
			responses.push_back(
				Operation::SetLayerFill {
					path: path.to_vec(),
					color: data.1.primary_color,
				}
				.into(),
			);
		}
	}
	advertise_actions!(FillMessageDiscriminant; MouseDown);
}

use crate::consts::SELECTION_TOLERANCE;
use crate::input::keyboard::{Key, MouseMotion};
use crate::message_prelude::*;
use crate::misc::{HintData, HintGroup, HintInfo, KeysGroup};
use crate::tool::{ToolActionHandlerData, ToolMessage};
use glam::DVec2;
use graphene::layers::LayerDataType;
use graphene::Quad;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct Eyedropper;

#[impl_message(Message, ToolMessage, Eyedropper)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum EyedropperMessage {
	LeftMouseDown,
	RightMouseDown,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Eyedropper {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		let hint_data = HintData(vec![HintGroup(vec![
			HintInfo {
				key_groups: vec![],
				mouse: Some(MouseMotion::Lmb),
				label: String::from("Sample Fill"),
				plus: false,
			},
			HintInfo {
				key_groups: vec![],
				mouse: Some(MouseMotion::Rmb),
				label: String::from("Sample Fill as Secondary"),
				plus: false,
			},
		])]);
		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());

		if action == ToolMessage::UpdateHints {
			return;
		}

		let mouse_pos = data.2.mouse.position;
		let tolerance = DVec2::splat(SELECTION_TOLERANCE);
		let quad = Quad::from_box([mouse_pos - tolerance, mouse_pos + tolerance]);

		if let Some(path) = data.0.graphene_document.intersects_quad_root(quad).last() {
			if let Ok(layer) = data.0.graphene_document.layer(path) {
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

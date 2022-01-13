use crate::consts::SELECTION_TOLERANCE;
use crate::document::DocumentMessageHandler;
use crate::input::{keyboard::MouseMotion, InputPreprocessor};
use crate::message_prelude::*;
use crate::misc::{HintData, HintGroup, HintInfo};
use crate::tool::{DocumentToolData, Fsm, ToolActionHandlerData, ToolMessage};
use glam::DVec2;
use graphene::layers::LayerDataType;
use graphene::Quad;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct Eyedropper {
	fsm_state: EyedropperToolFsmState,
	data: EyedropperToolData,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Eyedropper)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum EyedropperMessage {
	Abort,
	LeftMouseDown,
	RightMouseDown,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Eyedropper {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if action == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		let new_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, data.2, responses);

		if self.fsm_state != new_state {
			self.fsm_state = new_state;
			self.fsm_state.update_hints(responses);
		}
	}

	advertise_actions!(EyedropperMessageDiscriminant; LeftMouseDown, RightMouseDown);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EyedropperToolFsmState {
	Ready,
}

impl Default for EyedropperToolFsmState {
	fn default() -> Self {
		EyedropperToolFsmState::Ready
	}
}

#[derive(Clone, Debug, Default)]
struct EyedropperToolData {}

impl Fsm for EyedropperToolFsmState {
	type ToolData = EyedropperToolData;

	fn transition(
		self,
		event: ToolMessage,
		document: &DocumentMessageHandler,
		_tool_data: &DocumentToolData,
		_data: &mut Self::ToolData,
		input: &InputPreprocessor,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use EyedropperMessage::*;
		use EyedropperToolFsmState::*;
		if let ToolMessage::Eyedropper(event) = event {
			match (self, event) {
				(Ready, lmb_or_rmb) if lmb_or_rmb == LeftMouseDown || lmb_or_rmb == RightMouseDown => {
					let mouse_pos = input.mouse.position;
					let tolerance = DVec2::splat(SELECTION_TOLERANCE);
					let quad = Quad::from_box([mouse_pos - tolerance, mouse_pos + tolerance]);

					if let Some(path) = document.graphene_document.intersects_quad_root(quad).last() {
						if let Ok(layer) = document.graphene_document.layer(path) {
							if let LayerDataType::Shape(shape) = &layer.data {
								if let Some(fill) = shape.style.fill() {
									if let Some(color) = fill.color() {
										match lmb_or_rmb {
											EyedropperMessage::LeftMouseDown => responses.push_back(ToolMessage::SelectPrimaryColor(color).into()),
											EyedropperMessage::RightMouseDown => responses.push_back(ToolMessage::SelectSecondaryColor(color).into()),
											_ => {}
										}
									}
								}
							}
						}
					}

					Ready
				}
				_ => self,
			}
		} else {
			self
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			EyedropperToolFsmState::Ready => HintData(vec![HintGroup(vec![
				HintInfo {
					key_groups: vec![],
					mouse: Some(MouseMotion::Lmb),
					label: String::from("Sample to Primary"),
					plus: false,
				},
				HintInfo {
					key_groups: vec![],
					mouse: Some(MouseMotion::Rmb),
					label: String::from("Sample to Secondary"),
					plus: false,
				},
			])]),
		};

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}
}

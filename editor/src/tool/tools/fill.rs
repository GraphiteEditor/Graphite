use crate::consts::SELECTION_TOLERANCE;
use crate::document::DocumentMessageHandler;
use crate::input::{keyboard::MouseMotion, InputPreprocessor};
use crate::message_prelude::*;
use crate::misc::{HintData, HintGroup, HintInfo};
use crate::tool::ToolActionHandlerData;
use crate::tool::{DocumentToolData, Fsm, ToolMessage};
use glam::DVec2;
use graphene::{Operation, Quad};
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct Fill {
	fsm_state: FillToolFsmState,
	data: FillToolData,
}

#[impl_message(Message, ToolMessage, Fill)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum FillMessage {
	LeftMouseDown,
	RightMouseDown,
	Abort,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Fill {
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

	advertise_actions!(FillMessageDiscriminant; LeftMouseDown, RightMouseDown);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FillToolFsmState {
	Ready,
}

impl Default for FillToolFsmState {
	fn default() -> Self {
		FillToolFsmState::Ready
	}
}

#[derive(Clone, Debug, Default)]
struct FillToolData {}

impl Fsm for FillToolFsmState {
	type ToolData = FillToolData;

	fn transition(
		self,
		event: ToolMessage,
		document: &DocumentMessageHandler,
		tool_data: &DocumentToolData,
		data: &mut Self::ToolData,
		input: &InputPreprocessor,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use FillMessage::*;
		use FillToolFsmState::*;
		if let ToolMessage::Fill(event) = event {
			match (self, event) {
				(Ready, lmb_or_rmb) if lmb_or_rmb == LeftMouseDown || lmb_or_rmb == RightMouseDown => {
					let mouse_pos = input.mouse.position;
					let tolerance = DVec2::splat(SELECTION_TOLERANCE);
					let quad = Quad::from_box([mouse_pos - tolerance, mouse_pos + tolerance]);

					if let Some(path) = document.graphene_document.intersects_quad_root(quad).last() {
						let color = match lmb_or_rmb {
							LeftMouseDown => tool_data.primary_color,
							RightMouseDown => tool_data.secondary_color,
							Abort => unreachable!(),
						};
						responses.push_back(Operation::SetLayerFill { path: path.to_vec(), color }.into());
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
			FillToolFsmState::Ready => HintData(vec![HintGroup(vec![
				HintInfo {
					key_groups: vec![],
					mouse: Some(MouseMotion::Lmb),
					label: String::from("Fill with Primary"),
					plus: false,
				},
				HintInfo {
					key_groups: vec![],
					mouse: Some(MouseMotion::Rmb),
					label: String::from("Fill with Secondary"),
					plus: false,
				},
			])]),
		};

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}
}

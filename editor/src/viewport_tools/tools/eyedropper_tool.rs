use crate::consts::SELECTION_TOLERANCE;
use crate::document::DocumentMessageHandler;
use crate::frontend::utility_types::MouseCursorIcon;
use crate::input::keyboard::MouseMotion;
use crate::input::InputPreprocessorMessageHandler;
use crate::layout::widgets::PropertyHolder;
use crate::message_prelude::*;
use crate::misc::{HintData, HintGroup, HintInfo};
use crate::viewport_tools::tool::{DocumentToolData, Fsm, ToolActionHandlerData};

use graphene::intersection::Quad;
use graphene::layers::layer_info::LayerDataType;

use glam::DVec2;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct EyedropperTool {
	fsm_state: EyedropperToolFsmState,
	data: EyedropperToolData,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Eyedropper)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum EyedropperToolMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,

	// Tool-specific messages
	LeftMouseDown,
	RightMouseDown,
}

impl PropertyHolder for EyedropperTool {}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for EyedropperTool {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if action == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		if action == ToolMessage::UpdateCursor {
			self.fsm_state.update_cursor(responses);
			return;
		}

		let new_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, &(), data.2, responses);

		if self.fsm_state != new_state {
			self.fsm_state = new_state;
			self.fsm_state.update_hints(responses);
			self.fsm_state.update_cursor(responses);
		}
	}

	advertise_actions!(EyedropperToolMessageDiscriminant; LeftMouseDown, RightMouseDown);
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
	type ToolOptions = ();

	fn transition(
		self,
		event: ToolMessage,
		document: &DocumentMessageHandler,
		_tool_data: &DocumentToolData,
		_data: &mut Self::ToolData,
		_tool_options: &Self::ToolOptions,
		input: &InputPreprocessorMessageHandler,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use EyedropperToolFsmState::*;
		use EyedropperToolMessage::*;

		if let ToolMessage::Eyedropper(event) = event {
			match (self, event) {
				(Ready, lmb_or_rmb) if lmb_or_rmb == LeftMouseDown || lmb_or_rmb == RightMouseDown => {
					let mouse_pos = input.mouse.position;
					let tolerance = DVec2::splat(SELECTION_TOLERANCE);
					let quad = Quad::from_box([mouse_pos - tolerance, mouse_pos + tolerance]);

					// TODO: Destroy this pyramid
					if let Some(path) = document.graphene_document.intersects_quad_root(quad).last() {
						if let Ok(layer) = document.graphene_document.layer(path) {
							if let LayerDataType::Shape(shape) = &layer.data {
								if shape.style.fill().is_some() {
									match lmb_or_rmb {
										EyedropperToolMessage::LeftMouseDown => responses.push_back(ToolMessage::SelectPrimaryColor { color: shape.style.fill().color() }.into()),
										EyedropperToolMessage::RightMouseDown => responses.push_back(ToolMessage::SelectSecondaryColor { color: shape.style.fill().color() }.into()),
										_ => {}
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

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default }.into());
	}
}

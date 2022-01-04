use crate::input::keyboard::MouseMotion;
use crate::misc::{HintData, HintGroup, HintInfo, KeysGroup};
use crate::tool::{Fsm, ToolActionHandlerData};
use crate::{input::keyboard::Key, message_prelude::*};
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct Navigate {
	fsm_state: NavigateToolFsmState,
}

#[impl_message(Message, ToolMessage, Navigate)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum NavigateMessage {
	MouseMove { snap_angle: Key, snap_zoom: Key },
	TranslateCanvasBegin,
	RotateCanvasBegin,
	ZoomCanvasBegin,
	TransformCanvasEnd,
	Abort,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Navigate {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if action == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		let new_state = self.fsm_state.transition(action, data.0, data.1, &mut (), data.2, responses);

		if self.fsm_state != new_state {
			self.fsm_state = new_state;
			self.fsm_state.update_hints(responses);
		}
	}

	fn actions(&self) -> ActionList {
		use NavigateToolFsmState::*;
		match self.fsm_state {
			Ready => actions!(NavigateMessageDiscriminant; TranslateCanvasBegin, RotateCanvasBegin, ZoomCanvasBegin),
			_ => actions!(NavigateMessageDiscriminant; MouseMove, TransformCanvasEnd),
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NavigateToolFsmState {
	Ready,
	Translating,
	Rotating,
	Zooming,
}

impl Default for NavigateToolFsmState {
	fn default() -> Self {
		NavigateToolFsmState::Ready
	}
}

impl Fsm for NavigateToolFsmState {
	type ToolData = ();

	fn transition(
		self,
		message: ToolMessage,
		_document: &crate::document::DocumentMessageHandler,
		_tool_data: &crate::tool::DocumentToolData,
		_data: &mut Self::ToolData,
		_input: &crate::input::InputPreprocessor,
		messages: &mut VecDeque<Message>,
	) -> Self {
		if let ToolMessage::Navigate(navigate) = message {
			use NavigateMessage::*;
			match navigate {
				MouseMove { snap_angle, snap_zoom } => {
					messages.push_front(MovementMessage::MouseMove { snap_angle, snap_zoom }.into());
					self
				}
				TranslateCanvasBegin => {
					messages.push_front(MovementMessage::TranslateCanvasBegin.into());
					NavigateToolFsmState::Translating
				}
				RotateCanvasBegin => {
					messages.push_front(MovementMessage::RotateCanvasBegin.into());
					NavigateToolFsmState::Rotating
				}
				ZoomCanvasBegin => {
					messages.push_front(MovementMessage::ZoomCanvasBegin.into());
					NavigateToolFsmState::Zooming
				}
				TransformCanvasEnd => {
					messages.push_front(MovementMessage::TransformCanvasEnd.into());
					NavigateToolFsmState::Ready
				}
				Abort => {
					messages.push_front(MovementMessage::TransformCanvasEnd.into());
					NavigateToolFsmState::Ready
				}
			}
		} else {
			self
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			NavigateToolFsmState::Ready => HintData(vec![
				HintGroup(vec![HintInfo {
					key_groups: vec![],
					mouse: Some(MouseMotion::MmbDrag),
					label: String::from("Translate"),
					plus: false,
				}]),
				HintGroup(vec![
					HintInfo {
						key_groups: vec![],
						mouse: Some(MouseMotion::RmbDrag),
						label: String::from("Rotate (drag around centre)"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyControl])],
						mouse: None,
						label: String::from("Snap rotation to 15° increments"),
						plus: true,
					},
				]),
				HintGroup(vec![
					HintInfo {
						key_groups: vec![],
						mouse: Some(MouseMotion::LmbDrag),
						label: String::from("Zoom in and out (drag up and down)"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyShift])],
						mouse: None,
						label: String::from("Snap zoom to nearest increment"),
						plus: true,
					},
				]),
			]),
			NavigateToolFsmState::Rotating => HintData(vec![HintGroup(vec![HintInfo {
				key_groups: vec![KeysGroup(vec![Key::KeyControl])],
				mouse: None,
				label: String::from("Snap to 15° increments"),
				plus: false,
			}])]),
			NavigateToolFsmState::Zooming => HintData(vec![HintGroup(vec![HintInfo {
				key_groups: vec![KeysGroup(vec![Key::KeyShift])],
				mouse: None,
				label: String::from("Snap zoom to nearest increment"),
				plus: false,
			}])]),
			_ => HintData(Vec::new()),
		};

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}
}

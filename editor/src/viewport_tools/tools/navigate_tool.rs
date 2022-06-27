use crate::frontend::utility_types::MouseCursorIcon;
use crate::input::keyboard::{Key, MouseMotion};
use crate::layout::widgets::PropertyHolder;
use crate::message_prelude::*;
use crate::misc::{HintData, HintGroup, HintInfo, KeysGroup};
use crate::viewport_tools::tool::{Fsm, SignalToMessage, ToolActionHandlerData, ToolTransition};

use glam::DVec2;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct NavigateTool {
	fsm_state: NavigateToolFsmState,
	tool_data: NavigateToolData,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Navigate)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum NavigateToolMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,

	// Tool-specific messages
	ClickZoom {
		zoom_in: bool,
	},
	PointerMove {
		snap_angle: Key,
		snap_zoom: Key,
	},
	RotateCanvasBegin,
	TransformCanvasEnd,
	TranslateCanvasBegin,
	ZoomCanvasBegin,
}

impl PropertyHolder for NavigateTool {}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for NavigateTool {
	fn process_action(&mut self, action: ToolMessage, tool_data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if action == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		if action == ToolMessage::UpdateCursor {
			self.fsm_state.update_cursor(responses);
			return;
		}

		let new_state = self.fsm_state.transition(action, &mut self.tool_data, tool_data, &(), responses);

		if self.fsm_state != new_state {
			self.fsm_state = new_state;
			self.fsm_state.update_hints(responses);
			self.fsm_state.update_cursor(responses);
		}
	}

	fn actions(&self) -> ActionList {
		use NavigateToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(NavigateToolMessageDiscriminant; TranslateCanvasBegin, RotateCanvasBegin, ZoomCanvasBegin),
			_ => actions!(NavigateToolMessageDiscriminant; ClickZoom, PointerMove, TransformCanvasEnd),
		}
	}
}

impl ToolTransition for NavigateTool {
	fn shared_messages(&self) -> SignalToMessage {
		SignalToMessage {
			document_dirty: ToolMessage::NoOp,
			abort: NavigateToolMessage::Abort.into(),
			selection_changed: ToolMessage::NoOp,
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NavigateToolFsmState {
	Ready,
	Panning,
	Tilting,
	Zooming,
}

impl Default for NavigateToolFsmState {
	fn default() -> Self {
		NavigateToolFsmState::Ready
	}
}

#[derive(Clone, Debug, Default)]
struct NavigateToolData {
	drag_start: DVec2,
}

impl Fsm for NavigateToolFsmState {
	type ToolData = NavigateToolData;
	type ToolOptions = ();

	fn transition(
		self,
		message: ToolMessage,
		tool_data: &mut Self::ToolData,
		(_document, _global_tool_data, input, _font_cache): ToolActionHandlerData,
		_tool_options: &Self::ToolOptions,
		messages: &mut VecDeque<Message>,
	) -> Self {
		if let ToolMessage::Navigate(navigate) = message {
			use NavigateToolMessage::*;

			match navigate {
				ClickZoom { zoom_in } => {
					messages.push_front(MovementMessage::TransformCanvasEnd.into());

					// Mouse has not moved from pointerdown to pointerup
					if tool_data.drag_start == input.mouse.position {
						messages.push_front(if zoom_in {
							MovementMessage::IncreaseCanvasZoom { center_on_mouse: true }.into()
						} else {
							MovementMessage::DecreaseCanvasZoom { center_on_mouse: true }.into()
						});
					}

					NavigateToolFsmState::Ready
				}
				PointerMove { snap_angle, snap_zoom } => {
					messages.push_front(
						MovementMessage::PointerMove {
							snap_angle,
							wait_for_snap_angle_release: false,
							snap_zoom,
							zoom_from_viewport: Some(tool_data.drag_start),
						}
						.into(),
					);
					self
				}
				TranslateCanvasBegin => {
					tool_data.drag_start = input.mouse.position;
					messages.push_front(MovementMessage::TranslateCanvasBegin.into());
					NavigateToolFsmState::Panning
				}
				RotateCanvasBegin => {
					tool_data.drag_start = input.mouse.position;
					messages.push_front(MovementMessage::RotateCanvasBegin.into());
					NavigateToolFsmState::Tilting
				}
				ZoomCanvasBegin => {
					tool_data.drag_start = input.mouse.position;
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
				HintGroup(vec![
					HintInfo {
						key_groups: vec![],
						mouse: Some(MouseMotion::Lmb),
						label: String::from("Zoom In"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyShift])],
						mouse: None,
						label: String::from("Zoom Out"),
						plus: true,
					},
				]),
				HintGroup(vec![
					HintInfo {
						key_groups: vec![],
						mouse: Some(MouseMotion::LmbDrag),
						label: String::from("Zoom"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyControl])],
						mouse: None,
						label: String::from("Snap Increments"),
						plus: true,
					},
				]),
				HintGroup(vec![HintInfo {
					key_groups: vec![],
					mouse: Some(MouseMotion::MmbDrag),
					label: String::from("Pan"),
					plus: false,
				}]),
				HintGroup(vec![
					HintInfo {
						key_groups: vec![],
						mouse: Some(MouseMotion::RmbDrag),
						label: String::from("Tilt"),
						plus: false,
					},
					HintInfo {
						key_groups: vec![KeysGroup(vec![Key::KeyControl])],
						mouse: None,
						label: String::from("Snap 15°"),
						plus: true,
					},
				]),
			]),
			NavigateToolFsmState::Tilting => HintData(vec![HintGroup(vec![HintInfo {
				key_groups: vec![KeysGroup(vec![Key::KeyControl])],
				mouse: None,
				label: String::from("Snap 15°"),
				plus: false,
			}])]),
			NavigateToolFsmState::Zooming => HintData(vec![HintGroup(vec![HintInfo {
				key_groups: vec![KeysGroup(vec![Key::KeyControl])],
				mouse: None,
				label: String::from("Snap Increments"),
				plus: false,
			}])]),
			_ => HintData(Vec::new()),
		};

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		let cursor = match *self {
			NavigateToolFsmState::Ready => MouseCursorIcon::ZoomIn,
			NavigateToolFsmState::Panning => MouseCursorIcon::Grabbing,
			NavigateToolFsmState::Tilting => MouseCursorIcon::Default,
			NavigateToolFsmState::Zooming => MouseCursorIcon::ZoomIn,
		};

		responses.push_back(FrontendMessage::UpdateMouseCursor { cursor }.into());
	}
}

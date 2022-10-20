use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::MouseMotion;
use crate::messages::layout::utility_types::layout_widget::PropertyHolder;
use crate::messages::prelude::*;
use crate::messages::tool::utility_types::{DocumentToolData, EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct EyedropperTool {
	fsm_state: EyedropperToolFsmState,
	data: EyedropperToolData,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Eyedropper)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum EyedropperToolMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,

	// Tool-specific messages
	LeftPointerDown,
	LeftPointerUp,
	PointerMove,
	RightPointerDown,
	RightPointerUp,
}

impl ToolMetadata for EyedropperTool {
	fn icon_name(&self) -> String {
		"GeneralEyedropperTool".into()
	}
	fn tooltip(&self) -> String {
		"Eyedropper Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Eyedropper
	}
}

impl PropertyHolder for EyedropperTool {}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for EyedropperTool {
	fn process_message(&mut self, message: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if message == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		if message == ToolMessage::UpdateCursor {
			self.fsm_state.update_cursor(responses);
			return;
		}

		let new_state = self.fsm_state.transition(message, &mut self.data, data, &(), responses);

		if self.fsm_state != new_state {
			self.fsm_state = new_state;
			self.fsm_state.update_hints(responses);
			self.fsm_state.update_cursor(responses);
		}
	}

	advertise_actions!(EyedropperToolMessageDiscriminant;
		LeftPointerDown,
		LeftPointerUp,
		PointerMove,
		RightPointerDown,
		RightPointerUp,
	);
}

impl ToolTransition for EyedropperTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			document_dirty: None,
			tool_abort: Some(EyedropperToolMessage::Abort.into()),
			selection_changed: None,
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EyedropperToolFsmState {
	Ready,
	SamplingPrimary,
	SamplingSecondary,
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
		_tool_data: &mut Self::ToolData,
		(_document, _document_id, global_tool_data, input, _font_cache): ToolActionHandlerData,
		_tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use EyedropperToolFsmState::*;
		use EyedropperToolMessage::*;

		if let ToolMessage::Eyedropper(event) = event {
			match (self, event) {
				// Ready -> Sampling
				(Ready, mouse_down) | (Ready, mouse_down) if mouse_down == LeftPointerDown || mouse_down == RightPointerDown => {
					let (sampling_primary_or_secondary, state) = if mouse_down == LeftPointerDown {
						("primary".to_string(), SamplingPrimary)
					} else {
						("secondary".to_string(), SamplingSecondary)
					};

					update_cursor_preview(responses, input, global_tool_data, sampling_primary_or_secondary, false);
					state
				}
				// Sampling -> Sampling
				(SamplingPrimary, PointerMove) | (SamplingSecondary, PointerMove) => {
					if input.viewport_bounds.in_bounds(input.mouse.position) {
						let sampling_primary_or_secondary = if self == SamplingPrimary { "primary".to_string() } else { "secondary".to_string() };
						update_cursor_preview(responses, input, global_tool_data, sampling_primary_or_secondary, false);
					} else {
						disable_cursor_preview(responses);
					}
					self
				}
				// Sampling -> Ready
				(SamplingPrimary, mouse_up) | (SamplingSecondary, mouse_up) if mouse_up == LeftPointerUp || mouse_up == RightPointerUp => {
					let sampling_primary_or_secondary = if self == SamplingPrimary { "primary".to_string() } else { "secondary".to_string() };
					update_cursor_preview(responses, input, global_tool_data, sampling_primary_or_secondary, true);

					disable_cursor_preview(responses);
					Ready
				}
				// Ready -> Ready
				(Ready, Abort) => {
					disable_cursor_preview(responses);
					Ready
				}
				// Ready -> Ready
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
					key_groups_mac: None,
					mouse: Some(MouseMotion::Lmb),
					label: String::from("Sample to Primary"),
					plus: false,
				},
				HintInfo {
					key_groups: vec![],
					key_groups_mac: None,
					mouse: Some(MouseMotion::Rmb),
					label: String::from("Sample to Secondary"),
					plus: false,
				},
			])]),
			EyedropperToolFsmState::SamplingPrimary => HintData(vec![]),
			EyedropperToolFsmState::SamplingSecondary => HintData(vec![]),
		};

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		let cursor = match *self {
			EyedropperToolFsmState::Ready => MouseCursorIcon::Default,
			EyedropperToolFsmState::SamplingPrimary | EyedropperToolFsmState::SamplingSecondary => MouseCursorIcon::None,
		};

		responses.push_back(FrontendMessage::UpdateMouseCursor { cursor }.into());
	}
}

fn disable_cursor_preview(responses: &mut VecDeque<Message>) {
	responses.push_back(
		FrontendMessage::UpdateEyedropperSamplingState {
			mouse_position: None,
			primary_color: "".into(),
			secondary_color: "".into(),
			sampling_primary_or_secondary: "".into(),
			set_color_choice: false,
		}
		.into(),
	);
}

fn update_cursor_preview(
	responses: &mut VecDeque<Message>,
	input: &InputPreprocessorMessageHandler,
	global_tool_data: &DocumentToolData,
	sampling_primary_or_secondary: String,
	set_color_choice: bool,
) {
	responses.push_back(
		FrontendMessage::UpdateEyedropperSamplingState {
			mouse_position: Some(input.mouse.position.into()),
			primary_color: "#".to_string() + global_tool_data.primary_color.rgb_hex().as_str(),
			secondary_color: "#".to_string() + global_tool_data.secondary_color.rgb_hex().as_str(),
			sampling_primary_or_secondary,
			set_color_choice,
		}
		.into(),
	);
}

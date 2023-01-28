use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, MouseMotion};
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
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize, specta::Type)]
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
		self.fsm_state.process_event(message, &mut self.data, data, &(), responses, true);
	}

	advertise_actions!(EyedropperToolMessageDiscriminant;
		LeftPointerDown,
		LeftPointerUp,
		PointerMove,
		RightPointerDown,
		RightPointerUp,
		Abort,
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum EyedropperToolFsmState {
	#[default]
	Ready,
	SamplingPrimary,
	SamplingSecondary,
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
					update_cursor_preview(responses, input, global_tool_data, None);

					if mouse_down == LeftPointerDown {
						SamplingPrimary
					} else {
						SamplingSecondary
					}
				}
				// Sampling -> Sampling
				(SamplingPrimary, PointerMove) | (SamplingSecondary, PointerMove) => {
					if input.viewport_bounds.in_bounds(input.mouse.position) {
						update_cursor_preview(responses, input, global_tool_data, None);
					} else {
						disable_cursor_preview(responses);
					}

					self
				}
				// Sampling -> Ready
				(SamplingPrimary, mouse_up) | (SamplingSecondary, mouse_up) if mouse_up == LeftPointerUp || mouse_up == RightPointerUp => {
					let set_color_choice = if self == SamplingPrimary { "Primary".to_string() } else { "Secondary".to_string() };
					update_cursor_preview(responses, input, global_tool_data, Some(set_color_choice));
					disable_cursor_preview(responses);

					Ready
				}
				// Any -> Ready
				(_, Abort) => {
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
				HintInfo::mouse(MouseMotion::Lmb, "Sample to Primary"),
				HintInfo::mouse(MouseMotion::Rmb, "Sample to Secondary"),
			])]),
			EyedropperToolFsmState::SamplingPrimary | EyedropperToolFsmState::SamplingSecondary => HintData(vec![HintGroup(vec![HintInfo::keys([Key::Escape], "Cancel")])]),
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
			set_color_choice: None,
		}
		.into(),
	);
}

fn update_cursor_preview(responses: &mut VecDeque<Message>, input: &InputPreprocessorMessageHandler, global_tool_data: &DocumentToolData, set_color_choice: Option<String>) {
	responses.push_back(
		FrontendMessage::UpdateEyedropperSamplingState {
			mouse_position: Some(input.mouse.position.into()),
			primary_color: "#".to_string() + global_tool_data.primary_color.rgb_hex().as_str(),
			secondary_color: "#".to_string() + global_tool_data.secondary_color.rgb_hex().as_str(),
			set_color_choice,
		}
		.into(),
	);
}

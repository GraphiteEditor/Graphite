use super::tool_prelude::*;
use crate::messages::tool::utility_types::DocumentToolData;

#[derive(Default, ExtractField)]
pub struct EyedropperTool {
	fsm_state: EyedropperToolFsmState,
	data: EyedropperToolData,
}

#[impl_message(Message, ToolMessage, Eyedropper)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum EyedropperToolMessage {
	// Standard messages
	Abort,

	// Tool-specific messages
	SamplePrimaryColorBegin,
	SamplePrimaryColorEnd,
	PointerMove,
	SampleSecondaryColorBegin,
	SampleSecondaryColorEnd,
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

impl LayoutHolder for EyedropperTool {
	fn layout(&self) -> Layout {
		Layout::WidgetLayout(WidgetLayout::default())
	}
}

#[message_handler_data]
impl<'a> MessageHandler<ToolMessage, &mut ToolActionMessageContext<'a>> for EyedropperTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, context: &mut ToolActionMessageContext<'a>) {
		self.fsm_state.process_event(message, &mut self.data, context, &(), responses, true);
	}

	advertise_actions!(EyedropperToolMessageDiscriminant;
		SamplePrimaryColorBegin,
		SamplePrimaryColorEnd,
		SampleSecondaryColorBegin,
		SampleSecondaryColorEnd,
		PointerMove,
		Abort,
	);
}

impl ToolTransition for EyedropperTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			tool_abort: Some(EyedropperToolMessage::Abort.into()),
			working_color_changed: Some(EyedropperToolMessage::PointerMove.into()),
			..Default::default()
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

	fn transition(self, event: ToolMessage, _tool_data: &mut Self::ToolData, tool_action_data: &mut ToolActionMessageContext, _tool_options: &(), responses: &mut VecDeque<Message>) -> Self {
		let ToolActionMessageContext { global_tool_data, input, .. } = tool_action_data;

		let ToolMessage::Eyedropper(event) = event else { return self };
		match (self, event) {
			// Ready -> Sampling
			(EyedropperToolFsmState::Ready, mouse_down) if matches!(mouse_down, EyedropperToolMessage::SamplePrimaryColorBegin | EyedropperToolMessage::SampleSecondaryColorBegin) => {
				update_cursor_preview(responses, input, global_tool_data, None);

				if mouse_down == EyedropperToolMessage::SamplePrimaryColorBegin {
					EyedropperToolFsmState::SamplingPrimary
				} else {
					EyedropperToolFsmState::SamplingSecondary
				}
			}
			// Sampling -> Sampling
			(EyedropperToolFsmState::SamplingPrimary | EyedropperToolFsmState::SamplingSecondary, EyedropperToolMessage::PointerMove) => {
				if input.viewport_bounds.in_bounds(input.mouse.position) {
					update_cursor_preview(responses, input, global_tool_data, None);
				} else {
					disable_cursor_preview(responses);
				}

				self
			}
			// Sampling -> Ready
			(EyedropperToolFsmState::SamplingPrimary, EyedropperToolMessage::SamplePrimaryColorEnd) | (EyedropperToolFsmState::SamplingSecondary, EyedropperToolMessage::SampleSecondaryColorEnd) => {
				let set_color_choice = if self == EyedropperToolFsmState::SamplingPrimary { "Primary" } else { "Secondary" }.to_string();
				update_cursor_preview(responses, input, global_tool_data, Some(set_color_choice));
				disable_cursor_preview(responses);

				EyedropperToolFsmState::Ready
			}
			// Any -> Ready
			(_, EyedropperToolMessage::Abort) => {
				disable_cursor_preview(responses);

				EyedropperToolFsmState::Ready
			}
			// Ready -> Ready
			_ => self,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			EyedropperToolFsmState::Ready => HintData(vec![HintGroup(vec![
				HintInfo::mouse(MouseMotion::Lmb, "Sample to Primary"),
				HintInfo::keys_and_mouse([Key::Shift], MouseMotion::Lmb, "Sample to Secondary"),
			])]),
			EyedropperToolFsmState::SamplingPrimary | EyedropperToolFsmState::SamplingSecondary => {
				HintData(vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()])])
			}
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		let cursor = match *self {
			EyedropperToolFsmState::Ready => MouseCursorIcon::Default,
			EyedropperToolFsmState::SamplingPrimary | EyedropperToolFsmState::SamplingSecondary => MouseCursorIcon::None,
		};

		responses.add(FrontendMessage::UpdateMouseCursor { cursor });
	}
}

fn disable_cursor_preview(responses: &mut VecDeque<Message>) {
	responses.add(FrontendMessage::UpdateEyedropperSamplingState {
		mouse_position: None,
		primary_color: "".into(),
		secondary_color: "".into(),
		set_color_choice: None,
	});
}

fn update_cursor_preview(responses: &mut VecDeque<Message>, input: &InputPreprocessorMessageHandler, global_tool_data: &DocumentToolData, set_color_choice: Option<String>) {
	responses.add(FrontendMessage::UpdateEyedropperSamplingState {
		mouse_position: Some(input.mouse.position.into()),
		primary_color: "#".to_string() + global_tool_data.primary_color.to_rgb_hex_srgb().as_str(),
		secondary_color: "#".to_string() + global_tool_data.secondary_color.to_rgb_hex_srgb().as_str(),
		set_color_choice,
	});
}

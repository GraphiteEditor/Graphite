use super::tool_prelude::*;
use crate::messages::frontend::utility_types::EyedropperPreviewImage;
use crate::messages::tool::utility_types::DocumentToolData;
use graphene_std::color::SRGBA8;
use graphene_std::vector::style::RenderMode;

#[derive(Default, ExtractField)]
pub struct EyedropperTool {
	fsm_state: EyedropperToolFsmState,
	data: EyedropperToolData,
}

#[impl_message(Message, ToolMessage, Eyedropper)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(PartialEq, Eq, Clone, Debug, Hash, serde::Serialize, serde::Deserialize)]
pub enum EyedropperToolMessage {
	// Standard messages
	Abort,

	// Tool-specific messages
	SamplePrimaryColorBegin,
	SamplePrimaryColorEnd,
	PointerMove,
	SampleSecondaryColorBegin,
	SampleSecondaryColorEnd,

	PreviewImage { data: Vec<u8>, width: u32, height: u32 },
}

impl ToolMetadata for EyedropperTool {
	fn icon_name(&self) -> String {
		"GeneralEyedropperTool".into()
	}
	fn tooltip_label(&self) -> String {
		"Eyedropper Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Eyedropper
	}
}

impl ToolEditTarget for EyedropperTool {
	fn edit_target(&self) -> EditTarget {
		EditTarget::Nothing
	}
}

impl LayoutHolder for EyedropperTool {
	fn layout(&self) -> Layout {
		Layout::default()
	}
}

#[message_handler_data]
impl<'a> MessageHandler<ToolMessage, &mut ToolActionMessageContext<'a>> for EyedropperTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, context: &mut ToolActionMessageContext<'a>) {
		if let ToolMessage::Eyedropper(EyedropperToolMessage::PreviewImage { data, width, height }) = message {
			let image = EyedropperPreviewImage { data: data.into(), width, height };

			update_cursor_preview_common(responses, Some(image), context.input, context.global_tool_data, self.data.color_choice);

			if !self.data.preview {
				disable_cursor_preview(responses, &mut self.data);
			}
			return;
		}

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

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PrimarySecondary {
	#[default]
	Primary,
	Secondary,
}

#[derive(Clone, Debug, Default)]
struct EyedropperToolData {
	preview: bool,
	color_choice: Option<PrimarySecondary>,
}

impl Fsm for EyedropperToolFsmState {
	type ToolData = EyedropperToolData;
	type ToolOptions = ();

	fn transition(self, event: ToolMessage, tool_data: &mut Self::ToolData, tool_action_data: &mut ToolActionMessageContext, _tool_options: &(), responses: &mut VecDeque<Message>) -> Self {
		let ToolActionMessageContext {
			document,
			global_tool_data,
			input,
			viewport,
			..
		} = tool_action_data;
		let render_mode = document.render_mode;

		let ToolMessage::Eyedropper(event) = event else { return self };
		match (self, event) {
			// Ready -> Sampling
			(EyedropperToolFsmState::Ready, mouse_down) if matches!(mouse_down, EyedropperToolMessage::SamplePrimaryColorBegin | EyedropperToolMessage::SampleSecondaryColorBegin) => {
				update_cursor_preview(responses, tool_data, input, global_tool_data, None, render_mode);

				if mouse_down == EyedropperToolMessage::SamplePrimaryColorBegin {
					EyedropperToolFsmState::SamplingPrimary
				} else {
					EyedropperToolFsmState::SamplingSecondary
				}
			}
			// Sampling -> Sampling
			(EyedropperToolFsmState::SamplingPrimary | EyedropperToolFsmState::SamplingSecondary, EyedropperToolMessage::PointerMove) => {
				let mouse_position = viewport.logical(input.mouse.position);
				if viewport.is_in_bounds(mouse_position + viewport.offset()) {
					update_cursor_preview(responses, tool_data, input, global_tool_data, None, render_mode);
				} else {
					disable_cursor_preview(responses, tool_data);
				}

				self
			}
			// Sampling -> Ready, into the color the press chose even if Shift has changed since, and only when released over the viewport
			(EyedropperToolFsmState::SamplingPrimary | EyedropperToolFsmState::SamplingSecondary, EyedropperToolMessage::SamplePrimaryColorEnd | EyedropperToolMessage::SampleSecondaryColorEnd) => {
				let set_color_choice = if self == EyedropperToolFsmState::SamplingPrimary {
					PrimarySecondary::Primary
				} else {
					PrimarySecondary::Secondary
				};

				let mouse_position = viewport.logical(input.mouse.position);
				if viewport.is_in_bounds(mouse_position + viewport.offset()) {
					update_cursor_preview(responses, tool_data, input, global_tool_data, Some(set_color_choice), render_mode);
				}
				disable_cursor_preview(responses, tool_data);

				EyedropperToolFsmState::Ready
			}
			// Any -> Ready
			(_, EyedropperToolMessage::Abort) => {
				disable_cursor_preview(responses, tool_data);

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

		hint_data.send_layout(responses);
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		let cursor = match *self {
			EyedropperToolFsmState::Ready => MouseCursorIcon::Default,
			EyedropperToolFsmState::SamplingPrimary | EyedropperToolFsmState::SamplingSecondary => MouseCursorIcon::None,
		};

		responses.add(FrontendMessage::UpdateMouseCursor { cursor });
	}
}

fn disable_cursor_preview(responses: &mut VecDeque<Message>, tool_data: &mut EyedropperToolData) {
	tool_data.preview = false;
	responses.add(FrontendMessage::UpdateEyedropperSamplingState {
		image: None,
		mouse_position: None,
		primary_color: "".into(),
		secondary_color: "".into(),
		set_color_choice: None,
	});
}

fn update_cursor_preview(
	responses: &mut VecDeque<Message>,
	tool_data: &mut EyedropperToolData,
	input: &InputPreprocessorMessageHandler,
	global_tool_data: &DocumentToolData,
	set_color_choice: Option<PrimarySecondary>,
	render_mode: RenderMode,
) {
	tool_data.preview = true;
	tool_data.color_choice = set_color_choice;

	// On web, SVG Preview mode uses the frontend's SVG rasterization to sample pixels directly
	#[cfg(target_family = "wasm")]
	if render_mode == RenderMode::SvgPreview {
		update_cursor_preview_common(responses, None, input, global_tool_data, set_color_choice);
		return;
	}

	let _ = (&input, &global_tool_data, &render_mode);

	// For Vello-rendered modes (Normal, Outline, and Pixel Preview), submit a backend render request
	// which will return a zoomed-in pixel preview image via the EyedropperToolMessage::PreviewImage path
	responses.add(PortfolioMessage::SubmitEyedropperPreviewRender);
}

fn update_cursor_preview_common(
	responses: &mut VecDeque<Message>,
	image: Option<EyedropperPreviewImage>,
	input: &InputPreprocessorMessageHandler,
	global_tool_data: &DocumentToolData,
	set_color_choice: Option<PrimarySecondary>,
) {
	responses.add(FrontendMessage::UpdateEyedropperSamplingState {
		image,
		mouse_position: Some(input.mouse.position.into()),
		primary_color: SRGBA8::from(global_tool_data.primary_color).to_css_hex(),
		secondary_color: SRGBA8::from(global_tool_data.secondary_color).to_css_hex(),
		set_color_choice,
	});
}

#[cfg(test)]
mod test_eyedropper {
	use crate::messages::input_mapper::utility_types::pointer::EditorPointerState;
	pub use crate::test_utils::test_prelude::*;

	// Whether a batch of frontend messages hides the sampling preview, which ends a sample
	fn ends_sampling(messages: &[FrontendMessage]) -> bool {
		messages
			.iter()
			.any(|message| matches!(message, FrontendMessage::UpdateEyedropperSamplingState { mouse_position: None, .. }))
	}

	#[tokio::test]
	async fn changing_shift_mid_sample_still_ends_it_on_release() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.select_tool(ToolType::Eyedropper).await;

		for (press, release) in [(ModifierKeys::empty(), ModifierKeys::SHIFT), (ModifierKeys::SHIFT, ModifierKeys::empty())] {
			editor.left_mousedown(50., 50., press).await;
			let messages = editor
				.handle_message(InputPreprocessorMessage::PointerUp {
					editor_mouse_state: EditorPointerState {
						editor_position: (50., 50.).into(),
						mouse_keys: MouseKeys::empty(),
						..Default::default()
					},
					modifier_keys: release,
				})
				.await;
			assert!(
				ends_sampling(&messages),
				"releasing with Shift {press:?} at the press and {release:?} at the release should end the sample"
			);
		}
	}
}

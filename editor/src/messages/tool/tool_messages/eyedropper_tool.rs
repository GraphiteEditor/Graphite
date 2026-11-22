use super::tool_prelude::*;
use crate::messages::frontend::utility_types::EyedropperPreviewImage;
use crate::messages::tool::utility_types::DocumentToolData;
use graphene_std::color::SRGBA8;

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

			show_cursor_preview(responses, image, context.input, context.global_tool_data, self.data.color_choice.take());

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
	/// The color a released sample sets once the next preview render arrives, which a later press can't clear.
	color_choice: Option<PrimarySecondary>,
}

impl Fsm for EyedropperToolFsmState {
	type ToolData = EyedropperToolData;
	type ToolOptions = ();

	fn transition(self, event: ToolMessage, tool_data: &mut Self::ToolData, tool_action_data: &mut ToolActionMessageContext, _tool_options: &(), responses: &mut VecDeque<Message>) -> Self {
		let ToolActionMessageContext { input, viewport, .. } = tool_action_data;

		let ToolMessage::Eyedropper(event) = event else { return self };
		match (self, event) {
			// Ready -> Sampling
			(EyedropperToolFsmState::Ready, mouse_down) if matches!(mouse_down, EyedropperToolMessage::SamplePrimaryColorBegin | EyedropperToolMessage::SampleSecondaryColorBegin) => {
				request_cursor_preview(responses, tool_data);

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
					request_cursor_preview(responses, tool_data);
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
					tool_data.color_choice = Some(set_color_choice);
					request_cursor_preview(responses, tool_data);
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

fn request_cursor_preview(responses: &mut VecDeque<Message>, tool_data: &mut EyedropperToolData) {
	tool_data.preview = true;

	// The Vello render of the area under the cursor returns as `EyedropperToolMessage::PreviewImage`
	responses.add(PortfolioMessage::SubmitEyedropperPreviewRender);
}

fn show_cursor_preview(
	responses: &mut VecDeque<Message>,
	image: EyedropperPreviewImage,
	input: &InputPreprocessorMessageHandler,
	global_tool_data: &DocumentToolData,
	set_color_choice: Option<PrimarySecondary>,
) {
	responses.add(FrontendMessage::UpdateEyedropperSamplingState {
		image: Some(image),
		mouse_position: Some(input.mouse.position.into()),
		primary_color: SRGBA8::from(global_tool_data.primary_color).to_css_hex(),
		secondary_color: SRGBA8::from(global_tool_data.secondary_color).to_css_hex(),
		set_color_choice,
	});
}

#[cfg(test)]
mod test_eyedropper {
	use super::PrimarySecondary;
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

	#[tokio::test]
	async fn pressing_again_before_the_sample_renders_keeps_its_color() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.set_viewport_size(DVec2::ZERO, DVec2::splat(100.)).await;
		editor.select_tool(ToolType::Eyedropper).await;

		let pointer = |mouse_keys| EditorPointerState {
			editor_position: (50., 50.).into(),
			mouse_keys,
			..Default::default()
		};
		let modifier_keys = ModifierKeys::empty();

		// Dispatched without running the runtime in between, so the second press comes before the first sample's render
		editor.dispatch(InputPreprocessorMessage::PointerDown {
			editor_mouse_state: pointer(MouseKeys::LEFT),
			modifier_keys,
		});
		editor.dispatch(InputPreprocessorMessage::PointerUp {
			editor_mouse_state: pointer(MouseKeys::empty()),
			modifier_keys,
		});
		editor.dispatch(InputPreprocessorMessage::PointerDown {
			editor_mouse_state: pointer(MouseKeys::LEFT),
			modifier_keys,
		});

		editor.runtime.update();
		editor.runtime.run().await;
		let messages = editor.dispatch(Message::Wake);

		let sets_primary_color = messages.iter().any(|message| {
			matches!(
				message,
				FrontendMessage::UpdateEyedropperSamplingState {
					set_color_choice: Some(PrimarySecondary::Primary),
					..
				}
			)
		});
		assert!(sets_primary_color, "the first sample's color was lost to the second press");
	}
}

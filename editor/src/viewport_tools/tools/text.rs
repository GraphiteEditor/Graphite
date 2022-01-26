use crate::consts::SELECTION_TOLERANCE;
use crate::document::DocumentMessageHandler;
use crate::frontend::utility_types::MouseCursorIcon;
use crate::input::keyboard::{Key, MouseMotion};
use crate::input::InputPreprocessorMessageHandler;
use crate::message_prelude::*;
use crate::misc::{HintData, HintGroup, HintInfo, KeysGroup};
use crate::viewport_tools::tool::{DocumentToolData, Fsm, ToolActionHandlerData, ToolType};
use crate::viewport_tools::tool_options::ToolOptions;

use glam::{DAffine2, DVec2};
use graphene::intersection::Quad;
use graphene::layers::style;
use graphene::Operation;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct Text {
	fsm_state: TextToolFsmState,
	data: TextToolData,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Text)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum TextMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,

	// Tool-specific messages
	CommitText,
	Interact,
	TextChange {
		new_text: String,
	},
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Text {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if action == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		if action == ToolMessage::UpdateCursor {
			self.fsm_state.update_cursor(responses);
			return;
		}

		let new_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, data.2, responses);

		if self.fsm_state != new_state {
			self.fsm_state = new_state;
			self.fsm_state.update_hints(responses);
			self.fsm_state.update_cursor(responses);
		}
	}

	fn actions(&self) -> ActionList {
		use TextToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(TextMessageDiscriminant; Interact),
			Editing => actions!(TextMessageDiscriminant; Interact, Abort, CommitText),
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TextToolFsmState {
	Ready,
	Editing,
}

impl Default for TextToolFsmState {
	fn default() -> Self {
		TextToolFsmState::Ready
	}
}

#[derive(Clone, Debug, Default)]
struct TextToolData {
	path: Vec<LayerId>,
}

impl Fsm for TextToolFsmState {
	type ToolData = TextToolData;

	fn transition(
		self,
		event: ToolMessage,
		document: &DocumentMessageHandler,
		tool_data: &DocumentToolData,
		data: &mut Self::ToolData,
		input: &InputPreprocessorMessageHandler,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use TextMessage::*;
		use TextToolFsmState::*;

		if let ToolMessage::Text(event) = event {
			match (self, event) {
				(state, Interact) => {
					let mouse_pos = input.mouse.position;
					let tolerance = DVec2::splat(SELECTION_TOLERANCE);
					let quad = Quad::from_box([mouse_pos - tolerance, mouse_pos + tolerance]);

					let new_state = if let Some(l) = document
						.graphene_document
						.intersects_quad_root(quad)
						.last()
						.filter(|l| document.graphene_document.layer(l).map(|l| l.as_text().is_ok()).unwrap_or(false))
					// Editing existing text
					{
						if state == TextToolFsmState::Editing {
							let editable = false;
							responses.push_back(DocumentMessage::SetTextboxEditable { path: data.path.clone(), editable }.into());
						}

						data.path = l.clone();

						let editable = true;
						responses.push_back(DocumentMessage::SetTextboxEditable { path: data.path.clone(), editable }.into());

						Editing
					}
					// Creating new text
					else if state == TextToolFsmState::Ready {
						let transform = DAffine2::from_translation(input.mouse.position).to_cols_array();
						let font_size = match tool_data.tool_options.get(&ToolType::Text) {
							Some(&ToolOptions::Text { font_size }) => font_size,
							_ => 14,
						};
						data.path = vec![generate_uuid()];

						responses.push_back(
							Operation::AddText {
								path: data.path.clone(),
								transform: DAffine2::ZERO.to_cols_array(),
								insert_index: -1,
								text: r#"The quick brown
fox jumped over the lazy cat.
In publishing and graphic design, Lorem ipsum is a placeholder text commonly used to demonstrate the visual form of a document or a typeface without relying on meaningful content. Lorem ipsum may be used as a placeholder before the final copy is available. It is also used to temporarily replace text in a process called greeking, which allows designers to consider the form of a webpage or publication, without the meaning of the text influencing the design.

Lorem ipsum is typically a corrupted version of De finibus bonorum et malorum, a 1st-century BC text by the Roman statesman and philosopher Cicero, with words altered, added, and removed to make it nonsensical and improper Latin.

Test for really long word: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"#.to_string(),
								style: style::PathStyle::new(Some(style::Stroke::new(tool_data.primary_color, 0.)), None),
								size: font_size as f64,
							}
							.into(),
						);
						responses.push_back(Operation::SetLayerTransformInViewport { path: data.path.clone(), transform }.into());

						let editable = true;
						responses.push_back(DocumentMessage::SetTextboxEditable { path: data.path.clone(), editable }.into());

						Editing
					} else {
						// Removing old text as editable
						let editable = false;
						responses.push_back(DocumentMessage::SetTextboxEditable { path: data.path.clone(), editable }.into());
						Ready
					};

					new_state
				}
				(Editing, Abort) => {
					let editable = false;
					responses.push_back(DocumentMessage::SetTextboxEditable { path: data.path.clone(), editable }.into());
					Ready
				}
				(Editing, CommitText) => {
					responses.push_back(FrontendMessage::TriggerTextCommit.into());
					Editing
				}
				(Editing, TextChange { new_text }) => {
					responses.push_back(Operation::SetTextContent { path: data.path.clone(), new_text }.into());

					let editable = false;
					responses.push_back(DocumentMessage::SetTextboxEditable { path: data.path.clone(), editable }.into());

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
			TextToolFsmState::Ready => HintData(vec![HintGroup(vec![
				HintInfo {
					key_groups: vec![],
					mouse: Some(MouseMotion::Lmb),
					label: String::from("Add Text"),
					plus: false,
				},
				HintInfo {
					key_groups: vec![],
					mouse: Some(MouseMotion::Lmb),
					label: String::from("Edit Text"),
					plus: false,
				},
			])]),
			TextToolFsmState::Editing => HintData(vec![HintGroup(vec![
				HintInfo {
					key_groups: vec![KeysGroup(vec![Key::KeyControl, Key::KeyEnter])],
					mouse: None,
					label: String::from("Commit Edit"),
					plus: false,
				},
				HintInfo {
					key_groups: vec![KeysGroup(vec![Key::KeyEscape])],
					mouse: None,
					label: String::from("Discard Edit"),
					plus: false,
				},
			])]),
		};

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Text }.into());
	}
}

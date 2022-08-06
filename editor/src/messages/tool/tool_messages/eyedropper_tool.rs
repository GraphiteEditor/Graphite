use crate::consts::SELECTION_TOLERANCE;
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::MouseMotion;
use crate::messages::layout::utility_types::layout_widget::PropertyHolder;
use crate::messages::prelude::*;
use crate::messages::tool::utility_types::{EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

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
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum EyedropperToolMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,

	// Tool-specific messages
	LeftMouseDown,
	RightMouseDown,
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
		LeftMouseDown,
		RightMouseDown,
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
		(document, _global_tool_data, input, font_cache): ToolActionHandlerData,
		_tool_options: &Self::ToolOptions,
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
					if let Some(path) = document.graphene_document.intersects_quad_root(quad, font_cache).last() {
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
		};

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default }.into());
	}
}

use crate::consts::SELECTION_TOLERANCE;
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::MouseMotion;
use crate::messages::layout::utility_types::layout_widget::PropertyHolder;
use crate::messages::prelude::*;
use crate::messages::tool::utility_types::{EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use graphene::intersection::Quad;
use graphene::layers::style::Fill;
use graphene::Operation;

use glam::DVec2;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct FillTool {
	fsm_state: FillToolFsmState,
	data: FillToolData,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Fill)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum FillToolMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,

	// Tool-specific messages
	LeftMouseDown,
	RightMouseDown,
}

impl ToolMetadata for FillTool {
	fn icon_name(&self) -> String {
		"GeneralFillTool".into()
	}
	fn tooltip(&self) -> String {
		"Fill Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Fill
	}
}

impl PropertyHolder for FillTool {}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for FillTool {
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

	advertise_actions!(FillToolMessageDiscriminant;
		LeftMouseDown,
		RightMouseDown,
	);
}

impl ToolTransition for FillTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			document_dirty: None,
			tool_abort: Some(FillToolMessage::Abort.into()),
			selection_changed: None,
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FillToolFsmState {
	Ready,
}

impl Default for FillToolFsmState {
	fn default() -> Self {
		FillToolFsmState::Ready
	}
}

#[derive(Clone, Debug, Default)]
struct FillToolData {}

impl Fsm for FillToolFsmState {
	type ToolData = FillToolData;
	type ToolOptions = ();

	fn transition(
		self,
		event: ToolMessage,
		_tool_data: &mut Self::ToolData,
		(document, _document_id, global_tool_data, input, font_cache): ToolActionHandlerData,
		_tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use FillToolFsmState::*;
		use FillToolMessage::*;

		if let ToolMessage::Fill(event) = event {
			match (self, event) {
				(Ready, lmb_or_rmb) if lmb_or_rmb == LeftMouseDown || lmb_or_rmb == RightMouseDown => {
					let mouse_pos = input.mouse.position;
					let tolerance = DVec2::splat(SELECTION_TOLERANCE);
					let quad = Quad::from_box([mouse_pos - tolerance, mouse_pos + tolerance]);

					if let Some(path) = document.graphene_document.intersects_quad_root(quad, font_cache).last() {
						let color = match lmb_or_rmb {
							LeftMouseDown => global_tool_data.primary_color,
							RightMouseDown => global_tool_data.secondary_color,
							Abort => unreachable!(),
						};
						let fill = Fill::Solid(color);

						responses.push_back(DocumentMessage::StartTransaction.into());
						responses.push_back(Operation::SetLayerFill { path: path.to_vec(), fill }.into());
						responses.push_back(DocumentMessage::CommitTransaction.into());
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
			FillToolFsmState::Ready => HintData(vec![HintGroup(vec![
				HintInfo {
					key_groups: vec![],
					key_groups_mac: None,
					mouse: Some(MouseMotion::Lmb),
					label: String::from("Fill with Primary"),
					plus: false,
				},
				HintInfo {
					key_groups: vec![],
					key_groups_mac: None,
					mouse: Some(MouseMotion::Rmb),
					label: String::from("Fill with Secondary"),
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

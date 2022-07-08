use crate::consts::SELECTION_TOLERANCE;
use crate::frontend::utility_types::MouseCursorIcon;
use crate::input::keyboard::MouseMotion;
use crate::layout::widgets::PropertyHolder;
use crate::message_prelude::*;
use crate::misc::{HintData, HintGroup, HintInfo};
use crate::viewport_tools::tool::{Fsm, SignalToMessageMap, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};

use graphene::intersection::Quad;
use graphene::Operation;

use glam::DVec2;
use graphene::layers::style::Fill;
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
		"Fill Tool (F)".into()
	}
	fn tool_type(&self) -> crate::viewport_tools::tool::ToolType {
		ToolType::Fill
	}
}

impl PropertyHolder for FillTool {}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for FillTool {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if action == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		if action == ToolMessage::UpdateCursor {
			self.fsm_state.update_cursor(responses);
			return;
		}

		let new_state = self.fsm_state.transition(action, &mut self.data, data, &(), responses);

		if self.fsm_state != new_state {
			self.fsm_state = new_state;
			self.fsm_state.update_hints(responses);
			self.fsm_state.update_cursor(responses);
		}
	}

	advertise_actions!(FillToolMessageDiscriminant; LeftMouseDown, RightMouseDown);
}

impl ToolTransition for FillTool {
	fn signal_to_message_map(&self) -> SignalToMessageMap {
		SignalToMessageMap {
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
		(document, global_tool_data, input, font_cache): ToolActionHandlerData,
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
					mouse: Some(MouseMotion::Lmb),
					label: String::from("Fill with Primary"),
					plus: false,
				},
				HintInfo {
					key_groups: vec![],
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

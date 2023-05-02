use crate::consts::SELECTION_TOLERANCE;
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::MouseMotion;
use crate::messages::layout::utility_types::layout_widget::PropertyHolder;
use crate::messages::prelude::*;
use crate::messages::tool::utility_types::{EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use document_legacy::intersection::Quad;
use document_legacy::layers::style::Fill;

use glam::DVec2;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct FillTool {
	fsm_state: FillToolFsmState,
	data: FillToolData,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Fill)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize, specta::Type)]
pub enum FillToolMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,

	// Tool-specific messages
	LeftPointerDown,
	RightPointerDown,
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

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for FillTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		self.fsm_state.process_event(message, &mut self.data, tool_data, &(), responses, true);
	}

	advertise_actions!(FillToolMessageDiscriminant;
		LeftPointerDown,
		RightPointerDown,
	);
}

impl ToolTransition for FillTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			document_dirty: None,
			tool_abort: Some(FillToolMessage::Abort.into()),
			selection_changed: None,
			..Default::default()
		}
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum FillToolFsmState {
	#[default]
	Ready,
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
		ToolActionHandlerData {
			document,
			global_tool_data,
			input,
			render_data,
			..
		}: &mut ToolActionHandlerData,
		_tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use FillToolFsmState::*;
		use FillToolMessage::*;

		if let ToolMessage::Fill(event) = event {
			match (self, event) {
				(Ready, lmb_or_rmb) if lmb_or_rmb == LeftPointerDown || lmb_or_rmb == RightPointerDown => {
					let mouse_pos = input.mouse.position;
					let tolerance = DVec2::splat(SELECTION_TOLERANCE);
					let quad = Quad::from_box([mouse_pos - tolerance, mouse_pos + tolerance]);

					if let Some(path) = document.document_legacy.intersects_quad_root(quad, render_data).last() {
						let color = match lmb_or_rmb {
							LeftPointerDown => global_tool_data.primary_color,
							RightPointerDown => global_tool_data.secondary_color,
							Abort => unreachable!(),
						};
						let fill = Fill::Solid(color);

						responses.add(DocumentMessage::StartTransaction);
						responses.add(DocumentMessage::SetSelectedLayers {
							replacement_selected_layers: vec![path.to_vec()],
						});
						responses.add(GraphOperationMessage::FillSet { layer: path.to_vec(), fill });
						responses.add(DocumentMessage::CommitTransaction);
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
				HintInfo::mouse(MouseMotion::Lmb, "Fill with Primary"),
				HintInfo::mouse(MouseMotion::Rmb, "Fill with Secondary"),
			])]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}

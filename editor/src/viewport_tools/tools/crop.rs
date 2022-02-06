use crate::document::DocumentMessageHandler;
use crate::frontend::utility_types::MouseCursorIcon;
use crate::input::InputPreprocessorMessageHandler;
use crate::layout::widgets::{IconButton, LayoutRow, PopoverButton, PropertyHolder, Separator, SeparatorDirection, SeparatorType, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::message_prelude::*;
use crate::misc::{HintData, HintGroup, HintInfo, KeysGroup};
use crate::viewport_tools::snapping::SnapHandler;
use crate::viewport_tools::tool::{DocumentToolData, Fsm, ToolActionHandlerData, ToolType};

use graphene::document::Document;
use graphene::intersection::Quad;
use graphene::layers::layer_info::LayerDataType;
use graphene::Operation;

use super::shared::transformation_cage::*;

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct Crop {
	fsm_state: CropToolFsmState,
	data: CropToolData,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Crop)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum CropMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,
	#[remain::unsorted]
	DocumentIsDirty,

	// Tool-specific messages
	MouseDown,
	MouseMove,
	MouseUp,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Crop {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if action == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		if action == ToolMessage::UpdateCursor {
			responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default }.into());
			return;
		}

		let new_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, &(), data.2, responses);

		if self.fsm_state != new_state {
			self.fsm_state = new_state;
			self.fsm_state.update_hints(responses);
		}
	}

	advertise_actions!(CropMessageDiscriminant; MouseMove, Abort);
}

impl PropertyHolder for Crop {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CropToolFsmState {
	Ready,
	Resizing,
}

impl Default for CropToolFsmState {
	fn default() -> Self {
		CropToolFsmState::Ready
	}
}

#[derive(Clone, Debug, Default)]
struct CropToolData {
	bounding_box_overlays: Option<BoundingBoxOverlays>,
	selected_board: Option<Vec<LayerId>>,
}

impl Fsm for CropToolFsmState {
	type ToolData = CropToolData;
	type ToolOptions = ();

	fn transition(
		self,
		event: ToolMessage,
		document: &DocumentMessageHandler,
		tool_data: &DocumentToolData,
		data: &mut Self::ToolData,
		_tool_options: &Self::ToolOptions,
		input: &InputPreprocessorMessageHandler,
		responses: &mut VecDeque<Message>,
	) -> Self {
		if let ToolMessage::Crop(event) = event {
			match (self, event) {
				_ => self,
			}
		} else {
			self
		}
	}
}

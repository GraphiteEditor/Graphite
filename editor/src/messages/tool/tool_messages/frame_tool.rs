use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, MouseMotion};
use crate::messages::layout::utility_types::layout_widget::PropertyHolder;
use crate::messages::portfolio::document::node_graph;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::resize::Resize;
use crate::messages::tool::utility_types::{EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use document_legacy::Operation;

use glam::DAffine2;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct FrameTool {
	fsm_state: NodeGraphToolFsmState,
	tool_data: NodeGraphToolData,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Frame)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize, specta::Type)]
pub enum FrameToolMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,

	// Tool-specific messages
	DragStart,
	DragStop,
	Resize {
		center: Key,
		lock_ratio: Key,
	},
}

impl PropertyHolder for FrameTool {}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for FrameTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		self.fsm_state.process_event(message, &mut self.tool_data, tool_data, &(), responses, true);
	}

	fn actions(&self) -> ActionList {
		use NodeGraphToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(FrameToolMessageDiscriminant;
				DragStart,
			),
			Drawing => actions!(FrameToolMessageDiscriminant;
				DragStop,
				Abort,
				Resize,
			),
		}
	}
}

impl ToolMetadata for FrameTool {
	fn icon_name(&self) -> String {
		"RasterFrameTool".into()
	}
	fn tooltip(&self) -> String {
		"Frame Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Frame
	}
}

impl ToolTransition for FrameTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			tool_abort: Some(FrameToolMessage::Abort.into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum NodeGraphToolFsmState {
	#[default]
	Ready,
	Drawing,
}

#[derive(Clone, Debug, Default)]
struct NodeGraphToolData {
	data: Resize,
}

impl Fsm for NodeGraphToolFsmState {
	type ToolData = NodeGraphToolData;
	type ToolOptions = ();

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		ToolActionHandlerData { document, input, render_data, .. }: &mut ToolActionHandlerData,
		_tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use FrameToolMessage::*;
		use NodeGraphToolFsmState::*;

		let mut shape_data = &mut tool_data.data;

		if let ToolMessage::Frame(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					shape_data.start(responses, document, input, render_data);
					responses.add(DocumentMessage::StartTransaction);
					shape_data.path = Some(document.get_path_for_new_layer());
					responses.add(DocumentMessage::DeselectAllLayers);

					let network = node_graph::new_image_network(8, 0);

					responses.add(Operation::AddFrame {
						path: shape_data.path.clone().unwrap(),
						insert_index: -1,
						transform: DAffine2::ZERO.to_cols_array(),
						network,
					});

					Drawing
				}
				(state, Resize { center, lock_ratio }) => {
					let message = shape_data.calculate_transform(responses, document, input, center, lock_ratio, true);
					responses.try_add(message);

					state
				}
				(Drawing, DragStop) => {
					if let Some(layer_path) = &shape_data.path {
						responses.add(DocumentMessage::InputFrameRasterizeRegionBelowLayer { layer_path: layer_path.to_vec() });
					}

					input.mouse.finish_transaction(shape_data.viewport_drag_start(document), responses);
					shape_data.cleanup(responses);

					Ready
				}
				(Drawing, Abort) => {
					responses.add(DocumentMessage::AbortTransaction);

					shape_data.cleanup(responses);

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
			NodeGraphToolFsmState::Ready => HintData(vec![HintGroup(vec![
				HintInfo::mouse(MouseMotion::LmbDrag, "Draw Repaint Frame"),
				HintInfo::keys([Key::Shift], "Constrain Square").prepend_plus(),
				HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
			])]),
			NodeGraphToolFsmState::Drawing => HintData(vec![HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain Square"), HintInfo::keys([Key::Alt], "From Center")])]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Crosshair });
	}
}

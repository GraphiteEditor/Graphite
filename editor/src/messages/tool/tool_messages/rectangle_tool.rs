use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, MouseMotion};
use crate::messages::layout::utility_types::layout_widget::PropertyHolder;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::resize::Resize;
use crate::messages::tool::utility_types::{EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use glam::DVec2;
use graphene_core::vector::style::Fill;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct RectangleTool {
	fsm_state: RectangleToolFsmState,
	tool_data: RectangleToolData,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Rectangle)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize, specta::Type)]
pub enum RectangleToolMessage {
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

impl PropertyHolder for RectangleTool {}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for RectangleTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		self.fsm_state.process_event(message, &mut self.tool_data, tool_data, &(), responses, true);
	}

	fn actions(&self) -> ActionList {
		use RectangleToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(RectangleToolMessageDiscriminant;
				DragStart,
			),
			Drawing => actions!(RectangleToolMessageDiscriminant;
				DragStop,
				Abort,
				Resize,
			),
		}
	}
}

impl ToolMetadata for RectangleTool {
	fn icon_name(&self) -> String {
		"VectorRectangleTool".into()
	}
	fn tooltip(&self) -> String {
		"Rectangle Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Rectangle
	}
}

impl ToolTransition for RectangleTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			document_dirty: None,
			tool_abort: Some(RectangleToolMessage::Abort.into()),
			selection_changed: None,
		}
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum RectangleToolFsmState {
	#[default]
	Ready,
	Drawing,
}

#[derive(Clone, Debug, Default)]
struct RectangleToolData {
	data: Resize,
}

impl Fsm for RectangleToolFsmState {
	type ToolData = RectangleToolData;
	type ToolOptions = ();

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
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
		use RectangleToolFsmState::*;
		use RectangleToolMessage::*;

		let mut shape_data = &mut tool_data.data;

		if let ToolMessage::Rectangle(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					shape_data.start(responses, document, input, render_data);

					let subpath = bezier_rs::Subpath::new_rect(DVec2::ZERO, DVec2::ONE);

					let layer_path = document.get_path_for_new_layer();
					responses.add(DocumentMessage::StartTransaction);
					shape_data.path = Some(layer_path.clone());
					graph_modification_utils::new_vector_layer(vec![subpath], layer_path.clone(), responses);
					responses.add(GraphOperationMessage::FillSet {
						layer: layer_path,
						fill: Fill::solid(global_tool_data.primary_color),
					});

					Drawing
				}
				(state, Resize { center, lock_ratio }) => {
					if let Some(message) = shape_data.calculate_transform(responses, document, input, center, lock_ratio, false) {
						responses.add(message);
					}

					state
				}
				(Drawing, DragStop) => {
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
			RectangleToolFsmState::Ready => HintData(vec![HintGroup(vec![
				HintInfo::mouse(MouseMotion::LmbDrag, "Draw Rectangle"),
				HintInfo::keys([Key::Shift], "Constrain Square").prepend_plus(),
				HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
			])]),
			RectangleToolFsmState::Drawing => HintData(vec![HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain Square"), HintInfo::keys([Key::Alt], "From Center")])]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Crosshair });
	}
}

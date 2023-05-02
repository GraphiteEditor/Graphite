use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, MouseMotion};
use crate::messages::layout::utility_types::layout_widget::PropertyHolder;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::resize::Resize;
use crate::messages::tool::utility_types::{EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use graphene_core::vector::style::Fill;

use glam::DVec2;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct EllipseTool {
	fsm_state: EllipseToolFsmState,
	data: EllipseToolData,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Ellipse)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize, specta::Type)]
pub enum EllipseToolMessage {
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

impl ToolMetadata for EllipseTool {
	fn icon_name(&self) -> String {
		"VectorEllipseTool".into()
	}
	fn tooltip(&self) -> String {
		"Ellipse Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Ellipse
	}
}

impl PropertyHolder for EllipseTool {}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for EllipseTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		self.fsm_state.process_event(message, &mut self.data, tool_data, &(), responses, true);
	}

	fn actions(&self) -> ActionList {
		use EllipseToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(EllipseToolMessageDiscriminant;
				DragStart,
			),
			Drawing => actions!(EllipseToolMessageDiscriminant;
				DragStop,
				Abort,
				Resize,
			),
		}
	}
}

impl ToolTransition for EllipseTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			tool_abort: Some(EllipseToolMessage::Abort.into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum EllipseToolFsmState {
	#[default]
	Ready,
	Drawing,
}

#[derive(Clone, Debug, Default)]
struct EllipseToolData {
	data: Resize,
}

impl Fsm for EllipseToolFsmState {
	type ToolData = EllipseToolData;
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
		use EllipseToolFsmState::*;
		use EllipseToolMessage::*;

		let mut shape_data = &mut tool_data.data;

		if let ToolMessage::Ellipse(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					shape_data.start(responses, document, input, render_data);
					responses.add(DocumentMessage::StartTransaction);

					// Create a new layer path for this shape
					let layer_path = document.get_path_for_new_layer();
					shape_data.path = Some(layer_path.clone());

					// Create a new ellipse vector shape
					let subpath = bezier_rs::Subpath::new_ellipse(DVec2::ZERO, DVec2::ONE);
					let manipulator_groups = subpath.manipulator_groups().to_vec();
					graph_modification_utils::new_vector_layer(vec![subpath], layer_path.clone(), responses);
					graph_modification_utils::set_manipulator_mirror_angle(&manipulator_groups, &layer_path, true, responses);

					// Set the fill color to the primary working color
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
			EllipseToolFsmState::Ready => HintData(vec![HintGroup(vec![
				HintInfo::mouse(MouseMotion::LmbDrag, "Draw Ellipse"),
				HintInfo::keys([Key::Shift], "Constrain Circular").prepend_plus(),
				HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
			])]),
			EllipseToolFsmState::Drawing => HintData(vec![HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain Circular"), HintInfo::keys([Key::Alt], "From Center")])]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Crosshair });
	}
}

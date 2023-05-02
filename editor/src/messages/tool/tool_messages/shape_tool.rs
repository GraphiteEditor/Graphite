use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, MouseMotion};
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, PropertyHolder, WidgetLayout};
use crate::messages::layout::utility_types::widgets::input_widgets::NumberInput;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::resize::Resize;
use crate::messages::tool::utility_types::{EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use glam::DVec2;
use graphene_core::vector::style::Fill;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct ShapeTool {
	fsm_state: ShapeToolFsmState,
	tool_data: ShapeToolData,
	options: ShapeOptions,
}

pub struct ShapeOptions {
	vertices: u32,
}

impl Default for ShapeOptions {
	fn default() -> Self {
		Self { vertices: 6 }
	}
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Shape)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize, specta::Type)]
pub enum ShapeToolMessage {
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
	UpdateOptions(ShapeOptionsUpdate),
}

#[remain::sorted]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize, specta::Type)]
pub enum ShapeOptionsUpdate {
	Vertices(u32),
}

impl ToolMetadata for ShapeTool {
	fn icon_name(&self) -> String {
		"VectorShapeTool".into()
	}
	fn tooltip(&self) -> String {
		"Shape Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Shape
	}
}

impl PropertyHolder for ShapeTool {
	fn properties(&self) -> Layout {
		let sides = NumberInput::new(Some(self.options.vertices as f64))
			.label("Sides")
			.int()
			.min(3.)
			.max(1000.)
			.mode(crate::messages::layout::utility_types::widget_prelude::NumberInputMode::Increment)
			.on_update(|number_input: &NumberInput| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::Vertices(number_input.value.unwrap() as u32)).into())
			.widget_holder();
		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets: vec![sides] }]))
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for ShapeTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		if let ToolMessage::Shape(ShapeToolMessage::UpdateOptions(action)) = message {
			match action {
				ShapeOptionsUpdate::Vertices(vertices) => self.options.vertices = vertices,
			}
			return;
		}

		self.fsm_state.process_event(message, &mut self.tool_data, tool_data, &self.options, responses, true);
	}

	fn actions(&self) -> ActionList {
		use ShapeToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(ShapeToolMessageDiscriminant;
				DragStart,
			),
			Drawing => actions!(ShapeToolMessageDiscriminant;
				DragStop,
				Abort,
				Resize,
			),
		}
	}
}

impl ToolTransition for ShapeTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			document_dirty: None,
			tool_abort: Some(ShapeToolMessage::Abort.into()),
			selection_changed: None,
			..Default::default()
		}
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ShapeToolFsmState {
	#[default]
	Ready,
	Drawing,
}

#[derive(Clone, Debug, Default)]
struct ShapeToolData {
	sides: u32,
	data: Resize,
}

impl Fsm for ShapeToolFsmState {
	type ToolData = ShapeToolData;
	type ToolOptions = ShapeOptions;

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
		tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use ShapeToolFsmState::*;
		use ShapeToolMessage::*;

		let mut shape_data = &mut tool_data.data;

		if let ToolMessage::Shape(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					shape_data.start(responses, document, input, render_data);
					responses.add(DocumentMessage::StartTransaction);
					let layer_path = document.get_path_for_new_layer();
					shape_data.path = Some(layer_path.clone());
					tool_data.sides = tool_options.vertices;

					let subpath = bezier_rs::Subpath::new_regular_polygon(DVec2::ZERO, tool_data.sides as u64, 1.);
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
			ShapeToolFsmState::Ready => HintData(vec![HintGroup(vec![
				HintInfo::mouse(MouseMotion::LmbDrag, "Draw Shape"),
				HintInfo::keys([Key::Shift], "Constrain 1:1 Aspect").prepend_plus(),
				HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
			])]),
			ShapeToolFsmState::Drawing => HintData(vec![HintGroup(vec![HintInfo::keys([Key::Shift], "Constrain 1:1 Aspect"), HintInfo::keys([Key::Alt], "From Center")])]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Crosshair });
	}
}

use super::shared::resize::Resize;
use crate::consts::DRAG_THRESHOLD;
use crate::frontend::utility_types::MouseCursorIcon;
use crate::input::keyboard::{Key, MouseMotion};
use crate::layout::widgets::{Layout, LayoutGroup, NumberInput, PropertyHolder, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::message_prelude::*;
use crate::misc::{HintData, HintGroup, HintInfo, KeysGroup};
use crate::viewport_tools::tool::{Fsm, SignalToMessageMap, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};

use graphene::layers::style;
use graphene::Operation;

use glam::DAffine2;
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
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize)]
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
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum ShapeOptionsUpdate {
	Vertices(u32),
}

impl ToolMetadata for ShapeTool {
	fn icon_name(&self) -> String {
		"VectorShapeTool".into()
	}
	fn tooltip(&self) -> String {
		"Shape Tool (Y)".into()
	}
	fn tool_type(&self) -> crate::viewport_tools::tool::ToolType {
		ToolType::Shape
	}
}

impl PropertyHolder for ShapeTool {
	fn properties(&self) -> Layout {
		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row {
			widgets: vec![WidgetHolder::new(Widget::NumberInput(NumberInput {
				label: "Sides".into(),
				value: Some(self.options.vertices as f64),
				is_integer: true,
				min: Some(3.),
				max: Some(1000.),
				on_update: WidgetCallback::new(|number_input: &NumberInput| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::Vertices(number_input.value.unwrap() as u32)).into()),
				..NumberInput::default()
			}))],
		}]))
	}
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for ShapeTool {
	fn process_action(&mut self, action: ToolMessage, tool_data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if action == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		if action == ToolMessage::UpdateCursor {
			self.fsm_state.update_cursor(responses);
			return;
		}

		if let ToolMessage::Shape(ShapeToolMessage::UpdateOptions(action)) = action {
			match action {
				ShapeOptionsUpdate::Vertices(vertices) => self.options.vertices = vertices,
			}
			return;
		}

		let new_state = self.fsm_state.transition(action, &mut self.tool_data, tool_data, &self.options, responses);

		if self.fsm_state != new_state {
			self.fsm_state = new_state;
			self.fsm_state.update_hints(responses);
			self.fsm_state.update_cursor(responses);
		}
	}

	fn actions(&self) -> ActionList {
		use ShapeToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(ShapeToolMessageDiscriminant; DragStart),
			Drawing => actions!(ShapeToolMessageDiscriminant; DragStop, Abort, Resize),
		}
	}
}

impl ToolTransition for ShapeTool {
	fn signal_to_message_map(&self) -> SignalToMessageMap {
		SignalToMessageMap {
			document_dirty: None,
			tool_abort: Some(ShapeToolMessage::Abort.into()),
			selection_changed: None,
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShapeToolFsmState {
	Ready,
	Drawing,
}

impl Default for ShapeToolFsmState {
	fn default() -> Self {
		ShapeToolFsmState::Ready
	}
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
		(document, global_tool_data, input, font_cache): ToolActionHandlerData,
		tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use ShapeToolFsmState::*;
		use ShapeToolMessage::*;

		let mut shape_data = &mut tool_data.data;

		if let ToolMessage::Shape(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					shape_data.start(responses, document, input.mouse.position, font_cache);
					responses.push_back(DocumentMessage::StartTransaction.into());
					shape_data.path = Some(document.get_path_for_new_layer());
					responses.push_back(DocumentMessage::DeselectAllLayers.into());
					tool_data.sides = tool_options.vertices;

					responses.push_back(
						Operation::AddNgon {
							path: shape_data.path.clone().unwrap(),
							insert_index: -1,
							transform: DAffine2::ZERO.to_cols_array(),
							sides: tool_data.sides,
							style: style::PathStyle::new(None, style::Fill::solid(global_tool_data.primary_color)),
						}
						.into(),
					);

					Drawing
				}
				(state, Resize { center, lock_ratio }) => {
					if let Some(message) = shape_data.calculate_transform(responses, document, center, lock_ratio, input) {
						responses.push_back(message);
					}

					state
				}
				(Drawing, DragStop) => {
					match shape_data.drag_start.distance(input.mouse.position) <= DRAG_THRESHOLD {
						true => responses.push_back(DocumentMessage::AbortTransaction.into()),
						false => responses.push_back(DocumentMessage::CommitTransaction.into()),
					}

					shape_data.cleanup(responses);

					Ready
				}
				(Drawing, Abort) => {
					responses.push_back(DocumentMessage::AbortTransaction.into());

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
				HintInfo {
					key_groups: vec![],
					mouse: Some(MouseMotion::LmbDrag),
					label: String::from("Draw Shape"),
					plus: false,
				},
				HintInfo {
					key_groups: vec![KeysGroup(vec![Key::KeyShift])],
					mouse: None,
					label: String::from("Constrain 1:1 Aspect"),
					plus: true,
				},
				HintInfo {
					key_groups: vec![KeysGroup(vec![Key::KeyAlt])],
					mouse: None,
					label: String::from("From Center"),
					plus: true,
				},
			])]),
			ShapeToolFsmState::Drawing => HintData(vec![HintGroup(vec![
				HintInfo {
					key_groups: vec![KeysGroup(vec![Key::KeyShift])],
					mouse: None,
					label: String::from("Constrain 1:1 Aspect"),
					plus: false,
				},
				HintInfo {
					key_groups: vec![KeysGroup(vec![Key::KeyAlt])],
					mouse: None,
					label: String::from("From Center"),
					plus: false,
				},
			])]),
		};

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Crosshair }.into());
	}
}

use super::shared::resize::Resize;
use crate::consts::DRAG_THRESHOLD;
use crate::document::DocumentMessageHandler;
use crate::frontend::utility_types::MouseCursorIcon;
use crate::input::keyboard::{Key, MouseMotion};
use crate::input::InputPreprocessorMessageHandler;
use crate::layout::widgets::{LayoutRow, NumberInput, PropertyHolder, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::message_prelude::*;
use crate::misc::{HintData, HintGroup, HintInfo, KeysGroup};
use crate::viewport_tools::tool::{DocumentToolData, Fsm, ToolActionHandlerData};

use graphene::layers::style;
use graphene::Operation;

use glam::DAffine2;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct ShapeTool {
	fsm_state: ShapeToolFsmState,
	data: ShapeToolData,
	options: ShapeOptions,
}

pub struct ShapeOptions {
	vertices: u8,
}

impl Default for ShapeOptions {
	fn default() -> Self {
		Self { vertices: 6 }
	}
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Shape)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
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
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum ShapeOptionsUpdate {
	Vertices(u8),
}

impl PropertyHolder for ShapeTool {
	fn properties(&self) -> WidgetLayout {
		WidgetLayout::new(vec![LayoutRow::Row {
			name: "".into(),
			widgets: vec![WidgetHolder::new(Widget::NumberInput(NumberInput {
				label: "Sides".into(),
				value: self.options.vertices as f64,
				is_integer: true,
				min: Some(3.),
				max: Some(256.),
				on_update: WidgetCallback::new(|number_input| ShapeToolMessage::UpdateOptions(ShapeOptionsUpdate::Vertices(number_input.value as u8)).into()),
				..NumberInput::default()
			}))],
		}])
	}
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for ShapeTool {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
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

		let new_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, &self.options, data.2, responses);

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
	sides: u8,
	data: Resize,
}

impl Fsm for ShapeToolFsmState {
	type ToolData = ShapeToolData;
	type ToolOptions = ShapeOptions;

	fn transition(
		self,
		event: ToolMessage,
		document: &DocumentMessageHandler,
		tool_data: &DocumentToolData,
		data: &mut Self::ToolData,
		tool_options: &Self::ToolOptions,
		input: &InputPreprocessorMessageHandler,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use ShapeToolFsmState::*;
		use ShapeToolMessage::*;

		let mut shape_data = &mut data.data;

		if let ToolMessage::Shape(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					shape_data.start(responses, input.viewport_bounds.size(), document, input.mouse.position);
					responses.push_back(DocumentMessage::StartTransaction.into());
					shape_data.path = Some(document.get_path_for_new_layer());
					responses.push_back(DocumentMessage::DeselectAllLayers.into());
					data.sides = tool_options.vertices;

					responses.push_back(
						Operation::AddNgon {
							path: shape_data.path.clone().unwrap(),
							insert_index: -1,
							transform: DAffine2::ZERO.to_cols_array(),
							sides: data.sides,
							style: style::PathStyle::new(None, Some(style::Fill::new(tool_data.primary_color))),
						}
						.into(),
					);

					Drawing
				}
				(state, Resize { center, lock_ratio }) => {
					if let Some(message) = shape_data.calculate_transform(responses, input.viewport_bounds.size(), document, center, lock_ratio, input) {
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

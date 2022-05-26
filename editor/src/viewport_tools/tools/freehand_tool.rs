use crate::frontend::utility_types::MouseCursorIcon;
use crate::input::keyboard::MouseMotion;
use crate::layout::widgets::{LayoutRow, NumberInput, PropertyHolder, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::message_prelude::*;
use crate::misc::{HintData, HintGroup, HintInfo};
use crate::viewport_tools::tool::{DocumentToolData, Fsm, ToolActionHandlerData};

use graphene::layers::style;
use graphene::Operation;

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct FreehandTool {
	fsm_state: FreehandToolFsmState,
	data: FreehandToolData,
	options: FreehandOptions,
}

pub struct FreehandOptions {
	line_weight: f64,
}

impl Default for FreehandOptions {
	fn default() -> Self {
		Self { line_weight: 5. }
	}
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Freehand)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum FreehandToolMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,

	// Tool-specific messages
	DragStart,
	DragStop,
	PointerMove,
	UpdateOptions(FreehandToolMessageOptionsUpdate),
}

#[remain::sorted]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum FreehandToolMessageOptionsUpdate {
	LineWeight(f64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FreehandToolFsmState {
	Ready,
	Drawing,
}

impl PropertyHolder for FreehandTool {
	fn properties(&self) -> WidgetLayout {
		WidgetLayout::new(vec![LayoutRow::Row {
			widgets: vec![WidgetHolder::new(Widget::NumberInput(NumberInput {
				unit: " px".into(),
				label: "Weight".into(),
				value: Some(self.options.line_weight as f64),
				is_integer: false,
				min: Some(1.),
				on_update: WidgetCallback::new(|number_input: &NumberInput| FreehandToolMessage::UpdateOptions(FreehandToolMessageOptionsUpdate::LineWeight(number_input.value.unwrap())).into()),
				..NumberInput::default()
			}))],
		}])
	}
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for FreehandTool {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if action == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		if action == ToolMessage::UpdateCursor {
			self.fsm_state.update_cursor(responses);
			return;
		}

		if let ToolMessage::Freehand(FreehandToolMessage::UpdateOptions(action)) = action {
			match action {
				FreehandToolMessageOptionsUpdate::LineWeight(line_weight) => self.options.line_weight = line_weight,
			}
			return;
		}

		let new_state = self.fsm_state.transition(action, &mut self.data, data, &self.options, responses);

		if self.fsm_state != new_state {
			self.fsm_state = new_state;
			self.fsm_state.update_hints(responses);
			self.fsm_state.update_cursor(responses);
		}
	}

	fn actions(&self) -> ActionList {
		use FreehandToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(FreehandToolMessageDiscriminant; DragStart, DragStop, Abort),
			Drawing => actions!(FreehandToolMessageDiscriminant; DragStop, PointerMove, Abort),
		}
	}
}

impl Default for FreehandToolFsmState {
	fn default() -> Self {
		FreehandToolFsmState::Ready
	}
}
#[derive(Clone, Debug, Default)]
struct FreehandToolData {
	points: Vec<DVec2>,
	weight: f64,
	path: Option<Vec<LayerId>>,
}

impl Fsm for FreehandToolFsmState {
	type ToolData = FreehandToolData;
	type ToolOptions = FreehandOptions;

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		(document, global_tool_data, input, _font_cache): ToolActionHandlerData,
		tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use FreehandToolFsmState::*;
		use FreehandToolMessage::*;

		let transform = document.graphene_document.root.transform;

		if let ToolMessage::Freehand(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					responses.push_back(DocumentMessage::StartTransaction.into());
					responses.push_back(DocumentMessage::DeselectAllLayers.into());
					tool_data.path = Some(document.get_path_for_new_layer());

					let pos = transform.inverse().transform_point2(input.mouse.position);

					tool_data.points.push(pos);

					tool_data.weight = tool_options.line_weight;

					responses.push_back(add_polyline(tool_data, global_tool_data));

					Drawing
				}
				(Drawing, PointerMove) => {
					let pos = transform.inverse().transform_point2(input.mouse.position);

					if tool_data.points.last() != Some(&pos) {
						tool_data.points.push(pos);
					}

					responses.push_back(remove_preview(tool_data));
					responses.push_back(add_polyline(tool_data, global_tool_data));

					Drawing
				}
				(Drawing, DragStop) | (Drawing, Abort) => {
					if tool_data.points.len() >= 2 {
						responses.push_back(DocumentMessage::DeselectAllLayers.into());
						responses.push_back(remove_preview(tool_data));
						responses.push_back(add_polyline(tool_data, global_tool_data));
						responses.push_back(DocumentMessage::CommitTransaction.into());
					} else {
						responses.push_back(DocumentMessage::AbortTransaction.into());
					}

					tool_data.path = None;
					tool_data.points.clear();

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
			FreehandToolFsmState::Ready => HintData(vec![HintGroup(vec![HintInfo {
				key_groups: vec![],
				mouse: Some(MouseMotion::LmbDrag),
				label: String::from("Draw Polyline"),
				plus: false,
			}])]),
			FreehandToolFsmState::Drawing => HintData(vec![]),
		};

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default }.into());
	}
}

fn remove_preview(data: &FreehandToolData) -> Message {
	Operation::DeleteLayer { path: data.path.clone().unwrap() }.into()
}

fn add_polyline(data: &FreehandToolData, tool_data: &DocumentToolData) -> Message {
	let points: Vec<(f64, f64)> = data.points.iter().map(|p| (p.x, p.y)).collect();

	Operation::AddPolyline {
		path: data.path.clone().unwrap(),
		insert_index: -1,
		transform: DAffine2::IDENTITY.to_cols_array(),
		points,
		style: style::PathStyle::new(Some(style::Stroke::new(tool_data.primary_color, data.weight)), style::Fill::None),
	}
	.into()
}

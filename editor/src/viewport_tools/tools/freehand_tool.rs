use crate::document::DocumentMessageHandler;
use crate::frontend::utility_types::MouseCursorIcon;
use crate::input::keyboard::MouseMotion;
use crate::input::InputPreprocessorMessageHandler;
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
	line_weight: u32,
}

impl Default for FreehandOptions {
	fn default() -> Self {
		Self { line_weight: 5 }
	}
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Freehand)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
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
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum FreehandToolMessageOptionsUpdate {
	LineWeight(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FreehandToolFsmState {
	Ready,
	Drawing,
}

impl PropertyHolder for FreehandTool {
	fn properties(&self) -> WidgetLayout {
		WidgetLayout::new(vec![LayoutRow::Row {
			name: "".into(),
			widgets: vec![WidgetHolder::new(Widget::NumberInput(NumberInput {
				unit: " px".into(),
				label: "Weight".into(),
				value: self.options.line_weight as f64,
				is_integer: true,
				min: Some(1.),
				on_update: WidgetCallback::new(|number_input| FreehandToolMessage::UpdateOptions(FreehandToolMessageOptionsUpdate::LineWeight(number_input.value as u32)).into()),
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

		let new_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, &self.options, data.2, responses);

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
	weight: u32,
	path: Option<Vec<LayerId>>,
}

impl Fsm for FreehandToolFsmState {
	type ToolData = FreehandToolData;
	type ToolOptions = FreehandOptions;

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
		use FreehandToolFsmState::*;
		use FreehandToolMessage::*;

		let transform = document.graphene_document.root.transform;

		if let ToolMessage::Freehand(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					responses.push_back(DocumentMessage::StartTransaction.into());
					responses.push_back(DocumentMessage::DeselectAllLayers.into());
					data.path = Some(document.get_path_for_new_layer());

					let pos = transform.inverse().transform_point2(input.mouse.position);

					data.points.push(pos);

					data.weight = tool_options.line_weight;

					responses.push_back(add_polyline(data, tool_data));

					Drawing
				}
				(Drawing, PointerMove) => {
					let pos = transform.inverse().transform_point2(input.mouse.position);

					if data.points.last() != Some(&pos) {
						data.points.push(pos);
					}

					responses.push_back(remove_preview(data));
					responses.push_back(add_polyline(data, tool_data));

					Drawing
				}
				(Drawing, DragStop) | (Drawing, Abort) => {
					if data.points.len() >= 2 {
						responses.push_back(DocumentMessage::DeselectAllLayers.into());
						responses.push_back(remove_preview(data));
						responses.push_back(add_polyline(data, tool_data));
						responses.push_back(DocumentMessage::CommitTransaction.into());
					} else {
						responses.push_back(DocumentMessage::AbortTransaction.into());
					}

					data.path = None;
					data.points.clear();

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
		style: style::PathStyle::new(Some(style::Stroke::new(tool_data.primary_color, data.weight as f32)), None),
	}
	.into()
}

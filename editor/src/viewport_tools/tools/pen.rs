use crate::document::DocumentMessageHandler;
use crate::frontend::utility_types::MouseCursorIcon;
use crate::input::keyboard::{Key, MouseMotion};
use crate::input::InputPreprocessorMessageHandler;
use crate::layout::widgets::{LayoutRow, NumberInput, PropertyHolder, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::message_prelude::*;
use crate::misc::{HintData, HintGroup, HintInfo, KeysGroup};
use crate::viewport_tools::snapping::SnapHandler;
use crate::viewport_tools::tool::{get_new_layer_location, DocumentToolData, Fsm, ToolActionHandlerData};

use graphene::layers::style;
use graphene::Operation;

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct Pen {
	fsm_state: PenToolFsmState,
	data: PenToolData,
	options: PenOptions,
}

pub struct PenOptions {
	line_weight: u32,
}

impl Default for PenOptions {
	fn default() -> Self {
		Self { line_weight: 5 }
	}
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Pen)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum PenMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,

	// Tool-specific messages
	Confirm,
	DragStart,
	DragStop,
	PointerMove,
	Undo,
	UpdateOptions(PenOptionsUpdate),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PenToolFsmState {
	Ready,
	Drawing,
}

#[remain::sorted]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum PenOptionsUpdate {
	LineWeight(u32),
}

impl PropertyHolder for Pen {
	fn properties(&self) -> WidgetLayout {
		WidgetLayout::new(vec![LayoutRow::Row {
			name: "".into(),
			widgets: vec![WidgetHolder::new(Widget::NumberInput(NumberInput {
				unit: " px".into(),
				label: "Weight".into(),
				value: self.options.line_weight as f64,
				is_integer: true,
				min: Some(0.),
				on_update: WidgetCallback::new(|number_input| PenMessage::UpdateOptions(PenOptionsUpdate::LineWeight(number_input.value as u32)).into()),
				..NumberInput::default()
			}))],
		}])
	}
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Pen {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if action == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		if action == ToolMessage::UpdateCursor {
			self.fsm_state.update_cursor(responses);
			return;
		}

		if let ToolMessage::Pen(PenMessage::UpdateOptions(action)) = action {
			match action {
				PenOptionsUpdate::LineWeight(line_weight) => self.options.line_weight = line_weight,
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
		use PenToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(PenMessageDiscriminant; Undo, DragStart, DragStop, Confirm, Abort),
			Drawing => actions!(PenMessageDiscriminant; DragStop, PointerMove, Confirm, Abort),
		}
	}
}

impl Default for PenToolFsmState {
	fn default() -> Self {
		PenToolFsmState::Ready
	}
}
#[derive(Clone, Debug, Default)]
struct PenToolData {
	points: Vec<DVec2>,
	next_point: DVec2,
	weight: u32,
	path: Option<Vec<LayerId>>,
	snap_handler: SnapHandler,
}

impl Fsm for PenToolFsmState {
	type ToolData = PenToolData;
	type ToolOptions = PenOptions;

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
		use PenMessage::*;
		use PenToolFsmState::*;

		let transform = document.graphene_document.root.transform;

		if let ToolMessage::Pen(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					responses.push_back(DocumentMessage::StartTransaction.into());
					responses.push_back(DocumentMessage::DeselectAllLayers.into());
					data.path = Some(get_new_layer_location(&document));

					data.snap_handler.start_snap(document, document.visible_layers(), true, true);
					let snapped_position = data.snap_handler.snap_position(responses, input.viewport_bounds.size(), document, input.mouse.position);

					let pos = transform.inverse().transform_point2(snapped_position);

					data.points.push(pos);
					data.next_point = pos;

					data.weight = tool_options.line_weight;

					responses.push_back(make_operation(data, tool_data, true));

					Drawing
				}
				(Drawing, DragStop) => {
					let snapped_position = data.snap_handler.snap_position(responses, input.viewport_bounds.size(), document, input.mouse.position);
					let pos = transform.inverse().transform_point2(snapped_position);

					// TODO: introduce comparison threshold when operating with canvas coordinates (https://github.com/GraphiteEditor/Graphite/issues/100)
					if data.points.last() != Some(&pos) {
						data.points.push(pos);
						data.next_point = pos;
					}

					responses.push_back(remove_preview(data));
					responses.push_back(make_operation(data, tool_data, true));

					Drawing
				}
				(Drawing, PointerMove) => {
					let snapped_position = data.snap_handler.snap_position(responses, input.viewport_bounds.size(), document, input.mouse.position);
					let pos = transform.inverse().transform_point2(snapped_position);
					data.next_point = pos;

					responses.push_back(remove_preview(data));
					responses.push_back(make_operation(data, tool_data, true));

					Drawing
				}
				(Drawing, Confirm) | (Drawing, Abort) => {
					if data.points.len() >= 2 {
						responses.push_back(DocumentMessage::DeselectAllLayers.into());
						responses.push_back(remove_preview(data));
						responses.push_back(make_operation(data, tool_data, false));
						responses.push_back(DocumentMessage::CommitTransaction.into());
					} else {
						responses.push_back(DocumentMessage::AbortTransaction.into());
					}

					data.path = None;
					data.points.clear();
					data.snap_handler.cleanup(responses);

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
			PenToolFsmState::Ready => HintData(vec![HintGroup(vec![HintInfo {
				key_groups: vec![],
				mouse: Some(MouseMotion::Lmb),
				label: String::from("Draw Path"),
				plus: false,
			}])]),
			PenToolFsmState::Drawing => HintData(vec![
				HintGroup(vec![HintInfo {
					key_groups: vec![],
					mouse: Some(MouseMotion::Lmb),
					label: String::from("Extend Path"),
					plus: false,
				}]),
				HintGroup(vec![HintInfo {
					key_groups: vec![KeysGroup(vec![Key::KeyEnter])],
					mouse: None,
					label: String::from("End Path"),
					plus: false,
				}]),
			]),
		};

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default }.into());
	}
}

fn remove_preview(data: &PenToolData) -> Message {
	Operation::DeleteLayer { path: data.path.clone().unwrap() }.into()
}

fn make_operation(data: &PenToolData, tool_data: &DocumentToolData, show_preview: bool) -> Message {
	let mut points: Vec<(f64, f64)> = data.points.iter().map(|p| (p.x, p.y)).collect();
	if show_preview {
		points.push((data.next_point.x, data.next_point.y))
	}

	Operation::AddPolyline {
		path: data.path.clone().unwrap(),
		insert_index: -1,
		transform: DAffine2::IDENTITY.to_cols_array(),
		points,
		style: style::PathStyle::new(Some(style::Stroke::new(tool_data.primary_color, data.weight as f32)), None),
	}
	.into()
}

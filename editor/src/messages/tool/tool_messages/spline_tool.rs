use crate::consts::DRAG_THRESHOLD;
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, KeysGroup, MouseMotion};
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, PropertyHolder, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::messages::layout::utility_types::widgets::input_widgets::NumberInput;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::snapping::SnapManager;
use crate::messages::tool::utility_types::{DocumentToolData, EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use graphene::layers::style;
use graphene::LayerId;
use graphene::Operation;

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct SplineTool {
	fsm_state: SplineToolFsmState,
	tool_data: SplineToolData,
	options: SplineOptions,
}

pub struct SplineOptions {
	line_weight: f64,
}

impl Default for SplineOptions {
	fn default() -> Self {
		Self { line_weight: 5. }
	}
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Spline)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum SplineToolMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,

	// Tool-specific messages
	Confirm,
	DragStart,
	DragStop,
	PointerMove,
	Undo,
	UpdateOptions(SplineOptionsUpdate),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SplineToolFsmState {
	Ready,
	Drawing,
}

#[remain::sorted]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum SplineOptionsUpdate {
	LineWeight(f64),
}

impl ToolMetadata for SplineTool {
	fn icon_name(&self) -> String {
		"VectorSplineTool".into()
	}
	fn tooltip(&self) -> String {
		"Spline Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Spline
	}
}

impl PropertyHolder for SplineTool {
	fn properties(&self) -> Layout {
		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row {
			widgets: vec![WidgetHolder::new(Widget::NumberInput(NumberInput {
				unit: " px".into(),
				label: "Weight".into(),
				value: Some(self.options.line_weight),
				is_integer: false,
				min: Some(0.),
				on_update: WidgetCallback::new(|number_input: &NumberInput| SplineToolMessage::UpdateOptions(SplineOptionsUpdate::LineWeight(number_input.value.unwrap())).into()),
				..NumberInput::default()
			}))],
		}]))
	}
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for SplineTool {
	fn process_message(&mut self, message: ToolMessage, tool_data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if message == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		if message == ToolMessage::UpdateCursor {
			self.fsm_state.update_cursor(responses);
			return;
		}

		if let ToolMessage::Spline(SplineToolMessage::UpdateOptions(action)) = message {
			match action {
				SplineOptionsUpdate::LineWeight(line_weight) => self.options.line_weight = line_weight,
			}
			return;
		}

		let new_state = self.fsm_state.transition(message, &mut self.tool_data, tool_data, &self.options, responses);

		if self.fsm_state != new_state {
			self.fsm_state = new_state;
			self.fsm_state.update_hints(responses);
			self.fsm_state.update_cursor(responses);
		}
	}

	fn actions(&self) -> ActionList {
		use SplineToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(SplineToolMessageDiscriminant;
				Undo,
				DragStart,
				DragStop,
				Confirm,
				Abort,
			),
			Drawing => actions!(SplineToolMessageDiscriminant;
				DragStop,
				PointerMove,
				Confirm,
				Abort,
			),
		}
	}
}

impl ToolTransition for SplineTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			document_dirty: None,
			tool_abort: Some(SplineToolMessage::Abort.into()),
			selection_changed: None,
		}
	}
}

impl Default for SplineToolFsmState {
	fn default() -> Self {
		SplineToolFsmState::Ready
	}
}
#[derive(Clone, Debug, Default)]
struct SplineToolData {
	points: Vec<DVec2>,
	next_point: DVec2,
	weight: f64,
	path: Option<Vec<LayerId>>,
	snap_manager: SnapManager,
}

impl Fsm for SplineToolFsmState {
	type ToolData = SplineToolData;
	type ToolOptions = SplineOptions;

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		(document, global_tool_data, input, font_cache): ToolActionHandlerData,
		tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use SplineToolFsmState::*;
		use SplineToolMessage::*;

		let transform = document.graphene_document.root.transform;

		if let ToolMessage::Spline(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					responses.push_back(DocumentMessage::StartTransaction.into());
					responses.push_back(DocumentMessage::DeselectAllLayers.into());
					tool_data.path = Some(document.get_path_for_new_layer());

					tool_data.snap_manager.start_snap(document, document.bounding_boxes(None, None, font_cache), true, true);
					tool_data.snap_manager.add_all_document_handles(document, &[], &[], &[]);
					let snapped_position = tool_data.snap_manager.snap_position(responses, document, input.mouse.position);

					let pos = transform.inverse().transform_point2(snapped_position);

					tool_data.points.push(pos);
					tool_data.next_point = pos;

					tool_data.weight = tool_options.line_weight;

					responses.push_back(add_spline(tool_data, global_tool_data, true));

					Drawing
				}
				(Drawing, DragStop) => {
					let snapped_position = tool_data.snap_manager.snap_position(responses, document, input.mouse.position);
					let pos = transform.inverse().transform_point2(snapped_position);

					if let Some(last_pos) = tool_data.points.last() {
						if last_pos.distance(pos) > DRAG_THRESHOLD {
							tool_data.points.push(pos);
							tool_data.next_point = pos;
						}
					}

					responses.push_back(remove_preview(tool_data));
					responses.push_back(add_spline(tool_data, global_tool_data, true));

					Drawing
				}
				(Drawing, PointerMove) => {
					let snapped_position = tool_data.snap_manager.snap_position(responses, document, input.mouse.position);
					let pos = transform.inverse().transform_point2(snapped_position);
					tool_data.next_point = pos;

					responses.push_back(remove_preview(tool_data));
					responses.push_back(add_spline(tool_data, global_tool_data, true));

					Drawing
				}
				(Drawing, Confirm) | (Drawing, Abort) => {
					if tool_data.points.len() >= 2 {
						responses.push_back(remove_preview(tool_data));
						responses.push_back(add_spline(tool_data, global_tool_data, false));
						responses.push_back(DocumentMessage::CommitTransaction.into());
					} else {
						responses.push_back(DocumentMessage::AbortTransaction.into());
					}

					tool_data.path = None;
					tool_data.points.clear();
					tool_data.snap_manager.cleanup(responses);

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
			SplineToolFsmState::Ready => HintData(vec![HintGroup(vec![HintInfo {
				key_groups: vec![],
				key_groups_mac: None,
				mouse: Some(MouseMotion::Lmb),
				label: String::from("Draw Spline"),
				plus: false,
			}])]),
			SplineToolFsmState::Drawing => HintData(vec![
				HintGroup(vec![HintInfo {
					key_groups: vec![],
					key_groups_mac: None,
					mouse: Some(MouseMotion::Lmb),
					label: String::from("Extend Spline"),
					plus: false,
				}]),
				HintGroup(vec![HintInfo {
					key_groups: vec![KeysGroup(vec![Key::Enter])],
					key_groups_mac: None,
					mouse: None,
					label: String::from("End Spline"),
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

fn remove_preview(tool_data: &SplineToolData) -> Message {
	Operation::DeleteLayer {
		path: tool_data.path.clone().unwrap(),
	}
	.into()
}

fn add_spline(tool_data: &SplineToolData, global_tool_data: &DocumentToolData, show_preview: bool) -> Message {
	let mut points: Vec<(f64, f64)> = tool_data.points.iter().map(|p| (p.x, p.y)).collect();
	if show_preview {
		points.push((tool_data.next_point.x, tool_data.next_point.y))
	}

	Operation::AddSpline {
		path: tool_data.path.clone().unwrap(),
		insert_index: -1,
		transform: DAffine2::IDENTITY.to_cols_array(),
		points,
		style: style::PathStyle::new(Some(style::Stroke::new(global_tool_data.primary_color, tool_data.weight)), style::Fill::None),
	}
	.into()
}

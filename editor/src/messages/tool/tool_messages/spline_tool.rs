use crate::consts::DRAG_THRESHOLD;
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, MouseMotion};
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, PropertyHolder, WidgetLayout};
use crate::messages::layout::utility_types::widgets::input_widgets::NumberInput;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::snapping::SnapManager;
use crate::messages::tool::utility_types::{DocumentToolData, EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use document_legacy::{LayerId, Operation};
use graphene_core::vector::style::Stroke;

use glam::DVec2;
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
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SplineToolFsmState {
	#[default]
	Ready,
	Drawing,
}

#[remain::sorted]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
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
		let weight = NumberInput::new(Some(self.options.line_weight))
			.unit(" px")
			.label("Weight")
			.min(0.)
			.on_update(|number_input: &NumberInput| SplineToolMessage::UpdateOptions(SplineOptionsUpdate::LineWeight(number_input.value.unwrap())).into())
			.widget_holder();
		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets: vec![weight] }]))
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for SplineTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		if let ToolMessage::Spline(SplineToolMessage::UpdateOptions(action)) = message {
			match action {
				SplineOptionsUpdate::LineWeight(line_weight) => self.options.line_weight = line_weight,
			}
			return;
		}

		self.fsm_state.process_event(message, &mut self.tool_data, tool_data, &self.options, responses, true);
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
		use SplineToolFsmState::*;
		use SplineToolMessage::*;

		let transform = document.document_legacy.root.transform;

		if let ToolMessage::Spline(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					responses.push_back(DocumentMessage::StartTransaction.into());
					responses.push_back(DocumentMessage::DeselectAllLayers.into());
					tool_data.path = Some(document.get_path_for_new_layer());

					tool_data.snap_manager.start_snap(document, input, document.bounding_boxes(None, None, render_data), true, true);
					tool_data.snap_manager.add_all_document_handles(document, input, &[], &[], &[]);
					let snapped_position = tool_data.snap_manager.snap_position(responses, document, input.mouse.position);

					let pos = transform.inverse().transform_point2(snapped_position);

					tool_data.points.push(pos);
					tool_data.next_point = pos;

					tool_data.weight = tool_options.line_weight;

					add_spline(tool_data, global_tool_data, true, responses);

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
					add_spline(tool_data, global_tool_data, true, responses);

					Drawing
				}
				(Drawing, PointerMove) => {
					let snapped_position = tool_data.snap_manager.snap_position(responses, document, input.mouse.position);
					let pos = transform.inverse().transform_point2(snapped_position);
					tool_data.next_point = pos;

					responses.push_back(remove_preview(tool_data));
					add_spline(tool_data, global_tool_data, true, responses);

					Drawing
				}
				(Drawing, Confirm) | (Drawing, Abort) => {
					if tool_data.points.len() >= 2 {
						responses.push_back(remove_preview(tool_data));
						add_spline(tool_data, global_tool_data, false, responses);
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
			SplineToolFsmState::Ready => HintData(vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Draw Spline")])]),
			SplineToolFsmState::Drawing => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Extend Spline")]),
				HintGroup(vec![HintInfo::keys([Key::Enter], "End Spline")]),
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

fn add_spline(tool_data: &SplineToolData, global_tool_data: &DocumentToolData, show_preview: bool, responses: &mut VecDeque<Message>) {
	let mut points = tool_data.points.clone();
	if show_preview {
		points.push(tool_data.next_point)
	}

	let subpath = bezier_rs::Subpath::new_cubic_spline(points);
	let position = subpath.bounding_box().unwrap_or_default().into_iter().sum::<DVec2>() / 2.;

	let layer_path = tool_data.path.clone().unwrap();
	graph_modification_utils::new_vector_layer(vec![subpath], layer_path.clone(), responses);
	responses.add(GraphOperationMessage::StrokeSet {
		layer: layer_path.clone(),
		stroke: Stroke::new(global_tool_data.primary_color, tool_data.weight),
	});
}

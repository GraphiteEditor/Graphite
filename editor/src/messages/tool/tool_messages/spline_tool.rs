use crate::consts::DRAG_THRESHOLD;
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, MouseMotion};
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, PropertyHolder, WidgetCallback, WidgetLayout};
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::layout::utility_types::widget_prelude::{ColorInput, WidgetHolder};
use crate::messages::layout::utility_types::widgets::input_widgets::NumberInput;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::snapping::SnapManager;
use crate::messages::tool::utility_types::{EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use document_legacy::{LayerId, Operation};
use graphene_core::vector::style::{Fill, Stroke};
use graphene_core::Color;

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
	fill: ToolColorOptions,
	stroke: ToolColorOptions,
}

impl Default for SplineOptions {
	fn default() -> Self {
		Self {
			line_weight: 5.,
			fill: ToolColorOptions::new_none(),
			stroke: ToolColorOptions::new_primary(),
		}
	}
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Spline)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub enum SplineToolMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,
	#[remain::unsorted]
	WorkingColorChanged,

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
	FillColor(Option<Color>),
	FillColorType(ToolColorType),
	LineWeight(f64),
	StrokeColor(Option<Color>),
	StrokeColorType(ToolColorType),
	WorkingColors(Option<Color>, Option<Color>),
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

fn create_weight_widget(line_weight: f64) -> WidgetHolder {
	NumberInput::new(Some(line_weight))
		.unit(" px")
		.label("Weight")
		.min(0.)
		.on_update(|number_input: &NumberInput| SplineToolMessage::UpdateOptions(SplineOptionsUpdate::LineWeight(number_input.value.unwrap())).into())
		.widget_holder()
}

impl PropertyHolder for SplineTool {
	fn properties(&self) -> Layout {
		let mut widgets = self.options.fill.create_widgets(
			"Fill",
			true,
			WidgetCallback::new(|_| SplineToolMessage::UpdateOptions(SplineOptionsUpdate::FillColor(None)).into()),
			|color_type: ToolColorType| WidgetCallback::new(move |_| SplineToolMessage::UpdateOptions(SplineOptionsUpdate::FillColorType(color_type.clone())).into()),
			WidgetCallback::new(|color: &ColorInput| SplineToolMessage::UpdateOptions(SplineOptionsUpdate::FillColor(color.value)).into()),
		);

		widgets.push(WidgetHolder::section_separator());

		widgets.append(&mut self.options.stroke.create_widgets(
			"Stroke",
			true,
			WidgetCallback::new(|_| SplineToolMessage::UpdateOptions(SplineOptionsUpdate::StrokeColor(None)).into()),
			|color_type: ToolColorType| WidgetCallback::new(move |_| SplineToolMessage::UpdateOptions(SplineOptionsUpdate::StrokeColorType(color_type.clone())).into()),
			WidgetCallback::new(|color: &ColorInput| SplineToolMessage::UpdateOptions(SplineOptionsUpdate::StrokeColor(color.value)).into()),
		));
		widgets.push(WidgetHolder::unrelated_separator());
		widgets.push(create_weight_widget(self.options.line_weight));

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for SplineTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		if let ToolMessage::Spline(SplineToolMessage::UpdateOptions(action)) = message {
			match action {
				SplineOptionsUpdate::LineWeight(line_weight) => self.options.line_weight = line_weight,
				SplineOptionsUpdate::FillColor(color) => {
					self.options.fill.custom_color = color;
					self.options.fill.color_type = ToolColorType::Custom;
				}
				SplineOptionsUpdate::FillColorType(color_type) => self.options.fill.color_type = color_type,
				SplineOptionsUpdate::StrokeColor(color) => {
					self.options.stroke.custom_color = color;
					self.options.stroke.color_type = ToolColorType::Custom;
				}
				SplineOptionsUpdate::StrokeColorType(color_type) => self.options.stroke.color_type = color_type,
				SplineOptionsUpdate::WorkingColors(primary, secondary) => {
					self.options.stroke.primary_working_color = primary;
					self.options.stroke.secondary_working_color = secondary;
					self.options.fill.primary_working_color = primary;
					self.options.fill.secondary_working_color = secondary;
				}
			}

			responses.add(LayoutMessage::SendLayout {
				layout: self.properties(),
				layout_target: LayoutTarget::ToolOptions,
			});

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
			tool_abort: Some(SplineToolMessage::Abort.into()),
			working_color_changed: Some(SplineToolMessage::WorkingColorChanged.into()),
			..Default::default()
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
					responses.add(DocumentMessage::StartTransaction);
					responses.add(DocumentMessage::DeselectAllLayers);
					tool_data.path = Some(document.get_path_for_new_layer());

					tool_data.snap_manager.start_snap(document, input, document.bounding_boxes(None, None, render_data), true, true);
					tool_data.snap_manager.add_all_document_handles(document, input, &[], &[], &[]);
					let snapped_position = tool_data.snap_manager.snap_position(responses, document, input.mouse.position);

					let pos = transform.inverse().transform_point2(snapped_position);

					tool_data.points.push(pos);
					tool_data.next_point = pos;

					tool_data.weight = tool_options.line_weight;

					add_spline(tool_data, true, tool_options.fill.active_color(), tool_options.stroke.active_color(), responses);

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

					responses.add(remove_preview(tool_data));
					add_spline(tool_data, true, tool_options.fill.active_color(), tool_options.stroke.active_color(), responses);

					Drawing
				}
				(Drawing, PointerMove) => {
					let snapped_position = tool_data.snap_manager.snap_position(responses, document, input.mouse.position);
					let pos = transform.inverse().transform_point2(snapped_position);
					tool_data.next_point = pos;

					responses.add(remove_preview(tool_data));
					add_spline(tool_data, true, tool_options.fill.active_color(), tool_options.stroke.active_color(), responses);

					Drawing
				}
				(Drawing, Confirm) | (Drawing, Abort) => {
					if tool_data.points.len() >= 2 {
						responses.add(remove_preview(tool_data));
						add_spline(tool_data, false, tool_options.fill.active_color(), tool_options.stroke.active_color(), responses);
						responses.add(DocumentMessage::CommitTransaction);
					} else {
						responses.add(DocumentMessage::AbortTransaction);
					}

					tool_data.path = None;
					tool_data.points.clear();
					tool_data.snap_manager.cleanup(responses);

					Ready
				}
				(_, WorkingColorChanged) => {
					responses.add(SplineToolMessage::UpdateOptions(SplineOptionsUpdate::WorkingColors(
						Some(global_tool_data.primary_color),
						Some(global_tool_data.secondary_color),
					)));
					self
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

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}

fn remove_preview(tool_data: &SplineToolData) -> Message {
	Operation::DeleteLayer {
		path: tool_data.path.clone().unwrap(),
	}
	.into()
}

fn add_spline(tool_data: &SplineToolData, show_preview: bool, fill_color: Option<Color>, stroke_color: Option<Color>, responses: &mut VecDeque<Message>) {
	let mut points = tool_data.points.clone();
	if show_preview {
		points.push(tool_data.next_point)
	}

	let subpath = bezier_rs::Subpath::new_cubic_spline(points);

	let layer_path = tool_data.path.clone().unwrap();
	let manipulator_groups = subpath.manipulator_groups().to_vec();
	graph_modification_utils::new_vector_layer(vec![subpath], layer_path.clone(), responses);
	graph_modification_utils::set_manipulator_mirror_angle(&manipulator_groups, &layer_path, true, responses);

	responses.add(GraphOperationMessage::FillSet {
		layer: layer_path.clone(),
		fill: if let Some(color) = fill_color { Fill::Solid(color) } else { Fill::None },
	});

	responses.add(GraphOperationMessage::StrokeSet {
		layer: layer_path.clone(),
		stroke: Stroke::new(stroke_color, tool_data.weight),
	});
}

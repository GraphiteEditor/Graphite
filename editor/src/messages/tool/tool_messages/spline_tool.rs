use super::tool_prelude::*;
use crate::consts::{DEFAULT_STROKE_WIDTH, DRAG_THRESHOLD};
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::snapping::SnapManager;

use graph_craft::document::{value::TaggedValue, NodeId, NodeInput};
use graphene_core::Color;

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
			line_weight: DEFAULT_STROKE_WIDTH,
			fill: ToolColorOptions::new_none(),
			stroke: ToolColorOptions::new_primary(),
		}
	}
}

#[impl_message(Message, ToolMessage, Spline)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum SplineToolMessage {
	// Standard messages
	CanvasTransformed,
	Abort,
	WorkingColorChanged,

	// Tool-specific messages
	Confirm,
	DragStart,
	DragStop,
	PointerMove,
	PointerOutsideViewport,
	Undo,
	UpdateOptions(SplineOptionsUpdate),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SplineToolFsmState {
	#[default]
	Ready,
	Drawing,
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
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
		.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
		.on_update(|number_input: &NumberInput| SplineToolMessage::UpdateOptions(SplineOptionsUpdate::LineWeight(number_input.value.unwrap())).into())
		.widget_holder()
}

impl LayoutHolder for SplineTool {
	fn layout(&self) -> Layout {
		let mut widgets = self.options.fill.create_widgets(
			"Fill",
			true,
			|_| SplineToolMessage::UpdateOptions(SplineOptionsUpdate::FillColor(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| SplineToolMessage::UpdateOptions(SplineOptionsUpdate::FillColorType(color_type.clone())).into()),
			|color: &ColorButton| SplineToolMessage::UpdateOptions(SplineOptionsUpdate::FillColor(color.value.as_solid())).into(),
		);

		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());

		widgets.append(&mut self.options.stroke.create_widgets(
			"Stroke",
			true,
			|_| SplineToolMessage::UpdateOptions(SplineOptionsUpdate::StrokeColor(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| SplineToolMessage::UpdateOptions(SplineOptionsUpdate::StrokeColorType(color_type.clone())).into()),
			|color: &ColorButton| SplineToolMessage::UpdateOptions(SplineOptionsUpdate::StrokeColor(color.value.as_solid())).into(),
		));
		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		widgets.push(create_weight_widget(self.options.line_weight));

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for SplineTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		let ToolMessage::Spline(SplineToolMessage::UpdateOptions(action)) = message else {
			self.fsm_state.process_event(message, &mut self.tool_data, tool_data, &self.options, responses, true);
			return;
		};
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

		self.send_layout(responses, LayoutTarget::ToolOptions);
	}

	fn actions(&self) -> ActionList {
		match self.fsm_state {
			SplineToolFsmState::Ready => actions!(SplineToolMessageDiscriminant;
				Undo,
				DragStart,
				DragStop,
				Confirm,
				Abort,
			),
			SplineToolFsmState::Drawing => actions!(SplineToolMessageDiscriminant;
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
			canvas_transformed: Some(SplineToolMessage::CanvasTransformed.into()),
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
	layer: Option<LayerNodeIdentifier>,
	snap_manager: SnapManager,
	auto_panning: AutoPanning,
}

impl Fsm for SplineToolFsmState {
	type ToolData = SplineToolData;
	type ToolOptions = SplineOptions;

	fn transition(self, event: ToolMessage, tool_data: &mut Self::ToolData, tool_action_data: &mut ToolActionHandlerData, tool_options: &Self::ToolOptions, responses: &mut VecDeque<Message>) -> Self {
		let ToolActionHandlerData {
			document, global_tool_data, input, ..
		} = tool_action_data;

		let ToolMessage::Spline(event) = event else {
			return self;
		};
		match (self, event) {
			(_, SplineToolMessage::CanvasTransformed) => self,
			(SplineToolFsmState::Ready, SplineToolMessage::DragStart) => {
				responses.add(DocumentMessage::StartTransaction);
				responses.add(DocumentMessage::DeselectAllLayers);

				let parent = document.new_layer_parent(true);

				tool_data.weight = tool_options.line_weight;

				let node_type = resolve_document_node_type("Spline").expect("Spline node does not exist");
				let node = node_type.node_template_input_override([None, Some(NodeInput::value(TaggedValue::VecDVec2(Vec::new()), false))]);
				let nodes = vec![(NodeId(0), node)];

				let layer = graph_modification_utils::new_custom(NodeId::new(), nodes, parent, responses);
				tool_options.fill.apply_fill(layer, responses);
				tool_options.stroke.apply_stroke(tool_data.weight, layer, responses);
				tool_data.layer = Some(layer);

				responses.add(Message::StartBuffer);

				SplineToolFsmState::Drawing
			}
			(SplineToolFsmState::Drawing, SplineToolMessage::DragStop) => {
				responses.add(DocumentMessage::EndTransaction);

				let Some(layer) = tool_data.layer else {
					return SplineToolFsmState::Ready;
				};
				let snapped_position = input.mouse.position;
				let transform = document.metadata().transform_to_viewport(layer);
				let pos = transform.inverse().transform_point2(snapped_position);

				if tool_data.points.last().map_or(true, |last_pos| last_pos.distance(pos) > DRAG_THRESHOLD) {
					tool_data.points.push(pos);
					tool_data.next_point = pos;
				}

				update_spline(document, tool_data, true, responses);

				SplineToolFsmState::Drawing
			}
			(SplineToolFsmState::Drawing, SplineToolMessage::PointerMove) => {
				let Some(layer) = tool_data.layer else {
					return SplineToolFsmState::Ready;
				};
				let snapped_position = input.mouse.position; // tool_data.snap_manager.snap_position(responses, document, input.mouse.position);
				let transform = document.metadata().transform_to_viewport(layer);
				let pos = transform.inverse().transform_point2(snapped_position);
				tool_data.next_point = pos;

				update_spline(document, tool_data, true, responses);

				// Auto-panning
				let messages = [SplineToolMessage::PointerOutsideViewport.into(), SplineToolMessage::PointerMove.into()];
				tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);

				SplineToolFsmState::Drawing
			}
			(SplineToolFsmState::Drawing, SplineToolMessage::PointerOutsideViewport) => {
				// Auto-panning
				let _ = tool_data.auto_panning.shift_viewport(input, responses);

				SplineToolFsmState::Drawing
			}
			(state, SplineToolMessage::PointerOutsideViewport) => {
				// Auto-panning
				let messages = [SplineToolMessage::PointerOutsideViewport.into(), SplineToolMessage::PointerMove.into()];
				tool_data.auto_panning.stop(&messages, responses);

				state
			}
			(SplineToolFsmState::Drawing, SplineToolMessage::Confirm | SplineToolMessage::Abort) => {
				if tool_data.points.len() >= 2 {
					update_spline(document, tool_data, false, responses);
					responses.add(DocumentMessage::EndTransaction);
				} else {
					responses.add(DocumentMessage::AbortTransaction);
				}

				tool_data.layer = None;
				tool_data.points.clear();
				tool_data.snap_manager.cleanup(responses);

				SplineToolFsmState::Ready
			}
			(_, SplineToolMessage::WorkingColorChanged) => {
				responses.add(SplineToolMessage::UpdateOptions(SplineOptionsUpdate::WorkingColors(
					Some(global_tool_data.primary_color),
					Some(global_tool_data.secondary_color),
				)));
				self
			}
			_ => self,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			SplineToolFsmState::Ready => HintData(vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Draw Spline")])]),
			SplineToolFsmState::Drawing => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
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

fn update_spline(document: &DocumentMessageHandler, tool_data: &SplineToolData, show_preview: bool, responses: &mut VecDeque<Message>) {
	let mut points = tool_data.points.clone();
	if show_preview {
		points.push(tool_data.next_point)
	}
	let value = TaggedValue::VecDVec2(points);

	let Some(layer) = tool_data.layer else { return };

	let Some(node_id) = graph_modification_utils::NodeGraphLayer::new(layer, &document.network_interface).upstream_node_id_from_name("Spline") else {
		return;
	};
	responses.add_front(NodeGraphMessage::SetInputValue { node_id, input_index: 1, value });
}

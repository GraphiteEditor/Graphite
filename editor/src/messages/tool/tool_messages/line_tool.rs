use super::tool_prelude::*;
use crate::consts::LINE_ROTATE_SNAP_ANGLE;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::snapping::SnapManager;

use graphene_core::uuid::generate_uuid;
use graphene_core::vector::style::Stroke;
use graphene_core::Color;

#[derive(Default)]
pub struct LineTool {
	fsm_state: LineToolFsmState,
	tool_data: LineToolData,
	options: LineOptions,
}

pub struct LineOptions {
	line_weight: f64,
	stroke: ToolColorOptions,
}

impl Default for LineOptions {
	fn default() -> Self {
		Self {
			line_weight: 5.,
			stroke: ToolColorOptions::new_primary(),
		}
	}
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Line)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub enum LineToolMessage {
	// Standard messages
	#[remain::unsorted]
	DocumentIsDirty,
	#[remain::unsorted]
	CanvasTransformed,
	#[remain::unsorted]
	Abort,
	#[remain::unsorted]
	WorkingColorChanged,

	// Tool-specific messages
	DragStart,
	DragStop,
	Redraw {
		center: Key,
		lock_angle: Key,
		snap_angle: Key,
	},
	UpdateOptions(LineOptionsUpdate),
}

#[remain::sorted]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub enum LineOptionsUpdate {
	LineWeight(f64),
	StrokeColor(Option<Color>),
	StrokeColorType(ToolColorType),
	WorkingColors(Option<Color>, Option<Color>),
}

impl ToolMetadata for LineTool {
	fn icon_name(&self) -> String {
		"VectorLineTool".into()
	}
	fn tooltip(&self) -> String {
		"Line Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Line
	}
}

fn create_weight_widget(line_weight: f64) -> WidgetHolder {
	NumberInput::new(Some(line_weight))
		.unit(" px")
		.label("Weight")
		.min(0.)
		.max((1u64 << std::f64::MANTISSA_DIGITS) as f64)
		.on_update(|number_input: &NumberInput| LineToolMessage::UpdateOptions(LineOptionsUpdate::LineWeight(number_input.value.unwrap())).into())
		.widget_holder()
}

impl LayoutHolder for LineTool {
	fn layout(&self) -> Layout {
		let mut widgets = self.options.stroke.create_widgets(
			"Stroke",
			true,
			|_| LineToolMessage::UpdateOptions(LineOptionsUpdate::StrokeColor(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| LineToolMessage::UpdateOptions(LineOptionsUpdate::StrokeColorType(color_type.clone())).into()),
			|color: &ColorButton| LineToolMessage::UpdateOptions(LineOptionsUpdate::StrokeColor(color.value)).into(),
		);
		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		widgets.push(create_weight_widget(self.options.line_weight));

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for LineTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		let ToolMessage::Line(LineToolMessage::UpdateOptions(action)) = message else {
			self.fsm_state.process_event(message, &mut self.tool_data, tool_data, &self.options, responses, true);
			return;
		};
		match action {
			LineOptionsUpdate::LineWeight(line_weight) => self.options.line_weight = line_weight,
			LineOptionsUpdate::StrokeColor(color) => {
				self.options.stroke.custom_color = color;
				self.options.stroke.color_type = ToolColorType::Custom;
			}
			LineOptionsUpdate::StrokeColorType(color_type) => self.options.stroke.color_type = color_type,
			LineOptionsUpdate::WorkingColors(primary, secondary) => {
				self.options.stroke.primary_working_color = primary;
				self.options.stroke.secondary_working_color = secondary;
			}
		}

		self.send_layout(responses, LayoutTarget::ToolOptions);
	}

	fn actions(&self) -> ActionList {
		match self.fsm_state {
			LineToolFsmState::Ready => actions!(LineToolMessageDiscriminant; DragStart),
			LineToolFsmState::Drawing => actions!(LineToolMessageDiscriminant; DragStop, Redraw, Abort),
		}
	}
}

impl ToolTransition for LineTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			canvas_transformed: Some(LineToolMessage::CanvasTransformed.into()),
			tool_abort: Some(LineToolMessage::Abort.into()),
			working_color_changed: Some(LineToolMessage::WorkingColorChanged.into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum LineToolFsmState {
	#[default]
	Ready,
	Drawing,
}

#[derive(Clone, Debug, Default)]
struct LineToolData {
	drag_start: DVec2,
	drag_current: DVec2,
	angle: f64,
	weight: f64,
	layer: Option<LayerNodeIdentifier>,
	snap_manager: SnapManager,
}

impl Fsm for LineToolFsmState {
	type ToolData = LineToolData;
	type ToolOptions = LineOptions;

	fn transition(self, event: ToolMessage, tool_data: &mut Self::ToolData, tool_action_data: &mut ToolActionHandlerData, tool_options: &Self::ToolOptions, responses: &mut VecDeque<Message>) -> Self {
		let ToolActionHandlerData {
			document, global_tool_data, input, ..
		} = tool_action_data;

		let ToolMessage::Line(event) = event else {
			return self;
		};
		match (self, event) {
			(LineToolFsmState::Ready, LineToolMessage::DragStart) => {
				tool_data.snap_manager.start_snap(document, input, document.bounding_boxes(), true, true);

				let viewport_start = tool_data.snap_manager.snap_position(responses, document, input.mouse.position);
				tool_data.drag_start = document.metadata().document_to_viewport.inverse().transform_point2(viewport_start);

				let subpath = bezier_rs::Subpath::new_line(DVec2::ZERO, DVec2::X);

				responses.add(DocumentMessage::StartTransaction);

				let layer = graph_modification_utils::new_vector_layer(vec![subpath], generate_uuid(), document.new_layer_parent(), responses);
				responses.add(GraphOperationMessage::StrokeSet {
					layer: layer.to_path(),
					stroke: Stroke::new(tool_options.stroke.active_color(), tool_options.line_weight),
				});
				tool_data.layer = Some(layer);

				tool_data.weight = tool_options.line_weight;

				LineToolFsmState::Drawing
			}
			(LineToolFsmState::Drawing, LineToolMessage::Redraw { center, snap_angle, lock_angle }) => {
				tool_data.drag_current = tool_data.snap_manager.snap_position(responses, document, input.mouse.position);

				let keyboard = &input.keyboard;
				let transform = document.metadata().document_to_viewport;
				responses.add(generate_transform(tool_data, transform, keyboard.key(lock_angle), keyboard.key(snap_angle), keyboard.key(center)));

				LineToolFsmState::Drawing
			}
			(LineToolFsmState::Drawing, LineToolMessage::DragStop) => {
				tool_data.snap_manager.cleanup(responses);
				input.mouse.finish_transaction(tool_data.drag_start, responses);
				tool_data.layer = None;
				LineToolFsmState::Ready
			}
			(LineToolFsmState::Drawing, LineToolMessage::Abort) => {
				tool_data.snap_manager.cleanup(responses);
				responses.add(DocumentMessage::AbortTransaction);
				tool_data.layer = None;
				LineToolFsmState::Ready
			}
			(_, LineToolMessage::WorkingColorChanged) => {
				responses.add(LineToolMessage::UpdateOptions(LineOptionsUpdate::WorkingColors(
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
			LineToolFsmState::Ready => HintData(vec![HintGroup(vec![
				HintInfo::mouse(MouseMotion::LmbDrag, "Draw Line"),
				HintInfo::keys([Key::Shift], "Snap 15°").prepend_plus(),
				HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
				HintInfo::keys([Key::Control], "Lock Angle").prepend_plus(),
			])]),
			LineToolFsmState::Drawing => HintData(vec![HintGroup(vec![
				HintInfo::keys([Key::Shift], "Snap 15°"),
				HintInfo::keys([Key::Alt], "From Center"),
				HintInfo::keys([Key::Control], "Lock Angle"),
			])]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Crosshair });
	}
}

fn generate_transform(tool_data: &mut LineToolData, document_to_viewport: DAffine2, lock_angle: bool, snap_angle: bool, center: bool) -> Message {
	let mut start = document_to_viewport.transform_point2(tool_data.drag_start);
	let line_vector = tool_data.drag_current - start;

	let mut angle = -line_vector.angle_between(DVec2::X);

	if lock_angle {
		angle = tool_data.angle;
	}

	if snap_angle {
		let snap_resolution = LINE_ROTATE_SNAP_ANGLE.to_radians();
		angle = (angle / snap_resolution).round() * snap_resolution;
	}

	tool_data.angle = angle;

	let mut line_length = line_vector.length();

	if lock_angle {
		let angle_vec = DVec2::new(angle.cos(), angle.sin());
		line_length = line_vector.dot(angle_vec);
	}

	if center {
		start -= line_length * DVec2::new(angle.cos(), angle.sin());
		line_length *= 2.;
	}

	GraphOperationMessage::TransformSet {
		layer: tool_data.layer.unwrap().to_path(),
		transform: glam::DAffine2::from_scale_angle_translation(DVec2::new(line_length, 1.), angle, start),
		transform_in: TransformIn::Viewport,
		skip_rerender: false,
	}
	.into()
}

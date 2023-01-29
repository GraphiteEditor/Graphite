use crate::consts::LINE_ROTATE_SNAP_ANGLE;
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, MouseMotion};
use crate::messages::input_mapper::utility_types::input_mouse::ViewportPosition;
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, PropertyHolder, WidgetLayout};
use crate::messages::layout::utility_types::widgets::input_widgets::NumberInput;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::snapping::SnapManager;
use crate::messages::tool::utility_types::{EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use document_legacy::layers::style;
use document_legacy::LayerId;
use document_legacy::Operation;

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct LineTool {
	fsm_state: LineToolFsmState,
	tool_data: LineToolData,
	options: LineOptions,
}

pub struct LineOptions {
	line_weight: f64,
}

impl Default for LineOptions {
	fn default() -> Self {
		Self { line_weight: 5. }
	}
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Line)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub enum LineToolMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,

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

impl PropertyHolder for LineTool {
	fn properties(&self) -> Layout {
		let weight = NumberInput::new(Some(self.options.line_weight))
			.unit(" px")
			.label("Weight")
			.min(0.)
			.on_update(|number_input: &NumberInput| LineToolMessage::UpdateOptions(LineOptionsUpdate::LineWeight(number_input.value.unwrap())).into())
			.widget_holder();
		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets: vec![weight] }]))
	}
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for LineTool {
	fn process_message(&mut self, message: ToolMessage, messages: &mut VecDeque<Message>, tool_data: ToolActionHandlerData<'a>) {
		if let ToolMessage::Line(LineToolMessage::UpdateOptions(action)) = message {
			match action {
				LineOptionsUpdate::LineWeight(line_weight) => self.options.line_weight = line_weight,
			}
			return;
		}

		self.fsm_state.process_event(message, &mut self.tool_data, tool_data, &self.options, messages, true);
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
			document_dirty: None,
			tool_abort: Some(LineToolMessage::Abort.into()),
			selection_changed: None,
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
	drag_start: ViewportPosition,
	drag_current: ViewportPosition,
	angle: f64,
	weight: f64,
	path: Option<Vec<LayerId>>,
	snap_manager: SnapManager,
}

impl Fsm for LineToolFsmState {
	type ToolData = LineToolData;
	type ToolOptions = LineOptions;

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		(document, _document_id, global_tool_data, input, font_cache): ToolActionHandlerData,
		tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use LineToolFsmState::*;
		use LineToolMessage::*;

		if let ToolMessage::Line(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					tool_data.snap_manager.start_snap(document, input, document.bounding_boxes(None, None, font_cache), true, true);
					tool_data.snap_manager.add_all_document_handles(document, input, &[], &[], &[]);
					tool_data.drag_start = tool_data.snap_manager.snap_position(responses, document, input.mouse.position);

					responses.push_back(DocumentMessage::StartTransaction.into());
					tool_data.path = Some(document.get_path_for_new_layer());
					responses.push_back(DocumentMessage::DeselectAllLayers.into());

					tool_data.weight = tool_options.line_weight;

					responses.push_back(
						Operation::AddLine {
							path: tool_data.path.clone().unwrap(),
							insert_index: -1,
							transform: DAffine2::ZERO.to_cols_array(),
							style: style::PathStyle::new(Some(style::Stroke::new(global_tool_data.primary_color, tool_data.weight)), style::Fill::None),
						}
						.into(),
					);

					Drawing
				}
				(Drawing, Redraw { center, snap_angle, lock_angle }) => {
					tool_data.drag_current = tool_data.snap_manager.snap_position(responses, document, input.mouse.position);

					let keyboard = &input.keyboard;
					responses.push_back(generate_transform(tool_data, keyboard.key(lock_angle), keyboard.key(snap_angle), keyboard.key(center)));

					Drawing
				}
				(Drawing, DragStop) => {
					tool_data.snap_manager.cleanup(responses);
					input.mouse.finish_transaction(tool_data.drag_start, responses);
					tool_data.path = None;

					Ready
				}
				(Drawing, Abort) => {
					tool_data.snap_manager.cleanup(responses);
					responses.push_back(DocumentMessage::AbortTransaction.into());
					tool_data.path = None;
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

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Crosshair }.into());
	}
}

fn generate_transform(tool_data: &mut LineToolData, lock_angle: bool, snap_angle: bool, center: bool) -> Message {
	let mut start = tool_data.drag_start;
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

	Operation::SetLayerTransformInViewport {
		path: tool_data.path.clone().unwrap(),
		transform: glam::DAffine2::from_scale_angle_translation(DVec2::new(line_length, 1.), angle, start).to_cols_array(),
	}
	.into()
}

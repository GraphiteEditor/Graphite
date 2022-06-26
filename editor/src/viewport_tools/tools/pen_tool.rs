use crate::consts::{CREATE_CURVE_THRESHOLD, LINE_ROTATE_SNAP_ANGLE};
use crate::frontend::utility_types::MouseCursorIcon;
use crate::input::keyboard::{Key, MouseMotion};
use crate::input::InputPreprocessorMessageHandler;
use crate::layout::widgets::{LayoutRow, NumberInput, PropertyHolder, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::message_prelude::*;
use crate::misc::{HintData, HintGroup, HintInfo, KeysGroup};
use crate::viewport_tools::snapping::SnapHandler;
use crate::viewport_tools::tool::{Fsm, ToolActionHandlerData};
use crate::viewport_tools::vector_editor::overlay_renderer::OverlayRenderer;

use graphene::layers::style;
// use graphene::layers::vector::vector_shape::VectorShape;
use graphene::Operation;

use glam::{DAffine2, DVec2};
use kurbo::{PathEl, Point};
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct PenTool {
	fsm_state: PenToolFsmState,
	tool_data: PenToolData,
	options: PenOptions,
}

pub struct PenOptions {
	line_weight: f64,
}

impl Default for PenOptions {
	fn default() -> Self {
		Self { line_weight: 5. }
	}
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Pen)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum PenToolMessage {
	// Standard messages
	#[remain::unsorted]
	DocumentIsDirty,
	#[remain::unsorted]
	Abort,

	// Tool-specific messages
	Confirm,
	DragStart,
	DragStop,
	PointerMove {
		snap_angle: Key,
		break_handle: Key,
	},
	Undo,
	UpdateOptions(PenOptionsUpdate),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PenToolFsmState {
	Ready,
	DraggingHandle,
	PlacingAnchor,
}

#[remain::sorted]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum PenOptionsUpdate {
	LineWeight(f64),
}

impl PropertyHolder for PenTool {
	fn properties(&self) -> WidgetLayout {
		WidgetLayout::new(vec![LayoutRow::Row {
			widgets: vec![WidgetHolder::new(Widget::NumberInput(NumberInput {
				unit: " px".into(),
				label: "Weight".into(),
				value: Some(self.options.line_weight),
				is_integer: false,
				min: Some(0.),
				on_update: WidgetCallback::new(|number_input: &NumberInput| PenToolMessage::UpdateOptions(PenOptionsUpdate::LineWeight(number_input.value.unwrap())).into()),
				..NumberInput::default()
			}))],
		}])
	}
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for PenTool {
	fn process_action(&mut self, action: ToolMessage, tool_data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if action == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		if action == ToolMessage::UpdateCursor {
			self.fsm_state.update_cursor(responses);
			return;
		}

		if let ToolMessage::Pen(PenToolMessage::UpdateOptions(action)) = action {
			match action {
				PenOptionsUpdate::LineWeight(line_weight) => self.options.line_weight = line_weight,
			}
			return;
		}

		let new_state = self.fsm_state.transition(action, &mut self.tool_data, tool_data, &self.options, responses);

		if self.fsm_state != new_state {
			self.fsm_state = new_state;
			self.fsm_state.update_hints(responses);
			self.fsm_state.update_cursor(responses);
		}
	}

	fn actions(&self) -> ActionList {
		match self.fsm_state {
			PenToolFsmState::Ready => actions!(PenToolMessageDiscriminant; Undo, DragStart, DragStop, Confirm, Abort),
			PenToolFsmState::DraggingHandle | PenToolFsmState::PlacingAnchor => actions!(PenToolMessageDiscriminant; DragStart, DragStop, PointerMove, Confirm, Abort),
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
	weight: f64,
	path: Option<Vec<LayerId>>,
	overlay_renderer: OverlayRenderer,
	bez_path: Vec<PathEl>,
	snap_handler: SnapHandler,
}

impl Fsm for PenToolFsmState {
	type ToolData = PenToolData;
	type ToolOptions = PenOptions;

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		(document, global_tool_data, input, font_cache): ToolActionHandlerData,
		tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		let transform = tool_data.path.as_ref().and_then(|path| document.graphene_document.multiply_transforms(path).ok()).unwrap_or_default();

		if let ToolMessage::Pen(event) = event {
			match (self, event) {
				(_, PenToolMessage::DocumentIsDirty) => {
					// When the document has moved / needs to be redraw, re-render the overlays
					// TODO the overlay system should probably receive this message instead of the tool
					for layer_path in document.selected_visible_layers() {
						tool_data.overlay_renderer.render_vector_shape_overlays(&document.graphene_document, layer_path.to_vec(), responses);
					}
					self
				}
				(PenToolFsmState::Ready, PenToolMessage::DragStart) => {
					responses.push_back(DocumentMessage::StartTransaction.into());
					responses.push_back(DocumentMessage::DeselectAllLayers.into());

					// Create a new layer and prep snap system
					tool_data.path = Some(document.get_path_for_new_layer());
					tool_data.snap_handler.start_snap(document, document.bounding_boxes(None, None, font_cache), true, true);
					tool_data.snap_handler.add_all_document_handles(document, &[], &[], &[]);
					let snapped_position = tool_data.snap_handler.snap_position(responses, document, input.mouse.position);

					// Get the position and set properties
					let transform = tool_data
						.path
						.as_ref()
						.and_then(|path| document.graphene_document.multiply_transforms(&path[..path.len() - 1]).ok())
						.unwrap_or_default();
					let start_position = transform.inverse().transform_point2(snapped_position);
					tool_data.weight = tool_options.line_weight;

					// Create the initial shape with a `bez_path` (only contains a moveto initially)
					if let Some(layer_path) = &tool_data.path {
						tool_data.bez_path = start_bez_path(start_position);
						add_cubic(&mut tool_data.bez_path);
						responses.push_back(
							Operation::AddShape {
								path: layer_path.clone(),
								transform: DAffine2::IDENTITY.to_cols_array(),
								insert_index: -1,
								bez_path: tool_data.bez_path.clone().into_iter().collect(),
								style: style::PathStyle::new(Some(style::Stroke::new(global_tool_data.primary_color, tool_data.weight)), style::Fill::None),
							}
							.into(),
						);
					}

					PenToolFsmState::DraggingHandle
				}
				(PenToolFsmState::PlacingAnchor, PenToolMessage::DragStart) => {
					add_cubic(&mut tool_data.bez_path);
					PenToolFsmState::DraggingHandle
				}
				(PenToolFsmState::DraggingHandle, PenToolMessage::DragStop) => {
					// TODO: If the drag does not exceed the threshold, then replace the curve with a line
					if segement_control(tool_data.bez_path.last()).distance(input.mouse.position) < CREATE_CURVE_THRESHOLD {}

					PenToolFsmState::PlacingAnchor
				}
				(PenToolFsmState::DraggingHandle, PenToolMessage::PointerMove { snap_angle, break_handle }) => {
					if let Some(layer_path) = &tool_data.path {
						let mouse = tool_data.snap_handler.snap_position(responses, document, input.mouse.position);
						let pos = transform.inverse().transform_point2(mouse);
						let pos = compute_snapped_angle(input, snap_angle, pos, segement_control(tool_data.bez_path.iter().nth_back(1)));

						// Update points on current segment (to show preview of new handle)
						if let Some(PathEl::CurveTo(handle1, handle2, end)) = tool_data.bez_path.iter_mut().last() {
							let point = Point::new(pos.x, pos.y);
							(*handle1, *handle2, *end) = (point, point, point);
						}

						// Mirror handle of last segement
						if !input.keyboard.get(break_handle as usize) {
							if let Some(PathEl::CurveTo(_handle1, handle2, end)) = tool_data.bez_path.iter_mut().nth_back(1) {
								let end = DVec2::new(end.x, end.y);

								let pos = end - (pos - end);
								let point = Point::new(pos.x, pos.y);
								*handle2 = point;
							}
						}

						responses.push_back(apply_bez_path(layer_path.clone(), tool_data.bez_path.clone()));
					}

					self
				}
				(PenToolFsmState::PlacingAnchor, PenToolMessage::PointerMove { snap_angle, .. }) => {
					if let Some(layer_path) = &tool_data.path {
						let mouse = tool_data.snap_handler.snap_position(responses, document, input.mouse.position);
						let pos = transform.inverse().transform_point2(mouse);
						let pos = compute_snapped_angle(input, snap_angle, pos, segement_control(tool_data.bez_path.iter().nth_back(1)));

						// Update the current segement's last handle and end
						if let Some(PathEl::CurveTo(_handle1, handle2, end)) = tool_data.bez_path.iter_mut().last() {
							let point = Point::new(pos.x, pos.y);
							(*handle2, *end) = (point, point);
						}
						responses.push_back(apply_bez_path(layer_path.clone(), tool_data.bez_path.clone()));
					}

					self
				}
				(PenToolFsmState::DraggingHandle | PenToolFsmState::PlacingAnchor, PenToolMessage::Abort | PenToolMessage::Confirm) => {
					// Abort or commit the transaction to the undo history
					if tool_data.bez_path.len() > 2 {
						// Remove the last segment (an uncommitted preview)
						if let Some(layer_path) = &tool_data.path {
							tool_data.bez_path.pop();
							responses.push_back(apply_bez_path(layer_path.clone(), tool_data.bez_path.clone()));
						}

						responses.push_back(DocumentMessage::DeselectAllLayers.into());
						responses.push_back(DocumentMessage::CommitTransaction.into());
					} else {
						responses.push_back(DocumentMessage::AbortTransaction.into());
					}

					// Clean up overlays
					if let Some(layer_path) = &tool_data.path {
						tool_data.overlay_renderer.clear_vector_shape_overlays(&document.graphene_document, layer_path.clone(), responses);
					}
					tool_data.path = None;
					tool_data.snap_handler.cleanup(responses);

					PenToolFsmState::Ready
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
			PenToolFsmState::DraggingHandle | PenToolFsmState::PlacingAnchor => HintData(vec![
				HintGroup(vec![HintInfo {
					key_groups: vec![],
					mouse: Some(MouseMotion::LmbDrag),
					label: String::from("Add Handle"),
					plus: false,
				}]),
				HintGroup(vec![HintInfo {
					key_groups: vec![],
					mouse: Some(MouseMotion::Lmb),
					label: String::from("Add Control Point"),
					plus: false,
				}]),
				HintGroup(vec![HintInfo {
					key_groups: vec![KeysGroup(vec![Key::KeyControl])],
					mouse: None,
					label: String::from("Snap 15°"),
					plus: false,
				}]),
				HintGroup(vec![HintInfo {
					key_groups: vec![KeysGroup(vec![Key::KeyShift])],
					mouse: None,
					label: String::from("Break handle"),
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

/// Create the initial moveto for the `bez_path`
fn start_bez_path(start_position: DVec2) -> Vec<PathEl> {
	vec![PathEl::MoveTo(Point {
		x: start_position.x,
		y: start_position.y,
	})]
}

/// Computes the control point (point where the path will go through), panicing if [None] or [PathEl::ClosePath]
fn segement_control(path: Option<&PathEl>) -> DVec2 {
	match path {
		Some(PathEl::MoveTo(pos)) => DVec2::new(pos.x, pos.y),
		Some(PathEl::LineTo(pos)) => DVec2::new(pos.x, pos.y),
		Some(PathEl::CurveTo(_, _, pos)) => DVec2::new(pos.x, pos.y),
		Some(PathEl::QuadTo(_, pos)) => DVec2::new(pos.x, pos.y),
		_ => panic!("unexpected path data in pen tool"),
	}
}

/// Pushes a cubic bezier onto the path with all the points set to the end of the last segment
fn add_cubic(path: &mut Vec<PathEl>) {
	let last_point = segement_control(path.last());
	let point = Point::new(last_point.x, last_point.y);
	path.push(PathEl::CurveTo(point, point, point))
}

/// Apply the `bez_path` to the `shape` in the viewport
fn apply_bez_path(layer_path: Vec<LayerId>, bez_path: Vec<PathEl>) -> Message {
	Operation::SetShapePath {
		path: layer_path,
		bez_path: bez_path.into_iter().collect(),
	}
	.into()
}

/// Snap the angle of the line from relative to pos if the key is pressed
fn compute_snapped_angle(input: &InputPreprocessorMessageHandler, key: Key, pos: DVec2, relative: DVec2) -> DVec2 {
	if input.keyboard.get(key as usize) {
		let delta = relative - pos;

		let length = delta.length();
		let mut angle = -delta.angle_between(DVec2::X);

		let snap_resolution = LINE_ROTATE_SNAP_ANGLE.to_radians();
		angle = (angle / snap_resolution).round() * snap_resolution;

		let rotated = DVec2::new(length * angle.cos(), length * angle.sin());
		relative - rotated
	} else {
		pos
	}
}

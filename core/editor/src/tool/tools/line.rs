use crate::input::{mouse::ViewportPosition, InputPreprocessor};
use crate::tool::{DocumentToolData, Fsm, ToolActionHandlerData};
use crate::{document::Document, message_prelude::*};
use document_core::{layers::style, Operation};
use glam::{DAffine2, DVec2};

use std::f64::consts::PI;

#[derive(Default)]
pub struct Line {
	fsm_state: LineToolFsmState,
	data: LineToolData,
}

#[impl_message(Message, ToolMessage, Line)]
#[derive(PartialEq, Clone, Debug)]
pub enum LineMessage {
	DragStart,
	DragStop,
	MouseMove,
	Abort,
	Center,
	UnCenter,
	LockAngle,
	UnlockAngle,
	SnapToAngle,
	UnSnapToAngle,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Line {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		self.fsm_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, data.2, responses);
	}
	fn actions(&self) -> ActionList {
		use LineToolFsmState::*;
		match self.fsm_state {
			Ready => actions!(LineMessageDiscriminant;  DragStart, Center, UnCenter, LockAngle, UnlockAngle, SnapToAngle, UnSnapToAngle),
			Dragging => actions!(LineMessageDiscriminant; DragStop, MouseMove, Abort, Center, UnCenter, LockAngle, UnlockAngle,  SnapToAngle, UnSnapToAngle),
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LineToolFsmState {
	Ready,
	Dragging,
}

impl Default for LineToolFsmState {
	fn default() -> Self {
		LineToolFsmState::Ready
	}
}
#[derive(Clone, Debug, Default)]
struct LineToolData {
	drag_start: ViewportPosition,
	drag_current: ViewportPosition,
	angle: f64,
	snap_angle: bool,
	lock_angle: bool,
	center_around_cursor: bool,
}

impl Fsm for LineToolFsmState {
	type ToolData = LineToolData;

	fn transition(self, event: ToolMessage, document: &Document, tool_data: &DocumentToolData, data: &mut Self::ToolData, input: &InputPreprocessor, responses: &mut VecDeque<Message>) -> Self {
		let transform = document.document.root.transform;
		use LineMessage::*;
		use LineToolFsmState::*;
		if let ToolMessage::Line(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					data.drag_start = input.mouse.position;
					data.drag_current = input.mouse.position;

					responses.push_back(Operation::StartTransaction { path: vec![] }.into());

					Dragging
				}
				(Dragging, MouseMove) => {
					data.drag_current = input.mouse.position;

					responses.push_back(Operation::RollbackTransaction.into());
					responses.push_back(make_operation(data, tool_data, transform));

					Dragging
				}
				(Dragging, DragStop) => {
					data.drag_current = input.mouse.position;

					responses.push_back(Operation::RollbackTransaction.into());
					// TODO - introduce comparison threshold when operating with canvas coordinates (https://github.com/GraphiteEditor/Graphite/issues/100)
					if data.drag_start != data.drag_current {
						responses.push_back(make_operation(data, tool_data, transform));
						responses.push_back(DocumentMessage::DeselectAllLayers.into());
						responses.push_back(Operation::CommitTransaction.into());
					}

					Ready
				}
				// TODO - simplify with or_patterns when rust 1.53.0 is stable (https://github.com/rust-lang/rust/issues/54883)
				(Dragging, Abort) => {
					responses.push_back(Operation::AbortTransaction.into());

					Ready
				}
				(Ready, LockAngle) => update_state_no_op(&mut data.lock_angle, true, Ready),
				(Ready, UnlockAngle) => update_state_no_op(&mut data.lock_angle, false, Ready),
				(Dragging, LockAngle) => update_state(|data| &mut data.lock_angle, true, tool_data, data, responses, Dragging, transform),
				(Dragging, UnlockAngle) => update_state(|data| &mut data.lock_angle, false, tool_data, data, responses, Dragging, transform),

				(Ready, SnapToAngle) => update_state_no_op(&mut data.snap_angle, true, Ready),
				(Ready, UnSnapToAngle) => update_state_no_op(&mut data.snap_angle, false, Ready),
				(Dragging, SnapToAngle) => update_state(|data| &mut data.snap_angle, true, tool_data, data, responses, Dragging, transform),
				(Dragging, UnSnapToAngle) => update_state(|data| &mut data.snap_angle, false, tool_data, data, responses, Dragging, transform),

				(Ready, Center) => update_state_no_op(&mut data.center_around_cursor, true, Ready),
				(Ready, UnCenter) => update_state_no_op(&mut data.center_around_cursor, false, Ready),
				(Dragging, Center) => update_state(|data| &mut data.center_around_cursor, true, tool_data, data, responses, Dragging, transform),
				(Dragging, UnCenter) => update_state(|data| &mut data.center_around_cursor, false, tool_data, data, responses, Dragging, transform),
				_ => self,
			}
		} else {
			self
		}
	}
}

fn update_state_no_op(state: &mut bool, value: bool, new_state: LineToolFsmState) -> LineToolFsmState {
	*state = value;
	new_state
}

fn update_state(
	state: fn(&mut LineToolData) -> &mut bool,
	value: bool,
	tool_data: &DocumentToolData,
	data: &mut LineToolData,
	responses: &mut VecDeque<Message>,
	new_state: LineToolFsmState,
	transform: DAffine2,
) -> LineToolFsmState {
	*(state(data)) = value;

	responses.push_back(Operation::RollbackTransaction.into());
	responses.push_back(make_operation(data, tool_data, transform));

	new_state
}

fn make_operation(data: &mut LineToolData, tool_data: &DocumentToolData, transform: DAffine2) -> Message {
	let x0 = data.drag_start.x as f64;
	let y0 = data.drag_start.y as f64;
	let x1 = data.drag_current.x as f64;
	let y1 = data.drag_current.y as f64;

	let (dx, dy) = (x1 - x0, y1 - y0);
	let mut angle = f64::atan2(dx, dy);

	if data.lock_angle {
		angle = data.angle
	};

	if data.snap_angle {
		let snap_resolution = 12.0;
		angle = (angle * snap_resolution / PI).round() / snap_resolution * PI;
	}

	data.angle = angle;

	let (dir_x, dir_y) = (f64::sin(angle), f64::cos(angle));
	let projected_length = dx * dir_x + dy * dir_y;
	let (x1, y1) = (x0 + dir_x * projected_length, y0 + dir_y * projected_length);

	let (x0, y0) = if data.center_around_cursor { (x0 - (x1 - x0), y0 - (y1 - y0)) } else { (x0, y0) };

	Operation::AddLine {
		path: vec![],
		insert_index: -1,
		transform: (transform.inverse() * glam::DAffine2::from_scale_angle_translation(DVec2::new(x1 - x0, y1 - y0), 0., DVec2::new(x0, y0))).to_cols_array(),
		style: style::PathStyle::new(Some(style::Stroke::new(tool_data.primary_color, 5.)), None),
	}
	.into()
}

use crate::events::ViewportPosition;
use crate::tools::Fsm;
use crate::SvgDocument;
use crate::{
	dispatcher::{Action, ActionHandler, InputPreprocessor, Response},
	tools::{DocumentToolData, ToolActionHandlerData},
};
use document_core::layers::style;
use document_core::Operation;

use std::f64::consts::PI;

#[derive(Default)]
pub struct Line {
	fsm_state: LineToolFsmState,
	data: LineToolData,
}

impl<'a> ActionHandler<ToolActionHandlerData<'a>> for Line {
	fn process_action(&mut self, data: ToolActionHandlerData<'a>, input_preprocessor: &InputPreprocessor, action: &Action, responses: &mut Vec<Response>, operations: &mut Vec<Operation>) -> bool {
		let (consumed, state) = self.fsm_state.transition(action, data.0, data.1, &mut self.data, input_preprocessor, responses, operations);
		self.fsm_state = state;
		consumed
	}

	actions_fn!(Action::Undo);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LineToolFsmState {
	Ready,
	LmbDown,
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

	fn transition(
		self,
		event: &Action,
		_document: &SvgDocument,
		tool_data: &DocumentToolData,
		data: &mut Self::ToolData,
		input: &InputPreprocessor,
		_responses: &mut Vec<Response>,
		operations: &mut Vec<Operation>,
	) -> (bool, Self) {
		match (self, event) {
			(LineToolFsmState::Ready, Action::LmbDown) => {
				data.drag_start = input.mouse_state.position;
				data.drag_current = input.mouse_state.position;

				operations.push(Operation::MountWorkingFolder { path: vec![] });

				(true, LineToolFsmState::LmbDown)
			}
			(LineToolFsmState::LmbDown, Action::MouseMove) => {
				data.drag_current = input.mouse_state.position;

				operations.push(Operation::ClearWorkingFolder);
				operations.push(make_operation(data, tool_data));

				(true, LineToolFsmState::LmbDown)
			}
			(LineToolFsmState::LmbDown, Action::LmbUp) => {
				data.drag_current = input.mouse_state.position;

				operations.push(Operation::ClearWorkingFolder);
				// TODO - introduce comparison threshold when operating with canvas coordinates (https://github.com/GraphiteEditor/Graphite/issues/100)
				if data.drag_start != data.drag_current {
					operations.push(make_operation(data, tool_data));
					operations.push(Operation::CommitTransaction);
				}

				(true, LineToolFsmState::Ready)
			}
			// TODO - simplify with or_patterns when rust 1.53.0 is stable (https://github.com/rust-lang/rust/issues/54883)
			(LineToolFsmState::LmbDown, Action::Abort) | (LineToolFsmState::LmbDown, Action::RmbDown) => {
				operations.push(Operation::DiscardWorkingFolder);

				(true, LineToolFsmState::Ready)
			}
			(state, Action::LockAspectRatio) => {
				data.snap_angle = true;

				if state == LineToolFsmState::LmbDown {
					operations.push(Operation::ClearWorkingFolder);
					operations.push(make_operation(data, tool_data));
				}

				(true, self)
			}
			(state, Action::UnlockAspectRatio) => {
				data.snap_angle = false;

				if state == LineToolFsmState::LmbDown {
					operations.push(Operation::ClearWorkingFolder);
					operations.push(make_operation(data, tool_data));
				}

				(true, self)
			}
			(state, Action::SnapAngle) => {
				data.lock_angle = true;

				if state == LineToolFsmState::LmbDown {
					operations.push(Operation::ClearWorkingFolder);
					operations.push(make_operation(data, tool_data));
				}

				(true, self)
			}
			(state, Action::UnSnapAngle) => {
				data.lock_angle = false;

				if state == LineToolFsmState::LmbDown {
					operations.push(Operation::ClearWorkingFolder);
					operations.push(make_operation(data, tool_data));
				}

				(true, self)
			}
			(state, Action::Center) => {
				data.center_around_cursor = true;

				if state == LineToolFsmState::LmbDown {
					operations.push(Operation::ClearWorkingFolder);
					operations.push(make_operation(data, tool_data));
				}

				(true, self)
			}
			(state, Action::UnCenter) => {
				data.center_around_cursor = false;

				if state == LineToolFsmState::LmbDown {
					operations.push(Operation::ClearWorkingFolder);
					operations.push(make_operation(data, tool_data));
				}

				(true, self)
			}
			_ => (false, self),
		}
	}
}

fn make_operation(data: &mut LineToolData, tool_data: &DocumentToolData) -> Operation {
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
		x0,
		y0,
		x1,
		y1,
		style: style::PathStyle::new(Some(style::Stroke::new(tool_data.primary_color, 5.)), None),
	}
}

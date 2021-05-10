use crate::events::ViewportPosition;
use crate::tools::Fsm;
use crate::{
	dispatcher::{Action, ActionHandler, InputPreprocessor, Response},
	tools::{DocumentToolData, ToolActionHandlerData},
	SvgDocument,
};
use document_core::layers::style;
use document_core::Operation;

#[derive(Default)]
pub struct Rectangle {
	fsm_state: RectangleToolFsmState,
	data: RectangleToolData,
}

impl<'a> ActionHandler<ToolActionHandlerData<'a>> for Rectangle {
	fn process_action(&mut self, data: ToolActionHandlerData<'a>, input_preprocessor: &InputPreprocessor, action: &Action, responses: &mut Vec<Response>, operations: &mut Vec<Operation>) -> bool {
		let (consumed, state) = self.fsm_state.transition(action, data.0, data.1, &mut self.data, input_preprocessor, responses, operations);
		self.fsm_state = state;
		consumed
	}
	actions_fn!();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RectangleToolFsmState {
	Ready,
	LmbDown,
}

impl Default for RectangleToolFsmState {
	fn default() -> Self {
		RectangleToolFsmState::Ready
	}
}
#[derive(Clone, Debug, Default)]
struct RectangleToolData {
	drag_start: ViewportPosition,
	drag_current: ViewportPosition,
	constrain_to_square: bool,
	center_around_cursor: bool,
}

impl Fsm for RectangleToolFsmState {
	type ToolData = RectangleToolData;

	fn transition(
		self,
		event: &Action,
		document: &SvgDocument,
		tool_data: &DocumentToolData,
		data: &mut Self::ToolData,
		input: &InputPreprocessor,
		_responses: &mut Vec<Response>,
		operations: &mut Vec<Operation>,
	) -> (bool, Self) {
		match (self, event) {
			(RectangleToolFsmState::Ready, Action::LmbDown) => {
				data.drag_start = input.mouse_state.position;
				data.drag_current = input.mouse_state.position;
				operations.push(Operation::MountWorkingFolder { path: vec![] });
				(true, RectangleToolFsmState::LmbDown)
			}
			(RectangleToolFsmState::LmbDown, Action::MouseMove) => {
				data.drag_current = input.mouse_state.position;

				operations.push(Operation::ClearWorkingFolder);
				operations.push(make_operation(data, tool_data));

				(true, RectangleToolFsmState::LmbDown)
			}
			(RectangleToolFsmState::LmbDown, Action::LmbUp) => {
				data.drag_current = input.mouse_state.position;

				operations.push(Operation::ClearWorkingFolder);
				// TODO - introduce comparison threshold when operating with canvas coordinates (https://github.com/GraphiteEditor/Graphite/issues/100)
				if data.drag_start != data.drag_current {
					operations.push(make_operation(data, tool_data));
					operations.push(Operation::CommitTransaction);
				}

				(true, RectangleToolFsmState::Ready)
			}
			// TODO - simplify with or_patterns when rust 1.53.0 is stable (https://github.com/rust-lang/rust/issues/54883)
			(RectangleToolFsmState::LmbDown, Action::Abort) | (RectangleToolFsmState::LmbDown, Action::RmbDown) => {
				operations.push(Operation::DiscardWorkingFolder);

				(true, RectangleToolFsmState::Ready)
			}
			(state, Action::LockAspectRatio) => {
				data.constrain_to_square = true;

				if state == RectangleToolFsmState::LmbDown {
					operations.push(Operation::ClearWorkingFolder);
					operations.push(make_operation(data, tool_data));
				}

				(true, self)
			}
			(state, Action::UnlockAspectRatio) => {
				data.constrain_to_square = false;

				if state == RectangleToolFsmState::LmbDown {
					operations.push(Operation::ClearWorkingFolder);
					operations.push(make_operation(data, tool_data));
				}

				(true, self)
			}
			(state, Action::Center) => {
				data.center_around_cursor = true;

				if state == RectangleToolFsmState::LmbDown {
					operations.push(Operation::ClearWorkingFolder);
					operations.push(make_operation(data, tool_data));
				}

				(true, self)
			}
			(state, Action::UnCenter) => {
				data.center_around_cursor = false;

				if state == RectangleToolFsmState::LmbDown {
					operations.push(Operation::ClearWorkingFolder);
					operations.push(make_operation(data, tool_data));
				}

				(true, self)
			}
			_ => (false, self),
		}
	}
}

fn make_operation(data: &RectangleToolData, tool_data: &DocumentToolData) -> Operation {
	let x0 = data.drag_start.x as f64;
	let y0 = data.drag_start.y as f64;
	let x1 = data.drag_current.x as f64;
	let y1 = data.drag_current.y as f64;

	let (x0, y0, x1, y1) = if data.constrain_to_square {
		let (x_dir, y_dir) = ((x1 - x0).signum(), (y1 - y0).signum());
		let max_dist = f64::max((x1 - x0).abs(), (y1 - y0).abs());
		if data.center_around_cursor {
			(x0 - max_dist * x_dir, y0 - max_dist * y_dir, x0 + max_dist * x_dir, y0 + max_dist * y_dir)
		} else {
			(x0, y0, x0 + max_dist * x_dir, y0 + max_dist * y_dir)
		}
	} else {
		let (x0, y0) = if data.center_around_cursor {
			let delta_x = x1 - x0;
			let delta_y = y1 - y0;

			(x0 - delta_x, y0 - delta_y)
		} else {
			(x0, y0)
		};
		(x0, y0, x1, y1)
	};

	Operation::AddRect {
		path: vec![],
		insert_index: -1,
		x0,
		y0,
		x1,
		y1,
		style: style::PathStyle::new(None, Some(style::Fill::new(tool_data.primary_color))),
	}
}

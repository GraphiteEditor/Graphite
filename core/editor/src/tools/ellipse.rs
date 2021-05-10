use crate::events::ViewportPosition;
use crate::tools::Fsm;
use crate::SvgDocument;
use document_core::layers::style;
use document_core::Operation;

#[derive(Default)]
pub struct Ellipse {
	fsm_state: EllipseToolFsmState,
	data: EllipseToolData,
}

use crate::{
	dispatcher::{Action, ActionHandler, InputPreprocessor, Response},
	tools::{DocumentToolData, ToolActionHandlerData},
};
impl<'a> ActionHandler<ToolActionHandlerData<'a>> for Ellipse {
	fn process_action(&mut self, data: ToolActionHandlerData<'a>, input_preprocessor: &InputPreprocessor, action: &Action, responses: &mut Vec<Response>, operations: &mut Vec<Operation>) -> bool {
		let (consumed, state) = self.fsm_state.transition(action, data.0, data.1, &mut self.data, input_preprocessor, responses, operations);
		self.fsm_state = state;
		consumed
	}
	actions!();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EllipseToolFsmState {
	Ready,
	LmbDown,
}

impl Default for EllipseToolFsmState {
	fn default() -> Self {
		EllipseToolFsmState::Ready
	}
}
#[derive(Clone, Debug, Default)]
struct EllipseToolData {
	drag_start: ViewportPosition,
	drag_current: ViewportPosition,
	constrain_to_circle: bool,
	center_around_cursor: bool,
}

impl Fsm for EllipseToolFsmState {
	type ToolData = EllipseToolData;

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
			(EllipseToolFsmState::Ready, Action::LmbDown) => {
				data.drag_start = input.mouse_state.position;
				data.drag_current = input.mouse_state.position;
				operations.push(Operation::MountWorkingFolder { path: vec![] });
				(true, EllipseToolFsmState::LmbDown)
			}
			(EllipseToolFsmState::LmbDown, Action::MouseMove) => {
				data.drag_current = input.mouse_state.position;

				operations.push(Operation::ClearWorkingFolder);
				operations.push(make_operation(data, tool_data));

				(true, EllipseToolFsmState::LmbDown)
			}
			(EllipseToolFsmState::LmbDown, Action::LmbUp) => {
				data.drag_current = input.mouse_state.position;

				operations.push(Operation::ClearWorkingFolder);
				// TODO - introduce comparison threshold when operating with canvas coordinates (https://github.com/GraphiteEditor/Graphite/issues/100)
				if data.drag_start != data.drag_current {
					operations.push(make_operation(data, tool_data));
					operations.push(Operation::CommitTransaction);
				}

				(true, EllipseToolFsmState::Ready)
			}
			// TODO - simplify with or_patterns when rust 1.53.0 is stable (https://github.com/rust-lang/rust/issues/54883)
			(EllipseToolFsmState::LmbDown, Action::Abort) | (EllipseToolFsmState::LmbDown, Action::RmbDown) => {
				operations.push(Operation::DiscardWorkingFolder);

				(true, EllipseToolFsmState::Ready)
			}
			(state, Action::LockAspectRatio) => {
				data.constrain_to_circle = true;

				if state == EllipseToolFsmState::LmbDown {
					operations.push(Operation::ClearWorkingFolder);
					operations.push(make_operation(data, tool_data));
				}

				(true, self)
			}
			(state, Action::UnlockAspectRatio) => {
				data.constrain_to_circle = false;

				if state == EllipseToolFsmState::LmbDown {
					operations.push(Operation::ClearWorkingFolder);
					operations.push(make_operation(data, tool_data));
				}

				(true, self)
			}
			(state, Action::Center) => {
				data.center_around_cursor = true;

				if state == EllipseToolFsmState::LmbDown {
					operations.push(Operation::ClearWorkingFolder);
					operations.push(make_operation(data, tool_data));
				}

				(true, self)
			}
			(state, Action::UnCenter) => {
				data.center_around_cursor = false;

				if state == EllipseToolFsmState::LmbDown {
					operations.push(Operation::ClearWorkingFolder);
					operations.push(make_operation(data, tool_data));
				}

				(true, self)
			}
			_ => (false, self),
		}
	}
}

fn make_operation(data: &EllipseToolData, tool_data: &DocumentToolData) -> Operation {
	let x0 = data.drag_start.x as f64;
	let y0 = data.drag_start.y as f64;
	let x1 = data.drag_current.x as f64;
	let y1 = data.drag_current.y as f64;

	if data.constrain_to_circle {
		let (cx, cy, r) = if data.center_around_cursor {
			(x0, y0, f64::hypot(x1 - x0, y1 - y0))
		} else {
			let diameter = f64::max((x1 - x0).abs(), (y1 - y0).abs());
			let (x2, y2) = (x0 + (x1 - x0).signum() * diameter, y0 + (y1 - y0).signum() * diameter);
			((x0 + x2) * 0.5, (y0 + y2) * 0.5, diameter * 0.5)
		};
		Operation::AddCircle {
			path: vec![],
			insert_index: -1,
			cx,
			cy,
			r,
			style: style::PathStyle::new(None, Some(style::Fill::new(tool_data.primary_color))),
		}
	} else {
		let (cx, cy, r_scale) = if data.center_around_cursor { (x0, y0, 1.0) } else { ((x0 + x1) * 0.5, (y0 + y1) * 0.5, 0.5) };
		let (rx, ry) = ((x1 - x0).abs() * r_scale, (y1 - y0).abs() * r_scale);
		Operation::AddEllipse {
			path: vec![],
			insert_index: -1,
			cx,
			cy,
			rx,
			ry,
			rot: 0.0,
			style: style::PathStyle::new(None, Some(style::Fill::new(tool_data.primary_color))),
		}
	}
}

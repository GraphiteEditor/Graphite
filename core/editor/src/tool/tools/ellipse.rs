use crate::input::{mouse::ViewportPosition, InputPreprocessor};
use crate::tool::{DocumentToolData, Fsm, ToolActionHandlerData};
use crate::{message_prelude::*, SvgDocument};
use document_core::{layers::style, Operation};

#[derive(Default)]
pub struct Ellipse {
	fsm_state: EllipseToolFsmState,
	data: EllipseToolData,
}

#[impl_message(Message, ToolMessage, Ellipse)]
#[derive(PartialEq, Clone, Debug)]
pub enum EllipseMessage {
	Undo,
	DragStart,
	DragStop,
	MouseMove,
	Abort,
	Center,
	UnCenter,
	LockAspectRatio,
	UnlockAspectRatio,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Ellipse {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		self.fsm_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, data.2, responses);
	}
	fn actions(&self) -> ActionList {
		use EllipseToolFsmState::*;
		match self.fsm_state {
			Ready => actions!(EllipseMessageDiscriminant; Undo, DragStart, Center, UnCenter, LockAspectRatio, UnlockAspectRatio),
			Dragging => actions!(EllipseMessageDiscriminant; DragStop, Center, UnCenter, LockAspectRatio, UnlockAspectRatio, MouseMove, Abort),
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EllipseToolFsmState {
	Ready,
	Dragging,
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

	fn transition(self, event: ToolMessage, _document: &SvgDocument, tool_data: &DocumentToolData, data: &mut Self::ToolData, input: &InputPreprocessor, responses: &mut VecDeque<Message>) -> Self {
		use EllipseMessage::*;
		use EllipseToolFsmState::*;
		if let ToolMessage::Ellipse(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					data.drag_start = input.mouse_state.position;
					data.drag_current = input.mouse_state.position;
					responses.push_back(Operation::MountWorkingFolder { path: vec![] }.into());
					Dragging
				}
				(Dragging, MouseMove) => {
					data.drag_current = input.mouse_state.position;

					responses.push_back(Operation::ClearWorkingFolder.into());
					responses.push_back(make_operation(data, tool_data));

					Dragging
				}
				(Dragging, DragStop) => {
					data.drag_current = input.mouse_state.position;

					responses.push_back(Operation::ClearWorkingFolder.into());
					// TODO - introduce comparison threshold when operating with canvas coordinates (https://github.com/GraphiteEditor/Graphite/issues/100)
					if data.drag_start != data.drag_current {
						responses.push_back(make_operation(data, tool_data));
						responses.push_back(Operation::CommitTransaction.into());
					}

					Ready
				}
				// TODO - simplify with or_patterns when rust 1.53.0 is stable (https://github.com/rust-lang/rust/issues/54883)
				(Dragging, Abort) => {
					responses.push_back(Operation::DiscardWorkingFolder.into());

					Ready
				}
				(Ready, LockAspectRatio) => update_state_no_op(&mut data.constrain_to_circle, true, Ready),
				(Ready, UnlockAspectRatio) => update_state_no_op(&mut data.constrain_to_circle, false, Ready),
				(Dragging, LockAspectRatio) => update_state(|data| &mut data.constrain_to_circle, true, tool_data, data, responses, Dragging),
				(Dragging, UnlockAspectRatio) => update_state(|data| &mut data.constrain_to_circle, false, tool_data, data, responses, Dragging),

				(Ready, Center) => update_state_no_op(&mut data.center_around_cursor, true, Ready),
				(Ready, UnCenter) => update_state_no_op(&mut data.center_around_cursor, false, Ready),
				(Dragging, Center) => update_state(|data| &mut data.center_around_cursor, true, tool_data, data, responses, Dragging),
				(Dragging, UnCenter) => update_state(|data| &mut data.center_around_cursor, false, tool_data, data, responses, Dragging),
				_ => self,
			}
		} else {
			self
		}
	}
}

fn update_state_no_op(state: &mut bool, value: bool, new_state: EllipseToolFsmState) -> EllipseToolFsmState {
	*state = value;
	new_state
}

fn update_state(
	state: fn(&mut EllipseToolData) -> &mut bool,
	value: bool,
	tool_data: &DocumentToolData,
	data: &mut EllipseToolData,
	responses: &mut VecDeque<Message>,
	new_state: EllipseToolFsmState,
) -> EllipseToolFsmState {
	*(state(data)) = value;

	responses.push_back(Operation::ClearWorkingFolder.into());
	responses.push_back(make_operation(&data, tool_data).into());

	new_state
}

fn make_operation(data: &EllipseToolData, tool_data: &DocumentToolData) -> Message {
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
	.into()
}

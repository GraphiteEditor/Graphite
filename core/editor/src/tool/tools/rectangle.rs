use crate::input::{mouse::ViewportPosition, InputPreprocessor};
use crate::tool::{DocumentToolData, Fsm, ToolActionHandlerData};
use crate::{message_prelude::*, SvgDocument};
use document_core::{layers::style, Operation};

#[derive(Default)]
pub struct Rectangle {
	fsm_state: RectangleToolFsmState,
	data: RectangleToolData,
}

#[impl_message(Message, ToolMessage, Rectangle)]
#[derive(PartialEq, Clone, Debug)]
pub enum RectangleMessage {
	DragStart,
	DragStop,
	MouseMove,
	Abort,
	Center,
	UnCenter,
	LockAspectRatio,
	UnlockAspectRatio,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Rectangle {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		self.fsm_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, data.2, responses);
	}
	fn actions(&self) -> ActionList {
		use RectangleToolFsmState::*;
		match self.fsm_state {
			Ready => actions!(RectangleMessageDiscriminant;  DragStart, Center, UnCenter, LockAspectRatio, UnlockAspectRatio),
			Dragging => actions!(RectangleMessageDiscriminant; DragStop, Center, UnCenter, LockAspectRatio, UnlockAspectRatio, MouseMove, Abort),
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RectangleToolFsmState {
	Ready,
	Dragging,
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

	fn transition(self, event: ToolMessage, _document: &SvgDocument, tool_data: &DocumentToolData, data: &mut Self::ToolData, input: &InputPreprocessor, responses: &mut VecDeque<Message>) -> Self {
		use RectangleMessage::*;
		use RectangleToolFsmState::*;
		if let ToolMessage::Rectangle(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					data.drag_start = input.mouse.position;
					data.drag_current = input.mouse.position;
					responses.push_back(Operation::MountWorkingFolder { path: vec![] }.into());
					Dragging
				}
				(Dragging, MouseMove) => {
					data.drag_current = input.mouse.position;

					responses.push_back(Operation::ClearWorkingFolder.into());
					responses.push_back(make_operation(data, tool_data, input));

					Dragging
				}
				(Dragging, DragStop) => {
					data.drag_current = input.mouse.position;

					responses.push_back(Operation::ClearWorkingFolder.into());
					// TODO - introduce comparison threshold when operating with canvas coordinates (https://github.com/GraphiteEditor/Graphite/issues/100)
					if data.drag_start != data.drag_current {
						responses.push_back(make_operation(data, tool_data, input));
						responses.push_back(Operation::CommitTransaction.into());
					}

					Ready
				}
				// TODO - simplify with or_patterns when rust 1.53.0 is stable (https://github.com/rust-lang/rust/issues/54883)
				(Dragging, Abort) => {
					responses.push_back(Operation::DiscardWorkingFolder.into());

					Ready
				}
				(Ready, LockAspectRatio) => update_state_no_op(&mut data.constrain_to_square, true, Ready),
				(Ready, UnlockAspectRatio) => update_state_no_op(&mut data.constrain_to_square, false, Ready),
				(Dragging, LockAspectRatio) => update_state(|data| &mut data.constrain_to_square, true, tool_data, data, input, responses, Dragging),
				(Dragging, UnlockAspectRatio) => update_state(|data| &mut data.constrain_to_square, false, tool_data, data, input, responses, Dragging),

				(Ready, Center) => update_state_no_op(&mut data.center_around_cursor, true, Ready),
				(Ready, UnCenter) => update_state_no_op(&mut data.center_around_cursor, false, Ready),
				(Dragging, Center) => update_state(|data| &mut data.center_around_cursor, true, tool_data, data, input, responses, Dragging),
				(Dragging, UnCenter) => update_state(|data| &mut data.center_around_cursor, false, tool_data, data, input, responses, Dragging),
				_ => self,
			}
		} else {
			self
		}
	}
}

fn update_state_no_op(state: &mut bool, value: bool, new_state: RectangleToolFsmState) -> RectangleToolFsmState {
	*state = value;
	new_state
}

fn update_state(
	state: fn(&mut RectangleToolData) -> &mut bool,
	value: bool,
	tool_data: &DocumentToolData,
	data: &mut RectangleToolData,
	input: &InputPreprocessor,
	responses: &mut VecDeque<Message>,
	new_state: RectangleToolFsmState,
) -> RectangleToolFsmState {
	*(state(data)) = value;

	responses.push_back(Operation::ClearWorkingFolder.into());
	responses.push_back(make_operation(data, tool_data, input));

	new_state
}

fn make_operation(data: &RectangleToolData, tool_data: &DocumentToolData, input: &InputPreprocessor) -> Message {
	let (x0, y0) = data.drag_start.to_canvas_position(&input.canvas_transform, false).into();
	let (x1, y1) = data.drag_current.to_canvas_position(&input.canvas_transform, false).into();

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
		rotation:input.canvas_transform.degrees,
		style: style::PathStyle::new(None, Some(style::Fill::new(tool_data.primary_color))),
	}
	.into()
}

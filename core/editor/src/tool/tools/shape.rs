use crate::input::{mouse::ViewportPosition, InputPreprocessor};
use crate::tool::{DocumentToolData, Fsm, ToolActionHandlerData};
use crate::{document::Document, message_prelude::*};
use document_core::{layers::style, Operation};
use glam::{DAffine2, DVec2};

#[derive(Default)]
pub struct Shape {
	fsm_state: ShapeToolFsmState,
	data: ShapeToolData,
}

#[impl_message(Message, ToolMessage, Shape)]
#[derive(PartialEq, Clone, Debug)]
pub enum ShapeMessage {
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

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Shape {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		self.fsm_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, data.2, responses);
	}
	fn actions(&self) -> ActionList {
		use ShapeToolFsmState::*;
		match self.fsm_state {
			Ready => actions!(ShapeMessageDiscriminant; Undo, DragStart, Center, UnCenter, LockAspectRatio, UnlockAspectRatio),
			Dragging => actions!(ShapeMessageDiscriminant; DragStop, Center, UnCenter, LockAspectRatio, UnlockAspectRatio, MouseMove, Abort),
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShapeToolFsmState {
	Ready,
	Dragging,
}

impl Default for ShapeToolFsmState {
	fn default() -> Self {
		ShapeToolFsmState::Ready
	}
}
#[derive(Clone, Debug, Default)]
struct ShapeToolData {
	drag_start: ViewportPosition,
	drag_current: ViewportPosition,
	constrain_to_square: bool,
	center_around_cursor: bool,
	sides: u8,
}

impl Fsm for ShapeToolFsmState {
	type ToolData = ShapeToolData;

	fn transition(self, event: ToolMessage, document: &Document, tool_data: &DocumentToolData, data: &mut Self::ToolData, input: &InputPreprocessor, responses: &mut VecDeque<Message>) -> Self {
		let transform = document.document.root.transform;
		use ShapeMessage::*;
		use ShapeToolFsmState::*;
		if let ToolMessage::Shape(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					data.drag_start = input.mouse.position;
					data.drag_current = input.mouse.position;

					data.sides = 6;

					responses.push_back(Operation::MountWorkingFolder { path: vec![] }.into());
					Dragging
				}
				(Dragging, MouseMove) => {
					data.drag_current = input.mouse.position;
					responses.push_back(Operation::ClearWorkingFolder.into());
					responses.push_back(make_operation(data, tool_data, transform));

					Dragging
				}
				(Dragging, DragStop) => {
					data.drag_current = input.mouse.position;
					responses.push_back(Operation::ClearWorkingFolder.into());
					// TODO - introduce comparison threshold when operating with canvas coordinates (https://github.com/GraphiteEditor/Graphite/issues/100)
					if data.drag_start != data.drag_current {
						responses.push_back(make_operation(data, tool_data, transform));
						responses.push_back(Operation::CommitTransaction.into());
					}

					Ready
				}
				(Dragging, Abort) => {
					responses.push_back(Operation::DiscardWorkingFolder.into());

					Ready
				}

				(Ready, LockAspectRatio) => update_state_no_op(&mut data.constrain_to_square, true, Ready),
				(Ready, UnlockAspectRatio) => update_state_no_op(&mut data.constrain_to_square, false, Ready),
				(Dragging, LockAspectRatio) => update_state(|data| &mut data.constrain_to_square, true, tool_data, data, responses, Dragging, transform),
				(Dragging, UnlockAspectRatio) => update_state(|data| &mut data.constrain_to_square, false, tool_data, data, responses, Dragging, transform),

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

fn update_state_no_op(state: &mut bool, value: bool, new_state: ShapeToolFsmState) -> ShapeToolFsmState {
	*state = value;
	new_state
}

fn update_state(
	state: fn(&mut ShapeToolData) -> &mut bool,
	value: bool,
	tool_data: &DocumentToolData,
	data: &mut ShapeToolData,
	responses: &mut VecDeque<Message>,
	new_state: ShapeToolFsmState,
	transform: DAffine2,
) -> ShapeToolFsmState {
	*(state(data)) = value;

	responses.push_back(Operation::ClearWorkingFolder.into());
	responses.push_back(make_operation(data, tool_data, transform));

	new_state
}

fn make_operation(data: &ShapeToolData, tool_data: &DocumentToolData, transform: DAffine2) -> Message {
	let x0 = data.drag_start.x as f64;
	let y0 = data.drag_start.y as f64;
	let x1 = data.drag_current.x as f64;
	let y1 = data.drag_current.y as f64;

	// TODO: Use regular polygon's aspect ration for constraining rather than a square.
	let (x0, y0, x1, y1, equal_sides) = if data.constrain_to_square {
		let (x_dir, y_dir) = ((x1 - x0).signum(), (y1 - y0).signum());
		let max_dist = f64::max((x1 - x0).abs(), (y1 - y0).abs());
		if data.center_around_cursor {
			(x0 - max_dist * x_dir, y0 - max_dist * y_dir, x0 + max_dist * x_dir, y0 + max_dist * y_dir, true)
		} else {
			(x0, y0, x0 + max_dist * x_dir, y0 + max_dist * y_dir, true)
		}
	} else {
		let (x0, y0) = if data.center_around_cursor {
			let delta_x = x1 - x0;
			let delta_y = y1 - y0;

			(x0 - delta_x, y0 - delta_y)
		} else {
			(x0, y0)
		};
		(x0, y0, x1, y1, false)
	};

	Operation::AddShape {
		path: vec![],
		insert_index: -1,
		transform: (transform.inverse() * glam::DAffine2::from_scale_angle_translation(DVec2::new(x1 - x0, y1 - y0), 0., DVec2::new(x0, y0))).to_cols_array(),
		equal_sides,
		sides: data.sides,
		style: style::PathStyle::new(None, Some(style::Fill::new(tool_data.primary_color))),
	}
	.into()
}

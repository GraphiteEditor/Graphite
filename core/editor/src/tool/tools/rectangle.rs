use crate::input::{mouse::ViewportPosition, InputPreprocessor};
use crate::tool::{DocumentToolData, Fsm, ToolActionHandlerData};
use crate::{document::DocumentMessageHandler, message_prelude::*};
use document_core::{layers::style, Operation};
use glam::{DAffine2, DVec2};

#[derive(Default)]
pub struct Rectangle {
	fsm_state: RectangleToolFsmState,
	data: RectangleToolData,
}

#[impl_message(Message, ToolMessage, Rectangle)]
#[derive(PartialEq, Clone, Debug, Hash)]
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
#[derive(Clone, Debug, Default, Hash, Eq, PartialEq)]
struct RectangleToolData {
	drag_start: ViewportPosition,
	drag_current: ViewportPosition,
	constrain_to_square: bool,
	center_around_cursor: bool,
	shape_id: Option<LayerId>,
}

impl Fsm for RectangleToolFsmState {
	type ToolData = RectangleToolData;

	fn transition(
		self,
		event: ToolMessage,
		document: &DocumentMessageHandler,
		tool_data: &DocumentToolData,
		data: &mut Self::ToolData,
		input: &InputPreprocessor,
		responses: &mut VecDeque<Message>,
	) -> Self {
		let transform = document.document.root.transform;
		use RectangleMessage::*;
		use RectangleToolFsmState::*;
		if let ToolMessage::Rectangle(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					data.drag_start = input.mouse.position;
					data.drag_current = input.mouse.position;
					responses.push_back(DocumentMessage::StartTransaction.into());

					data.shape_id = Some(generate_hash(&*responses, input, document.document.hash()));
					responses.push_back(DocumentMessage::DeselectAllLayers.into());
					responses.push_back(make_operation(data, tool_data, transform));
					Dragging
				}
				(Dragging, MouseMove) => {
					data.drag_current = input.mouse.position;

					responses.push_back(make_transform(data, transform));

					Dragging
				}
				(Dragging, DragStop) => {
					data.drag_current = input.mouse.position;

					// TODO - introduce comparison threshold when operating with canvas coordinates (https://github.com/GraphiteEditor/Graphite/issues/100)
					if data.drag_start != data.drag_current {
						//responses.push_back(DocumentMessage::DeselectAllLayers.into());
						//responses.push_back(DocumentMessage::SelectLayers(vec![vec![data.shape_id.unwrap()]]).into());
						responses.push_back(DocumentMessage::CommitTransaction.into());
					}

					data.shape_id = None;
					Ready
				}
				// TODO - simplify with or_patterns when rust 1.53.0 is stable (https://github.com/rust-lang/rust/issues/54883)
				(Dragging, Abort) => {
					responses.push_back(DocumentMessage::AbortTransaction.into());
					data.shape_id = None;

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

fn update_state_no_op(state: &mut bool, value: bool, new_state: RectangleToolFsmState) -> RectangleToolFsmState {
	*state = value;
	new_state
}

fn update_state(
	state: fn(&mut RectangleToolData) -> &mut bool,
	value: bool,
	tool_data: &DocumentToolData,
	data: &mut RectangleToolData,
	responses: &mut VecDeque<Message>,
	new_state: RectangleToolFsmState,
	transform: DAffine2,
) -> RectangleToolFsmState {
	*(state(data)) = value;

	responses.push_back(make_operation(data, tool_data, transform));

	new_state
}

fn make_transform(data: &RectangleToolData, transform: DAffine2) -> Message {
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

	Operation::SetLayerTransform {
		path: vec![data.shape_id.unwrap()],
		transform: (transform.inverse() * glam::DAffine2::from_scale_angle_translation(DVec2::new(x1 - x0, y1 - y0), 0., DVec2::new(x0, y0))).to_cols_array(),
	}
	.into()
}

fn make_operation(data: &RectangleToolData, tool_data: &DocumentToolData, transform: DAffine2) -> Message {
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
		path: vec![data.shape_id.unwrap()],
		insert_index: -1,
		transform: (transform.inverse() * glam::DAffine2::from_scale_angle_translation(DVec2::new(x1 - x0, y1 - y0), 0., DVec2::new(x0, y0))).to_cols_array(),
		style: style::PathStyle::new(None, Some(style::Fill::new(tool_data.primary_color))),
	}
	.into()
}

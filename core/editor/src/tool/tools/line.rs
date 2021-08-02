use crate::consts::LINE_ROTATE_SNAP_ANGLE;
use crate::input::{mouse::ViewportPosition, InputPreprocessor};
use crate::tool::{DocumentToolData, Fsm, ToolActionHandlerData};
use crate::{document::DocumentMessageHandler, message_prelude::*};
use document_core::{layers::style, Operation};
use glam::{DAffine2, DVec2};

use std::f64::consts::PI;

#[derive(Default)]
pub struct Line {
	fsm_state: LineToolFsmState,
	data: LineToolData,
}

#[impl_message(Message, ToolMessage, Line)]
#[derive(PartialEq, Clone, Debug, Hash)]
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
	path: Option<Vec<LayerId>>,
}

impl Fsm for LineToolFsmState {
	type ToolData = LineToolData;

	fn transition(
		self,
		event: ToolMessage,
		document: &DocumentMessageHandler,
		tool_data: &DocumentToolData,
		data: &mut Self::ToolData,
		input: &InputPreprocessor,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use LineMessage::*;
		use LineToolFsmState::*;
		if let ToolMessage::Line(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					data.drag_start = input.mouse.position;
					responses.push_back(DocumentMessage::StartTransaction.into());
					data.path = Some(vec![generate_hash(&*responses, input, document.document.hash())]);
					responses.push_back(DocumentMessage::DeselectAllLayers.into());

					responses.push_back(
						Operation::AddLine {
							path: data.path.clone().unwrap(),
							insert_index: -1,
							transform: DAffine2::ZERO.to_cols_array(),
							style: style::PathStyle::new(Some(style::Stroke::new(tool_data.primary_color, 5.)), None),
						}
						.into(),
					);

					Dragging
				}
				(Dragging, MouseMove) => {
					data.drag_current = input.mouse.position;

					responses.push_back(generate_transform(data));

					Dragging
				}
				(Dragging, DragStop) => {
					data.drag_current = input.mouse.position;

					// TODO - introduce comparison threshold when operating with canvas coordinates (https://github.com/GraphiteEditor/Graphite/issues/100)
					match data.drag_start == input.mouse.position {
						true => responses.push_back(DocumentMessage::AbortTransaction.into()),
						false => responses.push_back(DocumentMessage::CommitTransaction.into()),
					}

					data.path = None;

					Ready
				}
				(Dragging, Abort) => {
					responses.push_back(DocumentMessage::AbortTransaction.into());
					data.path = None;
					Ready
				}
				(Ready, LockAngle) => update_state_no_op(&mut data.lock_angle, true, Ready),
				(Ready, UnlockAngle) => update_state_no_op(&mut data.lock_angle, false, Ready),
				(Dragging, LockAngle) => update_state(|data| &mut data.lock_angle, true, data, responses, Dragging),
				(Dragging, UnlockAngle) => update_state(|data| &mut data.lock_angle, false, data, responses, Dragging),

				(Ready, SnapToAngle) => update_state_no_op(&mut data.snap_angle, true, Ready),
				(Ready, UnSnapToAngle) => update_state_no_op(&mut data.snap_angle, false, Ready),
				(Dragging, SnapToAngle) => update_state(|data| &mut data.snap_angle, true, data, responses, Dragging),
				(Dragging, UnSnapToAngle) => update_state(|data| &mut data.snap_angle, false, data, responses, Dragging),

				(Ready, Center) => update_state_no_op(&mut data.center_around_cursor, true, Ready),
				(Ready, UnCenter) => update_state_no_op(&mut data.center_around_cursor, false, Ready),
				(Dragging, Center) => update_state(|data| &mut data.center_around_cursor, true, data, responses, Dragging),
				(Dragging, UnCenter) => update_state(|data| &mut data.center_around_cursor, false, data, responses, Dragging),
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

fn update_state(state: fn(&mut LineToolData) -> &mut bool, value: bool, data: &mut LineToolData, responses: &mut VecDeque<Message>, new_state: LineToolFsmState) -> LineToolFsmState {
	*(state(data)) = value;

	responses.push_back(generate_transform(data));

	new_state
}

fn generate_transform(data: &mut LineToolData) -> Message {
	let mut start = data.drag_start.as_f64();
	let stop = data.drag_current.as_f64();

	let mut dir = stop - start;

	let mut angle = f64::atan2(dir.x, dir.y);

	if data.lock_angle {
		angle = data.angle
	};

	if data.snap_angle {
		let snap_resolution = LINE_ROTATE_SNAP_ANGLE;
		angle = (angle * snap_resolution / PI).round() / snap_resolution * PI;
	}

	data.angle = angle;

	dir = DVec2::new(f64::sin(angle), f64::cos(angle)) * dir.length();

	if data.center_around_cursor {
		start -= dir / 2.;
	}

	Operation::SetLayerTransformInViewport {
		path: data.path.clone().unwrap(),
		transform: glam::DAffine2::from_scale_angle_translation(dir, 0., start).to_cols_array(),
	}
	.into()
}

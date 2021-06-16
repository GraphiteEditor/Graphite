use crate::input::InputPreprocessor;
use crate::tool::{DocumentToolData, Fsm, ToolActionHandlerData};
use crate::{message_prelude::*, SvgDocument};
use document_core::{layers::style, Operation};
use glam::{DAffine2, DVec2};

#[derive(Default)]
pub struct Pen {
	fsm_state: PenToolFsmState,
	data: PenToolData,
}

#[impl_message(Message, ToolMessage, Pen)]
#[derive(PartialEq, Clone, Debug)]
pub enum PenMessage {
	Undo,
	DragStart,
	DragStop,
	MouseMove,
	Confirm,
	Abort,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PenToolFsmState {
	Ready,
	Dragging,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Pen {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		self.fsm_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, data.2, responses);
	}
	fn actions(&self) -> ActionList {
		use PenToolFsmState::*;
		match self.fsm_state {
			Ready => actions!(PenMessageDiscriminant; Undo, DragStart, DragStop, Confirm, Abort),
			Dragging => actions!(PenMessageDiscriminant; DragStop, MouseMove, Confirm, Abort),
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
	points: Vec<DAffine2>,
	next_point: DAffine2,
}

impl Fsm for PenToolFsmState {
	type ToolData = PenToolData;

	fn transition(self, event: ToolMessage, document: &SvgDocument, tool_data: &DocumentToolData, data: &mut Self::ToolData, input: &InputPreprocessor, responses: &mut VecDeque<Message>) -> Self {
		let transform = document.root.transform;
		let pos = transform.inverse() * DAffine2::from_translation(DVec2::new(input.mouse.position.x as f64, input.mouse.position.y as f64));

		use PenMessage::*;
		use PenToolFsmState::*;
		if let ToolMessage::Pen(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					responses.push_back(Operation::MountWorkingFolder { path: vec![] }.into());

					data.points.push(pos);
					data.next_point = pos;

					Dragging
				}
				(Dragging, DragStop) => {
					// TODO - introduce comparison threshold when operating with canvas coordinates (https://github.com/GraphiteEditor/Graphite/issues/100)
					if data.points.last() != Some(&pos) {
						data.points.push(pos);
						data.next_point = pos;
					}

					responses.push_back(Operation::ClearWorkingFolder.into());
					responses.push_back(make_operation(data, tool_data, true));

					Dragging
				}
				(Dragging, MouseMove) => {
					data.next_point = pos;

					responses.push_back(Operation::ClearWorkingFolder.into());
					responses.push_back(make_operation(data, tool_data, true));

					Dragging
				}
				// TODO - simplify with or_patterns when rust 1.53.0 is stable  (https://github.com/rust-lang/rust/issues/54883)
				(Dragging, Confirm) => {
					responses.push_back(Operation::ClearWorkingFolder.into());

					if data.points.len() >= 2 {
						responses.push_back(make_operation(data, tool_data, false));
						responses.push_back(Operation::CommitTransaction.into());
					} else {
						responses.push_back(Operation::DiscardWorkingFolder.into());
					}

					data.points.clear();

					Ready
				}
				(Dragging, Abort) => {
					responses.push_back(Operation::DiscardWorkingFolder.into());
					data.points.clear();

					Ready
				}
				_ => self,
			}
		} else {
			self
		}
	}
}

fn make_operation(data: &PenToolData, tool_data: &DocumentToolData, show_preview: bool) -> Message {
	let mut points: Vec<(f64, f64)> = data.points.iter().map(|p| (p.translation.x, p.translation.y)).collect();
	if show_preview {
		points.push((data.next_point.translation.x, data.next_point.translation.y))
	}
	Operation::AddPen {
		path: vec![],
		insert_index: -1,
		cols: [1., 0., 0., 1., 0., 0.],
		points,
		style: style::PathStyle::new(Some(style::Stroke::new(tool_data.primary_color, 5.)), Some(style::Fill::none())),
	}
	.into()
}

use crate::input::{InputPreprocessor, mouse::CanvasPosition};
use crate::tool::{DocumentToolData, Fsm, ToolActionHandlerData};
use crate::{message_prelude::*, SvgDocument};
use document_core::{layers::style, Operation};

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
	points: Vec<CanvasPosition>,
	next_point: CanvasPosition,
}

impl Fsm for PenToolFsmState {
	type ToolData = PenToolData;

	fn transition(self, event: ToolMessage, _document: &SvgDocument, tool_data: &DocumentToolData, data: &mut Self::ToolData, input: &InputPreprocessor, responses: &mut VecDeque<Message>) -> Self {
		use PenMessage::*;
		use PenToolFsmState::*;
		if let ToolMessage::Pen(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					responses.push_back(Operation::MountWorkingFolder { path: vec![] }.into());

					let canvas_position = input.mouse.position.to_canvas_position(&input.canvas_transform);
					data.points.push(canvas_position);
					data.next_point = canvas_position;

					Dragging
				}
				(Dragging, DragStop) => {
					let canvas_position = input.mouse.position.to_canvas_position(&input.canvas_transform);
					// TODO - introduce comparison threshold when operating with canvas coordinates (https://github.com/GraphiteEditor/Graphite/issues/100)
					if data.points.last() != Some(&canvas_position) {
						data.points.push(canvas_position);
						data.next_point = canvas_position;
					}

					responses.push_back(Operation::ClearWorkingFolder.into());
					responses.push_back(make_operation(data, tool_data, true));

					Dragging
				}
				(Dragging, MouseMove) => {
					data.next_point = input.mouse.position.to_canvas_position(&input.canvas_transform);

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
	let mut points: Vec<(f64, f64)> = data.points.iter().map(|p| (p.x as f64, p.y as f64)).collect();
	if show_preview {
		points.push((data.next_point.x as f64, data.next_point.y as f64))
	}
	Operation::AddPen {
		path: vec![],
		insert_index: -1,
		points,
		style: style::PathStyle::new(Some(style::Stroke::new(tool_data.primary_color, 5.)), Some(style::Fill::none())),
	}
	.into()
}

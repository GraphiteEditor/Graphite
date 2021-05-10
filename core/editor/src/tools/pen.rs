use crate::events::ViewportPosition;
use crate::tools::Fsm;
use crate::SvgDocument;

use crate::{
	dispatcher::{Action, ActionHandler, InputPreprocessor, Response},
	tools::{DocumentToolData, ToolActionHandlerData},
};
use document_core::layers::style;
use document_core::Operation;

#[derive(Default)]
pub struct Pen {
	fsm_state: PenToolFsmState,
	data: PenToolData,
}

impl<'a> ActionHandler<ToolActionHandlerData<'a>> for Pen {
	fn process_action(&mut self, data: ToolActionHandlerData<'a>, input_preprocessor: &InputPreprocessor, action: &Action, responses: &mut Vec<Response>, operations: &mut Vec<Operation>) -> bool {
		let (consumed, state) = self.fsm_state.transition(action, data.0, data.1, &mut self.data, input_preprocessor, responses, operations);
		self.fsm_state = state;
		consumed
	}
	actions_fn!();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PenToolFsmState {
	Ready,
	LmbDown,
}

impl Default for PenToolFsmState {
	fn default() -> Self {
		PenToolFsmState::Ready
	}
}
#[derive(Clone, Debug, Default)]
struct PenToolData {
	points: Vec<ViewportPosition>,
	next_point: ViewportPosition,
}

impl Fsm for PenToolFsmState {
	type ToolData = PenToolData;

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
			(PenToolFsmState::Ready, Action::LmbDown) => {
				operations.push(Operation::MountWorkingFolder { path: vec![] });

				data.points.push(input.mouse_state.position);
				data.next_point = input.mouse_state.position;

				(true, PenToolFsmState::LmbDown)
			}
			(PenToolFsmState::LmbDown, Action::LmbUp) => {
				// TODO - introduce comparison threshold when operating with canvas coordinates (https://github.com/GraphiteEditor/Graphite/issues/100)
				if data.points.last() != Some(&input.mouse_state.position) {
					data.points.push(input.mouse_state.position);
					data.next_point = input.mouse_state.position;
				}

				(true, PenToolFsmState::LmbDown)
			}
			(PenToolFsmState::LmbDown, Action::MouseMove) => {
				data.next_point = input.mouse_state.position;

				operations.push(Operation::ClearWorkingFolder);
				operations.push(make_operation(data, tool_data, true));

				(true, PenToolFsmState::LmbDown)
			}
			// TODO - simplify with or_patterns when rust 1.53.0 is stable  (https://github.com/rust-lang/rust/issues/54883)
			(PenToolFsmState::LmbDown, Action::Confirm) | (PenToolFsmState::LmbDown, Action::Abort) | (PenToolFsmState::LmbDown, Action::RmbDown) => {
				operations.push(Operation::ClearWorkingFolder);

				if data.points.len() >= 2 {
					operations.push(make_operation(data, tool_data, false));
					operations.push(Operation::CommitTransaction);
				} else {
					operations.push(Operation::DiscardWorkingFolder);
				}

				data.points.clear();

				(true, PenToolFsmState::Ready)
			}
			_ => (false, self),
		}
	}
}

fn make_operation(data: &PenToolData, tool_data: &DocumentToolData, show_preview: bool) -> Operation {
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
}

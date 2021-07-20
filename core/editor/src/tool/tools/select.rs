use document_core::bounding_box::merge_bounding_boxes;
use document_core::color::Color;
use document_core::layers::style;
use document_core::layers::style::Fill;
use document_core::layers::style::Stroke;
use document_core::Operation;
use glam::{DAffine2, DVec2};

use crate::input::{mouse::ViewportPosition, InputPreprocessor};
use crate::tool::{DocumentToolData, Fsm, ToolActionHandlerData};
use crate::{consts::SELECTION_TOLERANCE, document::Document, message_prelude::*};

#[derive(Default)]
pub struct Select {
	fsm_state: SelectToolFsmState,
	data: SelectToolData,
}

#[impl_message(Message, ToolMessage, Select)]
#[derive(PartialEq, Clone, Debug)]
pub enum SelectMessage {
	Init,
	DragStart,
	DragStop,
	MouseMove,
	Abort,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Select {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		self.fsm_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, data.2, responses);
	}
	fn actions(&self) -> ActionList {
		use SelectToolFsmState::*;
		match self.fsm_state {
			Ready => actions!(SelectMessageDiscriminant;  DragStart),
			Dragging => actions!(SelectMessageDiscriminant; DragStop, MouseMove, Abort),
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectToolFsmState {
	Ready,
	Dragging,
}

impl Default for SelectToolFsmState {
	fn default() -> Self {
		SelectToolFsmState::Ready
	}
}

#[derive(Clone, Debug, Default)]
struct SelectToolData {
	drag_start: ViewportPosition,
	drag_current: ViewportPosition,
}

impl Fsm for SelectToolFsmState {
	type ToolData = SelectToolData;

	fn transition(self, event: ToolMessage, document: &Document, tool_data: &DocumentToolData, data: &mut Self::ToolData, input: &InputPreprocessor, responses: &mut VecDeque<Message>) -> Self {
		let transform = document.document.root.transform;
		use SelectMessage::*;
		use SelectToolFsmState::*;
		if let ToolMessage::Select(event) = event {
			match (self, event) {
				(_, Init) => {
					responses.push_back(Operation::MountWorkingFolder { path: vec![] }.into());
					make_selection_bounding_box(document, responses);
					self
				}
				(Ready, DragStart) => {
					data.drag_start = input.mouse.position;
					data.drag_current = input.mouse.position;

					Dragging
				}
				(Dragging, MouseMove) => {
					data.drag_current = input.mouse.position;

					responses.push_back(Operation::ClearWorkingFolder.into());
					responses.push_back(make_operation(data, tool_data, transform));

					make_selection_bounding_box(document, responses);

					Dragging
				}
				(Dragging, DragStop) => {
					data.drag_current = input.mouse.position;

					responses.push_back(Operation::ClearWorkingFolder.into());

					let (point_1, point_2) = if data.drag_start == data.drag_current {
						let (x, y) = (data.drag_current.x as f64, data.drag_current.y as f64);
						(
							DVec2::new(x - SELECTION_TOLERANCE, y - SELECTION_TOLERANCE),
							DVec2::new(x + SELECTION_TOLERANCE, y + SELECTION_TOLERANCE),
						)
					} else {
						(
							DVec2::new(data.drag_start.x as f64, data.drag_start.y as f64),
							DVec2::new(data.drag_current.x as f64, data.drag_current.y as f64),
						)
					};

					let quad = [
						DVec2::new(point_1.x, point_1.y),
						DVec2::new(point_2.x, point_1.y),
						DVec2::new(point_2.x, point_2.y),
						DVec2::new(point_1.x, point_2.y),
					];

					responses.push_back(Operation::DiscardWorkingFolder.into());
					let selected = if data.drag_start == data.drag_current {
						if let Some(intersection) = document.document.intersects_quad_root(quad).last() {
							vec![intersection.clone()]
						} else {
							vec![]
						}
					} else {
						document.document.intersects_quad_root(quad)
					};
					responses.push_back(DocumentMessage::SelectLayers(selected.clone()).into());

					responses.push_back(Operation::MountWorkingFolder { path: vec![] }.into());
					make_paths_bounding_box(selected, document, responses);

					Ready
				}
				(Dragging, Abort) => {
					responses.push_back(Operation::ClearWorkingFolder.into());

					make_selection_bounding_box(document, responses);

					Ready
				}
				_ => self,
			}
		} else {
			self
		}
	}
}

fn make_operation(data: &SelectToolData, _tool_data: &DocumentToolData, transform: DAffine2) -> Message {
	let x0 = data.drag_start.x as f64;
	let y0 = data.drag_start.y as f64;
	let x1 = data.drag_current.x as f64;
	let y1 = data.drag_current.y as f64;

	Operation::AddRect {
		path: vec![],
		insert_index: -1,
		transform: (transform.inverse() * glam::DAffine2::from_scale_angle_translation(DVec2::new(x1 - x0, y1 - y0), 0., DVec2::new(x0, y0))).to_cols_array(),
		style: style::PathStyle::new(Some(Stroke::new(Color::from_rgb8(0x31, 0x94, 0xD6), 2.0)), Some(Fill::none())),
	}
	.into()
}

fn make_selection_bounding_box(document: &Document, responses: &mut VecDeque<Message>) {
	let selected_layers_paths = document.layer_data.iter().filter_map(|(path, layer_data)| layer_data.selected.then(|| path)).cloned().collect();
	make_paths_bounding_box(selected_layers_paths, document, responses);
}

fn make_paths_bounding_box(paths: Vec<Vec<LayerId>>, document: &Document, responses: &mut VecDeque<Message>) {
	let non_empty_bounding_boxes = paths.iter().filter_map(|path| {
		if let Ok(some_bounding_box) = document.document.layer_axis_aligned_bounding_box(path) {
			some_bounding_box
		} else {
			None
		}
	});

	if let Some([min, max]) = non_empty_bounding_boxes.reduce(merge_bounding_boxes) {
		let x0 = min.x - 1.0;
		let y0 = min.y - 1.0;
		let x1 = max.x + 1.0;
		let y1 = max.y + 1.0;
		let root_transform = document.document.root.transform;
		responses.push_back(
			Operation::AddRect {
				path: vec![],
				insert_index: -1,
				transform: (root_transform.inverse() * glam::DAffine2::from_scale_angle_translation(DVec2::new(x1 - x0, y1 - y0), 0., DVec2::new(x0, y0))).to_cols_array(),
				style: style::PathStyle::new(Some(Stroke::new(Color::from_rgb8(0x00, 0xa6, 0xfb), 1.0)), Some(Fill::none())),
			}
			.into(),
		)
	}
}

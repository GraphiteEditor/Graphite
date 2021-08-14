use graphene::color::Color;
use graphene::layers::style;
use graphene::layers::style::Fill;
use graphene::layers::style::Stroke;
use graphene::Operation;
use graphene::Quad;

use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};

use crate::input::{mouse::ViewportPosition, InputPreprocessor};
use crate::tool::{DocumentToolData, Fsm, ToolActionHandlerData};
use crate::{
	consts::SELECTION_TOLERANCE,
	document::{AlignAggregate, AlignAxis, DocumentMessageHandler, FlipAxis},
	message_prelude::*,
};

#[derive(Default)]
pub struct Select {
	fsm_state: SelectToolFsmState,
	data: SelectToolData,
}

#[impl_message(Message, ToolMessage, Select)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, Hash)]
pub enum SelectMessage {
	Selected,
	DragStart,
	DragStop,
	MouseMove,
	Abort,

	UpdateBoundingBox,

	Align(AlignAxis, AlignAggregate),
	FlipHorizontal,
	FlipVertical,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Select {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		self.fsm_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, data.2, responses);
	}
	fn actions(&self) -> ActionList {
		use SelectToolFsmState::*;
		match self.fsm_state {
			Ready => actions!(SelectMessageDiscriminant; DragStart),
			Dragging => actions!(SelectMessageDiscriminant; DragStop, MouseMove),
			DrawingBox => actions!(SelectMessageDiscriminant; DragStop, MouseMove, Abort),
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectToolFsmState {
	Ready,
	Dragging,
	DrawingBox,
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
	layers_dragging: Vec<Vec<LayerId>>, // Paths and offsets
	box_id: Option<Vec<LayerId>>,
}

impl SelectToolData {
	fn selection_quad(&self) -> Quad {
		let bbox = self.selection_box();
		Quad::from_box(bbox)
	}

	fn selection_box(&self) -> [DVec2; 2] {
		if self.drag_current == self.drag_start {
			let tolerance = DVec2::splat(SELECTION_TOLERANCE);
			[self.drag_start.as_f64() - tolerance, self.drag_start.as_f64() + tolerance]
		} else {
			[self.drag_start.as_f64(), self.drag_current.as_f64()]
		}
	}
}

impl Fsm for SelectToolFsmState {
	type ToolData = SelectToolData;

	fn transition(
		self,
		event: ToolMessage,
		document: &DocumentMessageHandler,
		_tool_data: &DocumentToolData,
		data: &mut Self::ToolData,
		input: &InputPreprocessor,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use SelectMessage::*;
		use SelectToolFsmState::*;
		match event {
			ToolMessage::CanvasRotated | ToolMessage::SelectionUpdated => {
				responses.push_back(SelectMessage::UpdateBoundingBox.into());
				self
			}
			ToolMessage::Select(event) => match (self, event) {
				(_, UpdateBoundingBox) => {
					if data.box_id.is_some() {
						place_bounding_box_around_selection(&data.box_id, document, responses);
					}
					self
				}
				(Ready, Selected) => {
					if data.box_id.is_none() {
						data.box_id = Some(vec![generate_hash(&*responses, input, document.document.hash())]);
						responses.push_back(
							Operation::AddBoundingBox {
								path: data.box_id.clone().unwrap(),
								transform: DAffine2::ZERO.to_cols_array(),
								style: style::PathStyle::new(Some(Stroke::new(Color::from_rgb8(0x00, 0xA8, 0xFF), 1.0)), Some(Fill::none())),
							}
							.into(),
						);
						place_bounding_box_around_selection(&data.box_id, document, responses);
					}
					self
				}
				(Ready, DragStart) => {
					data.drag_start = input.mouse.position;
					data.drag_current = input.mouse.position;
					let mut selected: Vec<_> = document.selected_layers().cloned().collect();
					let quad = data.selection_quad();
					let intersection = document.document.intersects_quad_root(quad);
					// If no layer is currently selected and the user clicks on a shape, select that.
					if selected.is_empty() {
						if let Some(layer) = intersection.last() {
							selected.push(layer.clone());
							responses.push_back(DocumentMessage::SelectLayers(selected.clone()).into());
						}
					}
					// If the user clicks on a layer that is in their current selection, go into the dragging mode.
					// Otherwise enter the box select mode
					if selected.iter().any(|path| intersection.contains(path)) {
						data.layers_dragging = selected;
						Dragging
					} else {
						responses.push_back(DocumentMessage::DeselectAllLayers.into());
						DrawingBox
					}
				}
				(Dragging, MouseMove) => {
					for path in data.layers_dragging.iter() {
						responses.push_back(
							Operation::TransformLayerInViewport {
								path: path.clone(),
								transform: DAffine2::from_translation(input.mouse.position.as_f64() - data.drag_current.as_f64()).to_cols_array(),
							}
							.into(),
						);
					}
					data.drag_current = input.mouse.position;
					responses.push_back(SelectMessage::UpdateBoundingBox.into());
					Dragging
				}
				(DrawingBox, MouseMove) => {
					data.drag_current = input.mouse.position;
					let half_pixel_offset = DVec2::new(0.5, 0.5);
					let start = data.drag_start.as_f64() + half_pixel_offset;
					let size = data.drag_current.as_f64() - start + half_pixel_offset;

					responses.push_back(
						Operation::SetLayerTransformInViewport {
							path: data.box_id.clone().unwrap(),
							transform: DAffine2::from_scale_angle_translation(size, 0., start).to_cols_array(),
						}
						.into(),
					);
					DrawingBox
				}
				(Dragging, DragStop) => Ready,
				(DrawingBox, DragStop) => {
					let quad = data.selection_quad();
					responses.push_back(DocumentMessage::SelectLayers(document.document.intersects_quad_root(quad)).into());
					// // responses.push_back(Operation::DeleteLayer { path: data.box_id.take().unwrap() }.into());
					Ready
				}
				(Ready, Abort) => {
					if data.box_id.is_some() {
						responses.push_back(Operation::DeleteLayer { path: data.box_id.take().unwrap() }.into());
					}
					Ready
				}
				(_, Abort) => {
					place_bounding_box_around_selection(&data.box_id, document, responses);
					Ready
				}
				(_, Align(axis, aggregate)) => {
					responses.push_back(DocumentMessage::AlignSelectedLayers(axis, aggregate).into());

					self
				}
				(_, FlipHorizontal) => {
					responses.push_back(DocumentMessage::FlipSelectedLayers(FlipAxis::X).into());

					self
				}
				(_, FlipVertical) => {
					responses.push_back(DocumentMessage::FlipSelectedLayers(FlipAxis::Y).into());

					self
				}
				_ => self,
			},
			_ => self,
		}
	}
}

fn place_bounding_box_around_selection(box_id: &Option<Vec<u64>>, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
	let maybe_bounding_box = document.selected_layers_bounding_box();
	let transform = if let Some(bounding_box) = maybe_bounding_box {
		let start = bounding_box[0];
		let size = bounding_box[1] - start;
		DAffine2::from_scale_angle_translation(size, 0., start)
	} else {
		DAffine2::ZERO
	};
	responses.push_back(
		Operation::SetLayerTransformInViewport {
			path: box_id.clone().unwrap(),
			transform: transform.to_cols_array(),
		}
		.into(),
	);
}

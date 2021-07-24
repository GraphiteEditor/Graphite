use document_core::color::Color;
use document_core::layers::style;
use document_core::layers::style::Fill;
use document_core::layers::style::Stroke;
use document_core::Operation;
use glam::{DAffine2, DVec2};
use serde::{Deserialize, Serialize};

use crate::input::{mouse::ViewportPosition, InputPreprocessor};
use crate::tool::{DocumentToolData, Fsm, ToolActionHandlerData};
use crate::{consts::SELECTION_TOLERANCE, document::Document, message_prelude::*};

#[derive(Default)]
pub struct Select {
	fsm_state: SelectToolFsmState,
	data: SelectToolData,
}

#[impl_message(Message, ToolMessage, Select)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum SelectMessage {
	DragStart,
	DragStop,
	MouseMove,
	Abort,

	AlignLeft,
	AlignHorizontalCenter,
	AlignRight,
	AlignTop,
	AlignVerticalCenter,
	AlignBottom,
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
	layers_dragging: Vec<(Vec<LayerId>, DVec2)>, // Paths and offsets
}

impl Fsm for SelectToolFsmState {
	type ToolData = SelectToolData;

	fn transition(self, event: ToolMessage, document: &Document, tool_data: &DocumentToolData, data: &mut Self::ToolData, input: &InputPreprocessor, responses: &mut VecDeque<Message>) -> Self {
		let transform = document.document.root.transform;
		use SelectMessage::*;
		use SelectToolFsmState::*;
		if let ToolMessage::Select(event) = event {
			match (self, event) {
				(Ready, DragStart) => {
					data.drag_start = input.mouse.position;
					data.drag_current = input.mouse.position;

					let (point_1, point_2) = {
						let (x, y) = (data.drag_start.x as f64, data.drag_start.y as f64);
						(
							DVec2::new(x - SELECTION_TOLERANCE, y - SELECTION_TOLERANCE),
							DVec2::new(x + SELECTION_TOLERANCE, y + SELECTION_TOLERANCE),
						)
					};

					let quad = [
						DVec2::new(point_1.x, point_1.y),
						DVec2::new(point_2.x, point_1.y),
						DVec2::new(point_2.x, point_2.y),
						DVec2::new(point_1.x, point_2.y),
					];

					if let Some(intersection) = document.document.intersects_quad_root(quad).last() {
						// TODO: Replace root transformations with functions of the transform api
						let transformed_start = document.document.root.transform.inverse().transform_vector2(data.drag_start.as_dvec2());
						if document.layer_data.get(intersection).map_or(false, |layer_data| layer_data.selected) {
							data.layers_dragging = document
								.layer_data
								.iter()
								.filter_map(|(path, layer_data)| {
									layer_data
										.selected
										.then(|| (path.clone(), document.document.layer(path).unwrap().transform.translation - transformed_start))
								})
								.collect();
						} else {
							responses.push_back(DocumentMessage::SelectLayers(vec![intersection.clone()]).into());
							data.layers_dragging = vec![(intersection.clone(), document.document.layer(intersection).unwrap().transform.translation - transformed_start)]
						}
					} else {
						responses.push_back(Operation::MountWorkingFolder { path: vec![] }.into());
						data.layers_dragging = Vec::new();
					}

					Dragging
				}
				(Dragging, MouseMove) => {
					data.drag_current = input.mouse.position;

					if data.layers_dragging.is_empty() {
						responses.push_back(Operation::ClearWorkingFolder.into());
						responses.push_back(make_operation(data, tool_data, transform));
					} else {
						for (path, offset) in &data.layers_dragging {
							responses.push_back(DocumentMessage::DragLayer(path.clone(), offset.clone()).into());
						}
					}

					Dragging
				}
				(Dragging, DragStop) => {
					data.drag_current = input.mouse.position;

					if data.layers_dragging.is_empty() {
						responses.push_back(Operation::ClearWorkingFolder.into());
						responses.push_back(Operation::DiscardWorkingFolder.into());

						if data.drag_start == data.drag_current {
							responses.push_back(DocumentMessage::SelectLayers(vec![]).into());
						} else {
							let (point_1, point_2) = (
								DVec2::new(data.drag_start.x as f64, data.drag_start.y as f64),
								DVec2::new(data.drag_current.x as f64, data.drag_current.y as f64),
							);

							let quad = [
								DVec2::new(point_1.x, point_1.y),
								DVec2::new(point_2.x, point_1.y),
								DVec2::new(point_2.x, point_2.y),
								DVec2::new(point_1.x, point_2.y),
							];

							responses.push_back(DocumentMessage::SelectLayers(document.document.intersects_quad_root(quad)).into());
						}
					} else {
						data.layers_dragging = Vec::new();
					}

					Ready
				}
				(Dragging, Abort) => {
					responses.push_back(Operation::DiscardWorkingFolder.into());
					data.layers_dragging = Vec::new();

					Ready
				}
				(_, AlignLeft) => {
					align_selected(document, responses, AlignDimension::X, AlignAggregate::Min);

					self
				}
				(_, AlignHorizontalCenter) => {
					align_selected(document, responses, AlignDimension::X, AlignAggregate::Average);

					self
				}
				(_, AlignRight) => {
					align_selected(document, responses, AlignDimension::X, AlignAggregate::Max);

					self
				}
				(_, AlignTop) => {
					align_selected(document, responses, AlignDimension::Y, AlignAggregate::Min);

					self
				}
				(_, AlignVerticalCenter) => {
					align_selected(document, responses, AlignDimension::Y, AlignAggregate::Average);

					self
				}
				(_, AlignBottom) => {
					align_selected(document, responses, AlignDimension::Y, AlignAggregate::Max);

					self
				}
				(_, FlipHorizontal) => {
					let selected_layers = document.layer_data.iter().filter_map(|(path, data)| data.selected.then(|| path.clone()));
					for path in selected_layers {
						responses.push_back(DocumentMessage::FlipLayer(path, true, false).into());
					}

					self
				}
				(_, FlipVertical) => {
					let selected_layers = document.layer_data.iter().filter_map(|(path, data)| data.selected.then(|| path.clone()));
					for path in selected_layers {
						responses.push_back(DocumentMessage::FlipLayer(path, false, true).into());
					}

					self
				}
				_ => self,
			}
		} else {
			self
		}
	}
}

enum AlignDimension {
	X,
	Y,
}

enum AlignAggregate {
	Min,
	Max,
	Average,
}

fn align_selected(document: &Document, responses: &mut VecDeque<Message>, dimension: AlignDimension, aggregate: AlignAggregate) {
	let selected_paths: Vec<Vec<LayerId>> = document.layer_data.iter().filter_map(|(path, data)| data.selected.then(|| path.clone())).collect();
	let n_selected = selected_paths.len();
	if n_selected == 0 {
		return;
	}

	let selected_layers = selected_paths.iter().map(|path| {
		let layer = document.document.layer(path).unwrap();
		let point = {
			let bounding_box = layer.bounding_box(layer.transform, layer.style).unwrap();
			match aggregate {
				AlignAggregate::Min => bounding_box[0],
				AlignAggregate::Max => bounding_box[1],
				AlignAggregate::Average => bounding_box[0].lerp(bounding_box[1], 0.5),
			}
		};
		let (bounding_box_coord, translation_coord) = match dimension {
			AlignDimension::X => (point.x, layer.transform.translation.x),
			AlignDimension::Y => (point.y, layer.transform.translation.y),
		};
		(path.clone(), bounding_box_coord, translation_coord)
	});

	let bounding_box_coords = selected_layers.clone().map(|(_, bounding_box_coord, _)| bounding_box_coord);
	let aggregated_coord = match aggregate {
		AlignAggregate::Min => bounding_box_coords.reduce(|a, b| a.min(b)).unwrap(),
		AlignAggregate::Max => bounding_box_coords.reduce(|a, b| a.max(b)).unwrap(),
		AlignAggregate::Average => bounding_box_coords.sum::<f64>() / n_selected as f64,
	};
	for (path, bounding_box_coord, translation_coord) in selected_layers {
		let new_coord = match aggregate {
			AlignAggregate::Min => aggregated_coord - (bounding_box_coord - translation_coord),
			AlignAggregate::Max => aggregated_coord + (translation_coord - bounding_box_coord),
			AlignAggregate::Average => aggregated_coord - (bounding_box_coord - translation_coord),
		};
		match dimension {
			AlignDimension::X => responses.push_back(DocumentMessage::SetLayerCoordinates(path, Some(new_coord), None).into()),
			AlignDimension::Y => responses.push_back(DocumentMessage::SetLayerCoordinates(path, None, Some(new_coord)).into()),
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

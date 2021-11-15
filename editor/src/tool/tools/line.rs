use crate::consts::LINE_ROTATE_SNAP_ANGLE;
use crate::input::keyboard::Key;
use crate::input::{mouse::ViewportPosition, InputPreprocessor};
use crate::tool::{snapping, DocumentToolData, Fsm, ToolActionHandlerData, ToolOptions, ToolType};
use crate::{document::DocumentMessageHandler, message_prelude::*};
use glam::{DAffine2, DVec2};
use graphene::{layers::style, Operation};
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct Line {
	fsm_state: LineToolFsmState,
	data: LineToolData,
}

#[impl_message(Message, ToolMessage, Line)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum LineMessage {
	DragStart,
	DragStop,
	Redraw { center: Key, lock_angle: Key, snap_angle: Key },
	Abort,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Line {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		self.fsm_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, data.2, responses);
	}
	fn actions(&self) -> ActionList {
		use LineToolFsmState::*;
		match self.fsm_state {
			Ready => actions!(LineMessageDiscriminant;  DragStart),
			Dragging => actions!(LineMessageDiscriminant; DragStop, Redraw, Abort),
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
	weight: u32,
	path: Option<Vec<LayerId>>,
	snap_targets: Option<[Vec<f64>; 2]>,
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
					let snap_targets = snapping::get_snap_targets(document, document.all_layers_sorted(), &[]);

					let snapped_position = snapping::snap_position(&snap_targets, input.mouse.position);
					data.drag_start = snapped_position;

					data.snap_targets = Some(snap_targets);

					responses.push_back(DocumentMessage::StartTransaction.into());
					data.path = Some(vec![generate_uuid()]);
					responses.push_back(DocumentMessage::DeselectAllLayers.into());

					data.weight = match tool_data.tool_options.get(&ToolType::Line) {
						Some(&ToolOptions::Line { weight }) => weight,
						_ => 5,
					};

					responses.push_back(
						Operation::AddLine {
							path: data.path.clone().unwrap(),
							insert_index: -1,
							transform: DAffine2::ZERO.to_cols_array(),
							style: style::PathStyle::new(Some(style::Stroke::new(tool_data.primary_color, data.weight as f32)), None),
						}
						.into(),
					);

					Dragging
				}
				(Dragging, Redraw { center, snap_angle, lock_angle }) => {
					data.drag_current = snapping::snap_position(data.snap_targets.as_ref().expect("Snap targets not populated for line tool."), input.mouse.position);

					let values: Vec<_> = [lock_angle, snap_angle, center].iter().map(|k| input.keyboard.get(*k as usize)).collect();
					responses.push_back(generate_transform(data, values[0], values[1], values[2]));

					Dragging
				}
				(Dragging, DragStop) => {
					data.drag_current = snapping::snap_position(data.snap_targets.as_ref().expect("Snap targets not populated for line tool."), input.mouse.position);

					// TODO; introduce comparison threshold when operating with canvas coordinates (https://github.com/GraphiteEditor/Graphite/issues/100)
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
				_ => self,
			}
		} else {
			self
		}
	}
}

fn generate_transform(data: &mut LineToolData, lock: bool, snap: bool, center: bool) -> Message {
	let mut start = data.drag_start;
	let stop = data.drag_current;

	let dir = stop - start;

	let mut angle = -dir.angle_between(DVec2::X);

	if lock {
		angle = data.angle
	};

	if snap {
		let snap_resolution = LINE_ROTATE_SNAP_ANGLE.to_radians();
		angle = (angle / snap_resolution).round() * snap_resolution;
	}

	data.angle = angle;

	let mut scale = dir.length();

	if lock {
		let angle_vec = DVec2::new(angle.cos(), angle.sin());
		scale = dir.dot(angle_vec);
	}

	if center {
		start -= dir / 2.;
	}

	Operation::SetLayerTransformInViewport {
		path: data.path.clone().unwrap(),
		transform: glam::DAffine2::from_scale_angle_translation(DVec2::splat(scale), angle, start).to_cols_array(),
	}
	.into()
}

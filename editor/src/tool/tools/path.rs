use crate::document::DocumentMessageHandler;
use crate::input::keyboard::Key;
use crate::input::{mouse::ViewportPosition, InputPreprocessor};
use crate::message_prelude::*;
use crate::tool::ToolActionHandlerData;
use crate::tool::{DocumentToolData, Fsm};
use glam::{DAffine2, DVec2};
use graphene::color::Color;
use graphene::layers::style;
use graphene::layers::style::Fill;
use graphene::layers::style::Stroke;
use graphene::Operation;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct Path {
	fsm_state: PathToolFsmState,
	data: PathToolData,
}

#[impl_message(Message, ToolMessage, Path)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, Hash)]
pub enum PathMessage {
	Abort,
	SelectedLayersChanged,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for Path {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		self.fsm_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, data.2, responses);
	}
	fn actions(&self) -> ActionList {
		use PathToolFsmState::*;
		match self.fsm_state {
			Ready => actions!(PathMessageDiscriminant;),
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PathToolFsmState {
	Ready,
}

impl Default for PathToolFsmState {
	fn default() -> Self {
		PathToolFsmState::Ready
	}
}

#[derive(Clone, Debug, Default)]
struct PathToolData {
	vector_handle_markers: Option<Vec<LayerId>>,
}

impl PathToolData {}

impl Fsm for PathToolFsmState {
	type ToolData = PathToolData;

	fn transition(
		self,
		event: ToolMessage,
		document: &DocumentMessageHandler,
		_tool_data: &DocumentToolData,
		data: &mut Self::ToolData,
		input: &InputPreprocessor,
		responses: &mut VecDeque<Message>,
	) -> Self {
		use PathMessage::*;
		use PathToolFsmState::*;
		if let ToolMessage::Path(event) = event {
			match (self, event) {
				(_, SelectedLayersChanged) => {
					let response = match data.vector_handle_markers.take() {
						// Some(path) => Operation::DeleteLayer { path }.into(),
						path => {
							let path = path.unwrap_or_else(|| add_marker(responses));
							data.vector_handle_markers = Some(path.clone());

							let transform = DAffine2::IDENTITY.to_cols_array();
							Operation::SetLayerTransformInViewport { path, transform }.into()
						}
						_ => Message::NoOp,
					};
					responses.push_back(response);
					self
				}
				_ => self,
			}
		} else {
			self
		}
	}
}

fn transform_from_canvas(pos1: DVec2, pos2: DVec2) -> [f64; 6] {
	DAffine2::from_scale_angle_translation(pos2 - pos1, 0., pos1).to_cols_array()
}

fn add_marker(responses: &mut VecDeque<Message>) -> Vec<LayerId> {
	let path = vec![generate_uuid()];
	responses.push_back(
		Operation::AddRect {
			path: path.clone(),
			insert_index: 0,
			transform: DAffine2::IDENTITY.to_cols_array(),
			style: style::PathStyle::new(Some(Stroke::new(Color::from_rgb8(0xFF, 0x00, 0x00), 2.0)), Some(Fill::none())),
		}
		.into(),
	);

	path
}

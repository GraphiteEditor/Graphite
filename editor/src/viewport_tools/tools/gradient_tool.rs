use crate::consts::{COLOR_ACCENT, SELECTION_TOLERANCE, VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE};
use crate::document::DocumentMessageHandler;
use crate::frontend::utility_types::MouseCursorIcon;
use crate::input::input_preprocessor::KeyPosition;
use crate::input::keyboard::{Key, MouseMotion};
use crate::input::InputPreprocessorMessageHandler;
use crate::layout::widgets::PropertyHolder;
use crate::message_prelude::*;
use crate::misc::{HintData, HintGroup, HintInfo, KeysGroup};
use crate::viewport_tools::snapping::SnapHandler;
use crate::viewport_tools::tool::{DocumentToolData, Fsm, ToolActionHandlerData};

use graphene::color::Color;
use graphene::intersection::Quad;
use graphene::layers::style::{Fill, Gradient, PathStyle, Stroke};
use graphene::Operation;

use super::shared::transformation_cage::*;

use glam::{DAffine2, DVec2, Vec2Swizzles};
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct GradientTool {
	fsm_state: GradientToolFsmState,
	data: GradientToolData,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Gradient)]
#[derive(PartialEq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum GradientToolMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,
	#[remain::unsorted]
	DocumentIsDirty,

	// Tool-specific messages
	PointerDown,
	PointerMove {
		constrain_axis: Key,
	},
	PointerUp,
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for GradientTool {
	fn process_action(&mut self, action: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if action == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		if action == ToolMessage::UpdateCursor {
			self.fsm_state.update_cursor(responses);
			return;
		}

		let new_state = self.fsm_state.transition(action, data.0, data.1, &mut self.data, &(), data.2, responses);

		if self.fsm_state != new_state {
			self.fsm_state = new_state;
			self.fsm_state.update_hints(responses);
		}
	}

	advertise_actions!(GradientToolMessageDiscriminant; PointerDown, PointerUp, PointerMove, Abort);
}

impl PropertyHolder for GradientTool {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GradientToolFsmState {
	Ready,
	Drawing,
	ResizingBounds,
	Dragging,
}

impl Default for GradientToolFsmState {
	fn default() -> Self {
		GradientToolFsmState::Ready
	}
}

fn from_gradient_space(path: &[LayerId], document: &DocumentMessageHandler) -> DAffine2 {
	let layer = document.graphene_document.layer(path).unwrap();
	let bounds = layer.current_bounding_box().unwrap();
	let bound_transform = DAffine2::from_scale_angle_translation(bounds[1] - bounds[0], 0., bounds[0]);

	document.graphene_document.multiply_transforms(&path[..path.len() - 1]).unwrap() * bound_transform
}

/// Contains info on the overlays for a single gradient
#[derive(Clone, Debug, Default)]
pub struct GradientOverlay {
	pub handles: [Vec<LayerId>; 2],
	pub line: Vec<LayerId>,
}

impl GradientOverlay {
	fn generate_handle(translation: DVec2, responses: &mut VecDeque<Message>) -> Vec<LayerId> {
		let path = vec![generate_uuid()];

		let operation = Operation::AddOverlayEllipse {
			path: path.clone(),
			transform: DAffine2::from_scale_angle_translation(DVec2::splat(VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE), 0., translation).to_cols_array(),
			style: PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), Fill::flat(Color::WHITE)),
		};
		responses.push_back(DocumentMessage::Overlays(operation.into()).into());

		path
	}

	pub fn new(fill: &Gradient, path: &[LayerId], document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) -> Self {
		let transform = from_gradient_space(path, document);
		let Gradient { start, end, .. } = fill;
		let handles = [
			Self::generate_handle(transform.transform_point2(*start), responses),
			Self::generate_handle(transform.transform_point2(*end), responses),
		];

		Self { handles, line: Vec::new() }
	}
	pub fn delete(self, responses: &mut VecDeque<Message>) {
		//responses.push_back(DocumentMessage::Overlays(Operation::DeleteLayer { path: self.line }.into()).into());
		let [start, end] = self.handles;
		responses.push_back(DocumentMessage::Overlays(Operation::DeleteLayer { path: start }.into()).into());
		responses.push_back(DocumentMessage::Overlays(Operation::DeleteLayer { path: end }.into()).into());
	}
}

#[derive(Clone, Debug, Default)]
struct SelectedGradient {
	path: Vec<LayerId>,
	transform: DAffine2,
	gradient: Gradient,
	dragging_start: bool,
}

impl SelectedGradient {
	pub fn new(gradient: Gradient, path: &[LayerId], document: &DocumentMessageHandler) -> Self {
		let transform = from_gradient_space(path, document);
		Self {
			path: path.to_vec(),
			transform,
			gradient,
			dragging_start: false,
		}
	}

	pub fn with_start(mut self, start: DVec2) -> Self {
		self.gradient.start = self.transform.inverse().transform_point2(start);
		self
	}

	pub fn update_gradient(&mut self, mouse: DVec2, responses: &mut VecDeque<Message>) {
		let mouse = self.transform.inverse().transform_point2(mouse);
		if self.dragging_start {
			self.gradient.start = mouse;
		} else {
			self.gradient.end = mouse;
		}
		let fill = Fill::LinearGradient(self.gradient.clone());
		responses.push_back(Operation::SetLayerFill { path: self.path.clone(), fill }.into());
	}
}

#[derive(Clone, Debug, Default)]
struct GradientToolData {
	gradient_overlays: Vec<GradientOverlay>,
	selected_gradient: Option<SelectedGradient>,
	snap_handler: SnapHandler,
	cursor: MouseCursorIcon,
	drag_start: DVec2,
	drag_current: DVec2,
}

impl Fsm for GradientToolFsmState {
	type ToolData = GradientToolData;
	type ToolOptions = ();

	fn transition(
		self,
		event: ToolMessage,
		document: &DocumentMessageHandler,
		tool_data: &DocumentToolData,
		data: &mut Self::ToolData,
		_tool_options: &Self::ToolOptions,
		input: &InputPreprocessorMessageHandler,
		responses: &mut VecDeque<Message>,
	) -> Self {
		if let ToolMessage::Gradient(event) = event {
			match (self, event) {
				(_, GradientToolMessage::DocumentIsDirty) => {
					for layer in document.selected_visible_layers() {}

					self
				}
				(GradientToolFsmState::Ready, GradientToolMessage::PointerDown) => {
					responses.push_back(ToolMessage::DocumentIsDirty.into());

					let tolerance = DVec2::splat(SELECTION_TOLERANCE);
					let quad = Quad::from_box([input.mouse.position - tolerance, input.mouse.position + tolerance]);
					let intersection = document.graphene_document.intersects_quad_root(quad).pop();

					if let Some(intersection) = intersection {
						if !document.selected_layers_contains(&intersection) {
							let replacement_selected_layers = vec![intersection.clone()];

							responses.push_back(DocumentMessage::SetSelectedLayers { replacement_selected_layers }.into());
						}

						let layer = document.graphene_document.layer(&intersection).unwrap();
						if let Fill::LinearGradient(gradient) = layer.style().unwrap().fill() {}

						let gradient = Gradient::new(DVec2::ZERO, tool_data.secondary_color, DVec2::ONE, tool_data.primary_color, generate_uuid());
						let mut selected_gradient = SelectedGradient::new(gradient, &intersection, document).with_start(input.mouse.position);
						selected_gradient.update_gradient(input.mouse.position, responses);

						data.selected_gradient = Some(selected_gradient);

						GradientToolFsmState::Drawing
					} else {
						GradientToolFsmState::Ready
					}
				}
				(GradientToolFsmState::Drawing, GradientToolMessage::PointerMove { .. }) => {
					if let Some(selected_gradient) = &mut data.selected_gradient {
						selected_gradient.update_gradient(input.mouse.position, responses);
					}
					GradientToolFsmState::Drawing
				}

				(GradientToolFsmState::Drawing, GradientToolMessage::PointerUp) => GradientToolFsmState::Ready,
				_ => self,
			}
		} else {
			self
		}
	}

	fn update_hints(&self, _responses: &mut VecDeque<Message>) {}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default }.into());
	}
}

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
use graphene::layers::layer_info::Layer;
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

fn from_gradient_space(path: &[LayerId], layer: &Layer, document: &DocumentMessageHandler) -> DAffine2 {
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

		let size = DVec2::splat(VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE);

		let operation = Operation::AddOverlayEllipse {
			path: path.clone(),
			transform: DAffine2::from_scale_angle_translation(size, 0., translation - size / 2.).to_cols_array(),
			style: PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), Fill::flat(Color::WHITE)),
		};
		responses.push_back(DocumentMessage::Overlays(operation.into()).into());

		path
	}
	fn generate_line(start: DVec2, end: DVec2, responses: &mut VecDeque<Message>) -> Vec<LayerId> {
		let path = vec![generate_uuid()];

		let line_vector = end - start;
		let scale = DVec2::splat(line_vector.length());
		let angle = -line_vector.angle_between(DVec2::X);
		let translation = start;
		let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();

		let operation = Operation::AddOverlayLine {
			path: path.clone(),
			transform,
			style: PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), Fill::None),
		};
		responses.push_back(DocumentMessage::Overlays(operation.into()).into());

		path
	}

	pub fn new(fill: &Gradient, path: &[LayerId], layer: &Layer, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) -> Self {
		let transform = from_gradient_space(path, layer, document);
		let Gradient { start, end, .. } = fill;
		let [start, end] = [transform.transform_point2(*start), transform.transform_point2(*end)];

		let line = Self::generate_line(start, end, responses);
		let handles = [Self::generate_handle(start, responses), Self::generate_handle(end, responses)];

		Self { handles, line }
	}
	pub fn delete(self, responses: &mut VecDeque<Message>) {
		responses.push_back(DocumentMessage::Overlays(Operation::DeleteLayer { path: self.line }.into()).into());
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
	pub fn new(gradient: Gradient, path: &[LayerId], layer: &Layer, document: &DocumentMessageHandler) -> Self {
		let transform = from_gradient_space(path, layer, document);
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
					while let Some(overlay) = data.gradient_overlays.pop() {
						overlay.delete(responses);
					}

					for path in document.selected_visible_layers() {
						let layer = document.graphene_document.layer(path).unwrap();

						if let Ok(Fill::LinearGradient(gradient)) = layer.style().map(|style| style.fill()) {
							data.gradient_overlays.push(GradientOverlay::new(gradient, path, layer, document, responses))
						}
					}

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

						let gradient = Gradient::new(DVec2::ZERO, tool_data.secondary_color, DVec2::ONE, tool_data.primary_color, generate_uuid());
						let mut selected_gradient = SelectedGradient::new(gradient, &intersection, layer, document).with_start(input.mouse.position);
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

				(_, GradientToolMessage::Abort) => {
					while let Some(overlay) = data.gradient_overlays.pop() {
						overlay.delete(responses);
					}
					GradientToolFsmState::Ready
				}
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

use crate::consts::{COLOR_ACCENT, LINE_ROTATE_SNAP_ANGLE, SELECTION_TOLERANCE, VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE};
use crate::document::DocumentMessageHandler;
use crate::frontend::utility_types::MouseCursorIcon;
use crate::input::keyboard::Key;
use crate::input::InputPreprocessorMessageHandler;
use crate::layout::widgets::PropertyHolder;
use crate::message_prelude::*;
use crate::viewport_tools::snapping::SnapHandler;
use crate::viewport_tools::tool::{DocumentToolData, Fsm, ToolActionHandlerData};

use graphene::color::Color;
use graphene::intersection::Quad;
use graphene::layers::layer_info::{Layer, LayerDataType};
use graphene::layers::style::{Fill, Gradient, PathStyle, Stroke};
use graphene::Operation;

use glam::{DAffine2, DVec2};
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
}

impl Default for GradientToolFsmState {
	fn default() -> Self {
		GradientToolFsmState::Ready
	}
}

/// Computes the transform from gradient space to layer space (where gradient space is 0..1 in layer space)
fn gradient_space_transform(path: &[LayerId], layer: &Layer, document: &DocumentMessageHandler) -> DAffine2 {
	let bounds = layer.current_bounding_box().unwrap();
	let bound_transform = DAffine2::from_scale_angle_translation(bounds[1] - bounds[0], 0., bounds[0]);

	document.graphene_document.multiply_transforms(&path[..path.len() - 1]).unwrap() * bound_transform
}

/// Contains info on the overlays for a single gradient
#[derive(Clone, Debug, Default)]
pub struct GradientOverlay {
	pub handles: [Vec<LayerId>; 2],
	pub line: Vec<LayerId>,
	path: Vec<LayerId>,
	transform: DAffine2,
	gradient: Gradient,
}

impl GradientOverlay {
	fn generate_overlay_handle(translation: DVec2, responses: &mut VecDeque<Message>, selected: bool) -> Vec<LayerId> {
		let path = vec![generate_uuid()];

		let size = DVec2::splat(VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE);

		let fill = if selected { Fill::flat(COLOR_ACCENT) } else { Fill::flat(Color::WHITE) };

		let operation = Operation::AddOverlayEllipse {
			path: path.clone(),
			transform: DAffine2::from_scale_angle_translation(size, 0., translation - size / 2.).to_cols_array(),
			style: PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), fill),
		};
		responses.push_back(DocumentMessage::Overlays(operation.into()).into());

		path
	}
	fn generate_overlay_line(start: DVec2, end: DVec2, responses: &mut VecDeque<Message>) -> Vec<LayerId> {
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

	pub fn new(fill: &Gradient, dragging_start: Option<bool>, path: &[LayerId], layer: &Layer, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) -> Self {
		let transform = gradient_space_transform(path, layer, document);
		let Gradient { start, end, .. } = fill;
		let [start, end] = [transform.transform_point2(*start), transform.transform_point2(*end)];

		let line = Self::generate_overlay_line(start, end, responses);
		let handles = [
			Self::generate_overlay_handle(start, responses, dragging_start == Some(true)),
			Self::generate_overlay_handle(end, responses, dragging_start == Some(false)),
		];

		let path = path.to_vec();
		let gradient = fill.clone();

		Self {
			handles,
			line,
			path,
			transform,
			gradient,
		}
	}

	pub fn delete_overlays(self, responses: &mut VecDeque<Message>) {
		responses.push_back(DocumentMessage::Overlays(Operation::DeleteLayer { path: self.line }.into()).into());
		let [start, end] = self.handles;
		responses.push_back(DocumentMessage::Overlays(Operation::DeleteLayer { path: start }.into()).into());
		responses.push_back(DocumentMessage::Overlays(Operation::DeleteLayer { path: end }.into()).into());
	}

	pub fn evaluate_gradient_start(&self) -> DVec2 {
		self.transform.transform_point2(self.gradient.start)
	}

	pub fn evaluate_gradient_end(&self) -> DVec2 {
		self.transform.transform_point2(self.gradient.end)
	}
}

/// Contains information about the selected gradient handle
#[derive(Clone, Debug, Default)]
struct SelectedGradient {
	path: Vec<LayerId>,
	transform: DAffine2,
	gradient: Gradient,
	dragging_start: bool,
}

impl SelectedGradient {
	pub fn new(gradient: Gradient, path: &[LayerId], layer: &Layer, document: &DocumentMessageHandler) -> Self {
		let transform = gradient_space_transform(path, layer, document);
		Self {
			path: path.to_vec(),
			transform,
			gradient,
			dragging_start: false,
		}
	}

	pub fn with_gradient_start(mut self, start: DVec2) -> Self {
		self.gradient.start = self.transform.inverse().transform_point2(start);
		self
	}

	pub fn update_gradient(&mut self, mut mouse: DVec2, responses: &mut VecDeque<Message>, snap_rotate: bool) {
		if snap_rotate {
			let point = if self.dragging_start {
				self.transform.transform_point2(self.gradient.end)
			} else {
				self.transform.transform_point2(self.gradient.start)
			};

			let delta = point - mouse;

			let length = delta.length();
			let mut angle = -delta.angle_between(DVec2::X);

			let snap_resolution = LINE_ROTATE_SNAP_ANGLE.to_radians();
			angle = (angle / snap_resolution).round() * snap_resolution;

			let rotated = DVec2::new(length * angle.cos(), length * angle.sin());
			mouse = point - rotated;
		}

		mouse = self.transform.inverse().transform_point2(mouse);

		if self.dragging_start {
			self.gradient.start = mouse;
		} else {
			self.gradient.end = mouse;
		}

		self.gradient.transform = self.transform.inverse();
		let fill = Fill::LinearGradient(self.gradient.clone());
		let path = self.path.clone();
		responses.push_back(Operation::SetLayerFill { path, fill }.into());
	}
}

#[derive(Clone, Debug, Default)]
struct GradientToolData {
	gradient_overlays: Vec<GradientOverlay>,
	selected_gradient: Option<SelectedGradient>,
	snap_handler: SnapHandler,
}

pub fn start_snap(snap_handler: &mut SnapHandler, document: &DocumentMessageHandler, layer: &Layer, path: &[LayerId]) {
	snap_handler.start_snap(document, document.bounding_boxes(None, None), true, true);
	if let LayerDataType::Shape(s) = &layer.data {
		let transform = document.graphene_document.multiply_transforms(path).unwrap();
		let snap_points = s
			.path
			.iter()
			.filter_map(|shape| match shape {
				kurbo::PathEl::MoveTo(point) => Some(point),
				kurbo::PathEl::LineTo(point) => Some(point),
				kurbo::PathEl::QuadTo(_, point) => Some(point),
				kurbo::PathEl::CurveTo(_, _, point) => Some(point),
				kurbo::PathEl::ClosePath => None,
			})
			.map(|point| DVec2::new(point.x, point.y))
			.map(|pos| transform.transform_point2(pos))
			.collect();
		snap_handler.add_snap_points(document, snap_points);
	}
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
						overlay.delete_overlays(responses);
					}

					for path in document.selected_visible_layers() {
						let layer = document.graphene_document.layer(path).unwrap();

						if let Ok(Fill::LinearGradient(gradient)) = layer.style().map(|style| style.fill()) {
							let dragging_start = data
								.selected_gradient
								.as_ref()
								.map_or(None, |selected| if selected.path == path { Some(selected.dragging_start) } else { None });
							data.gradient_overlays.push(GradientOverlay::new(gradient, dragging_start, path, layer, document, responses))
						}
					}

					self
				}
				(GradientToolFsmState::Ready, GradientToolMessage::PointerDown) => {
					responses.push_back(ToolMessage::DocumentIsDirty.into());

					let mouse = input.mouse.position;
					let tolerance = VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE.powi(2);

					let mut dragging = false;
					for overlay in &data.gradient_overlays {
						if overlay.evaluate_gradient_start().distance_squared(mouse) < tolerance {
							dragging = true;
							start_snap(&mut data.snap_handler, document, document.graphene_document.layer(&overlay.path).unwrap(), &overlay.path);
							data.selected_gradient = Some(SelectedGradient {
								path: overlay.path.clone(),
								transform: overlay.transform.clone(),
								gradient: overlay.gradient.clone(),
								dragging_start: true,
							})
						}
						if overlay.evaluate_gradient_end().distance_squared(mouse) < tolerance {
							dragging = true;
							start_snap(&mut data.snap_handler, document, document.graphene_document.layer(&overlay.path).unwrap(), &overlay.path);
							data.selected_gradient = Some(SelectedGradient {
								path: overlay.path.clone(),
								transform: overlay.transform.clone(),
								gradient: overlay.gradient.clone(),
								dragging_start: false,
							})
						}
					}
					if dragging {
						GradientToolFsmState::Drawing
					} else {
						let tolerance = DVec2::splat(SELECTION_TOLERANCE);
						let quad = Quad::from_box([input.mouse.position - tolerance, input.mouse.position + tolerance]);
						let intersection = document.graphene_document.intersects_quad_root(quad).pop();

						if let Some(intersection) = intersection {
							if !document.selected_layers_contains(&intersection) {
								let replacement_selected_layers = vec![intersection.clone()];

								responses.push_back(DocumentMessage::SetSelectedLayers { replacement_selected_layers }.into());
							}

							let layer = document.graphene_document.layer(&intersection).unwrap();

							let gradient = Gradient::new(DVec2::ZERO, tool_data.secondary_color, DVec2::ONE, tool_data.primary_color, DAffine2::IDENTITY, generate_uuid());
							let mut selected_gradient = SelectedGradient::new(gradient, &intersection, layer, document).with_gradient_start(input.mouse.position);
							selected_gradient.update_gradient(input.mouse.position, responses, false);

							data.selected_gradient = Some(selected_gradient);

							start_snap(&mut data.snap_handler, document, layer, &intersection);

							GradientToolFsmState::Drawing
						} else {
							GradientToolFsmState::Ready
						}
					}
				}
				(GradientToolFsmState::Drawing, GradientToolMessage::PointerMove { constrain_axis }) => {
					if let Some(selected_gradient) = &mut data.selected_gradient {
						let mouse = data.snap_handler.snap_position(responses, input.viewport_bounds.size(), document, input.mouse.position);
						selected_gradient.update_gradient(mouse, responses, input.keyboard.get(constrain_axis as usize));
					}
					GradientToolFsmState::Drawing
				}

				(GradientToolFsmState::Drawing, GradientToolMessage::PointerUp) => {
					data.snap_handler.cleanup(responses);

					GradientToolFsmState::Ready
				}

				(_, GradientToolMessage::Abort) => {
					data.snap_handler.cleanup(responses);

					while let Some(overlay) = data.gradient_overlays.pop() {
						overlay.delete_overlays(responses);
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

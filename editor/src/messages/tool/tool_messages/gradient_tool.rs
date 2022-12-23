use crate::application::generate_uuid;
use crate::consts::{COLOR_ACCENT, LINE_ROTATE_SNAP_ANGLE, MANIPULATOR_GROUP_MARKER_SIZE, SELECTION_TOLERANCE};
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::input_mapper::utility_types::input_keyboard::{Key, KeysGroup, MouseMotion};
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, PropertyHolder, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::messages::layout::utility_types::widgets::input_widgets::{RadioEntryData, RadioInput};
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::snapping::SnapManager;
use crate::messages::tool::utility_types::{EventToMessageMap, Fsm, ToolActionHandlerData, ToolMetadata, ToolTransition, ToolType};
use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

use graphene::color::Color;
use graphene::intersection::Quad;
use graphene::layers::layer_info::Layer;
use graphene::layers::style::{Fill, Gradient, GradientType, PathStyle, Stroke};
use graphene::LayerId;
use graphene::Operation;

use glam::{DAffine2, DVec2};
use graphene::layers::text_layer::FontCache;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct GradientTool {
	fsm_state: GradientToolFsmState,
	data: GradientToolData,
	options: GradientOptions,
}

pub struct GradientOptions {
	gradient_type: GradientType,
}

impl Default for GradientOptions {
	fn default() -> Self {
		Self { gradient_type: GradientType::Linear }
	}
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Gradient)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize)]
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
	UpdateOptions(GradientOptionsUpdate),
}

#[remain::sorted]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum GradientOptionsUpdate {
	Type(GradientType),
}

impl ToolMetadata for GradientTool {
	fn icon_name(&self) -> String {
		"GeneralGradientTool".into()
	}
	fn tooltip(&self) -> String {
		"Gradient Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Gradient
	}
}

impl<'a> MessageHandler<ToolMessage, ToolActionHandlerData<'a>> for GradientTool {
	fn process_message(&mut self, message: ToolMessage, data: ToolActionHandlerData<'a>, responses: &mut VecDeque<Message>) {
		if message == ToolMessage::UpdateHints {
			self.fsm_state.update_hints(responses);
			return;
		}

		if message == ToolMessage::UpdateCursor {
			self.fsm_state.update_cursor(responses);
			return;
		}
		if let ToolMessage::Gradient(GradientToolMessage::UpdateOptions(action)) = message {
			match action {
				GradientOptionsUpdate::Type(gradient_type) => self.options.gradient_type = gradient_type,
			}
			return;
		}

		let new_state = self.fsm_state.transition(message, &mut self.data, data, &self.options, responses);

		if self.fsm_state != new_state {
			self.fsm_state = new_state;
			self.fsm_state.update_hints(responses);
		}
	}

	advertise_actions!(GradientToolMessageDiscriminant;
		PointerDown,
		PointerUp,
		PointerMove,
		Abort,
	);
}

impl PropertyHolder for GradientTool {
	fn properties(&self) -> Layout {
		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row {
			widgets: vec![WidgetHolder::new(Widget::RadioInput(RadioInput {
				selected_index: if self.options.gradient_type == GradientType::Radial { 1 } else { 0 },
				entries: vec![
					RadioEntryData {
						value: "linear".into(),
						label: "Linear".into(),
						tooltip: "Linear Gradient".into(),
						on_update: widget_callback!(|_| GradientToolMessage::UpdateOptions(GradientOptionsUpdate::Type(GradientType::Linear)).into()),
						..RadioEntryData::default()
					},
					RadioEntryData {
						value: "radial".into(),
						label: "Radial".into(),
						tooltip: "Radial Gradient".into(),
						on_update: widget_callback!(|_| GradientToolMessage::UpdateOptions(GradientOptionsUpdate::Type(GradientType::Radial)).into()),
						..RadioEntryData::default()
					},
				],
				..Default::default()
			}))],
		}]))
	}
}

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
fn gradient_space_transform(path: &[LayerId], layer: &Layer, document: &DocumentMessageHandler, font_cache: &FontCache) -> DAffine2 {
	let bounds = layer.aabb_for_transform(DAffine2::IDENTITY, font_cache).unwrap();
	let bound_transform = DAffine2::from_scale_angle_translation(bounds[1] - bounds[0], 0., bounds[0]);

	let multiplied = document.graphene_document.multiply_transforms(path).unwrap();

	multiplied * bound_transform
}

/// Contains info on the overlays for a single gradient
#[derive(Clone, Debug, Default)]
pub struct GradientOverlay {
	pub handles: [Vec<LayerId>; 2],
	pub line: Vec<LayerId>,
	pub steps: Vec<Vec<LayerId>>,
	path: Vec<LayerId>,
	transform: DAffine2,
	gradient: Gradient,
}

impl GradientOverlay {
	fn generate_overlay_handle(translation: DVec2, responses: &mut VecDeque<Message>, selected: bool) -> Vec<LayerId> {
		let path = vec![generate_uuid()];

		let size = DVec2::splat(MANIPULATOR_GROUP_MARKER_SIZE);

		let fill = if selected { Fill::solid(COLOR_ACCENT) } else { Fill::solid(Color::WHITE) };

		let operation = Operation::AddEllipse {
			path: path.clone(),
			transform: DAffine2::from_scale_angle_translation(size, 0., translation - size / 2.).to_cols_array(),
			style: PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), fill),
			insert_index: -1,
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

		let operation = Operation::AddLine {
			path: path.clone(),
			transform,
			style: PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), Fill::None),
			insert_index: -1,
		};
		responses.push_back(DocumentMessage::Overlays(operation.into()).into());

		path
	}

	pub fn new(
		fill: &Gradient,
		dragging: Option<GradientDragTarget>,
		path: &[LayerId],
		layer: &Layer,
		document: &DocumentMessageHandler,
		responses: &mut VecDeque<Message>,
		font_cache: &FontCache,
	) -> Self {
		let transform = gradient_space_transform(path, layer, document, font_cache);
		let Gradient { start, end, positions, .. } = fill;
		let [start, end] = [transform.transform_point2(*start), transform.transform_point2(*end)];

		let line = Self::generate_overlay_line(start, end, responses);
		let handles = [
			Self::generate_overlay_handle(start, responses, dragging == Some(GradientDragTarget::Start)),
			Self::generate_overlay_handle(end, responses, dragging == Some(GradientDragTarget::End)),
		];

		let not_at_end = |(_, x): &(_, f64)| x.abs() > f64::EPSILON * 1000. && (1. - x).abs() > f64::EPSILON * 1000.;
		let create_step = |(index, pos)| Self::generate_overlay_handle(start.lerp(end, pos), responses, dragging == Some(GradientDragTarget::Step(index)));
		let steps = positions.iter().map(|(pos, _)| *pos).enumerate().filter(not_at_end).map(create_step).collect();

		let path = path.to_vec();
		let gradient = fill.clone();

		Self {
			handles,
			steps,
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
		for step in self.steps {
			responses.push_back(DocumentMessage::Overlays(Operation::DeleteLayer { path: step }.into()).into());
		}
	}

	pub fn evaluate_gradient_start(&self) -> DVec2 {
		self.transform.transform_point2(self.gradient.start)
	}

	pub fn evaluate_gradient_end(&self) -> DVec2 {
		self.transform.transform_point2(self.gradient.end)
	}
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, Default)]
pub enum GradientDragTarget {
	Start,
	#[default]
	End,
	Step(usize),
}

/// Contains information about the selected gradient handle
#[derive(Clone, Debug, Default)]
struct SelectedGradient {
	path: Vec<LayerId>,
	transform: DAffine2,
	gradient: Gradient,
	dragging: GradientDragTarget,
}

impl SelectedGradient {
	pub fn new(gradient: Gradient, path: &[LayerId], layer: &Layer, document: &DocumentMessageHandler, font_cache: &FontCache) -> Self {
		let transform = gradient_space_transform(path, layer, document, font_cache);
		Self {
			path: path.to_vec(),
			transform,
			gradient,
			dragging: GradientDragTarget::End,
		}
	}

	pub fn with_gradient_start(mut self, start: DVec2) -> Self {
		self.gradient.start = self.transform.inverse().transform_point2(start);
		self
	}

	pub fn update_gradient(&mut self, mut mouse: DVec2, responses: &mut VecDeque<Message>, snap_rotate: bool, gradient_type: GradientType) {
		self.gradient.gradient_type = gradient_type;

		if snap_rotate && matches!(self.dragging, GradientDragTarget::End | GradientDragTarget::Start) {
			let point = if self.dragging == GradientDragTarget::Start {
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

		match self.dragging {
			GradientDragTarget::Start => self.gradient.start = mouse,
			GradientDragTarget::End => self.gradient.end = mouse,
			GradientDragTarget::Step(s) => {
				// Calculate the new position by finding the closest point on the line
				let new_pos = ((self.gradient.end - self.gradient.start).angle_between(mouse - self.gradient.start)).cos() * self.gradient.start.distance(mouse)
					/ self.gradient.start.distance(self.gradient.end);

				// Should not go off end but can swap (like inscape)
				let clamped = new_pos.clamp(0., 1.);
				self.gradient.positions[s].0 = clamped;
				let new_pos = self.gradient.positions[s];

				self.gradient.positions.sort_unstable_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
				self.dragging = GradientDragTarget::Step(self.gradient.positions.iter().position(|x| *x == new_pos).unwrap());
			}
		}

		self.gradient.transform = self.transform;
		let fill = Fill::Gradient(self.gradient.clone());
		let path = self.path.clone();
		responses.push_back(Operation::SetLayerFill { path, fill }.into());
	}
}

impl ToolTransition for GradientTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			document_dirty: Some(GradientToolMessage::DocumentIsDirty.into()),
			tool_abort: Some(GradientToolMessage::Abort.into()),
			selection_changed: None,
		}
	}
}

#[derive(Clone, Debug, Default)]
struct GradientToolData {
	gradient_overlays: Vec<GradientOverlay>,
	selected_gradient: Option<SelectedGradient>,
	snap_manager: SnapManager,
}

pub fn start_snap(snap_manager: &mut SnapManager, document: &DocumentMessageHandler, font_cache: &FontCache) {
	snap_manager.start_snap(document, document.bounding_boxes(None, None, font_cache), true, true);
	snap_manager.add_all_document_handles(document, &[], &[], &[]);
}

impl Fsm for GradientToolFsmState {
	type ToolData = GradientToolData;
	type ToolOptions = GradientOptions;

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		(document, _document_id, global_tool_data, input, font_cache): ToolActionHandlerData,
		tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		if let ToolMessage::Gradient(event) = event {
			match (self, event) {
				(_, GradientToolMessage::DocumentIsDirty) => {
					while let Some(overlay) = tool_data.gradient_overlays.pop() {
						overlay.delete_overlays(responses);
					}

					for path in document.selected_visible_layers() {
						if !document.graphene_document.multiply_transforms(path).unwrap().inverse().is_finite() {
							continue;
						}
						let layer = document.graphene_document.layer(path).unwrap();

						if let Ok(Fill::Gradient(gradient)) = layer.style().map(|style| style.fill()) {
							let dragging = tool_data
								.selected_gradient
								.as_ref()
								.and_then(|selected| if selected.path == path { Some(selected.dragging) } else { None });
							tool_data.gradient_overlays.push(GradientOverlay::new(gradient, dragging, path, layer, document, responses, font_cache))
						}
					}

					self
				}
				(GradientToolFsmState::Ready, GradientToolMessage::PointerDown) => {
					responses.push_back(BroadcastEvent::DocumentIsDirty.into());

					let mouse = input.mouse.position;
					let tolerance = MANIPULATOR_GROUP_MARKER_SIZE.powi(2);

					let mut dragging = false;
					for overlay in &tool_data.gradient_overlays {
						// Check for dragging step
						for (index, (pos, _)) in overlay.gradient.positions.iter().enumerate() {
							let pos = overlay.transform.transform_point2(overlay.gradient.start.lerp(overlay.gradient.end, *pos));
							if pos.distance_squared(mouse) < tolerance {
								dragging = true;
								tool_data.selected_gradient = Some(SelectedGradient {
									path: overlay.path.clone(),
									transform: overlay.transform,
									gradient: overlay.gradient.clone(),
									dragging: GradientDragTarget::Step(index),
								})
							}
						}

						// Check dragging start or end handle
						for (pos, dragging_target) in [
							(overlay.evaluate_gradient_start(), GradientDragTarget::Start),
							(overlay.evaluate_gradient_end(), GradientDragTarget::End),
						] {
							if pos.distance_squared(mouse) < tolerance {
								dragging = true;
								start_snap(&mut tool_data.snap_manager, document, font_cache);
								tool_data.selected_gradient = Some(SelectedGradient {
									path: overlay.path.clone(),
									transform: overlay.transform,
									gradient: overlay.gradient.clone(),
									dragging: dragging_target,
								})
							}
						}
					}
					if dragging {
						GradientToolFsmState::Drawing
					} else {
						let tolerance = DVec2::splat(SELECTION_TOLERANCE);
						let quad = Quad::from_box([input.mouse.position - tolerance, input.mouse.position + tolerance]);
						let intersection = document.graphene_document.intersects_quad_root(quad, font_cache).pop();

						if let Some(intersection) = intersection {
							if !document.selected_layers_contains(&intersection) {
								let replacement_selected_layers = vec![intersection.clone()];

								responses.push_back(DocumentMessage::SetSelectedLayers { replacement_selected_layers }.into());
							}

							let layer = document.graphene_document.layer(&intersection).unwrap();

							let gradient = Gradient::new(
								DVec2::ZERO,
								global_tool_data.secondary_color,
								DVec2::ONE,
								global_tool_data.primary_color,
								DAffine2::IDENTITY,
								generate_uuid(),
								tool_options.gradient_type,
							);
							let mut selected_gradient = SelectedGradient::new(gradient, &intersection, layer, document, font_cache).with_gradient_start(input.mouse.position);
							selected_gradient.update_gradient(input.mouse.position, responses, false, tool_options.gradient_type);

							tool_data.selected_gradient = Some(selected_gradient);

							start_snap(&mut tool_data.snap_manager, document, font_cache);

							GradientToolFsmState::Drawing
						} else {
							GradientToolFsmState::Ready
						}
					}
				}
				(GradientToolFsmState::Drawing, GradientToolMessage::PointerMove { constrain_axis }) => {
					if let Some(selected_gradient) = &mut tool_data.selected_gradient {
						let mouse = tool_data.snap_manager.snap_position(responses, document, input.mouse.position);
						selected_gradient.update_gradient(mouse, responses, input.keyboard.get(constrain_axis as usize), selected_gradient.gradient.gradient_type);
					}
					GradientToolFsmState::Drawing
				}

				(GradientToolFsmState::Drawing, GradientToolMessage::PointerUp) => {
					tool_data.snap_manager.cleanup(responses);

					GradientToolFsmState::Ready
				}

				(_, GradientToolMessage::Abort) => {
					tool_data.snap_manager.cleanup(responses);

					while let Some(overlay) = tool_data.gradient_overlays.pop() {
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

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			GradientToolFsmState::Ready => HintData(vec![HintGroup(vec![
				HintInfo {
					key_groups: vec![],
					key_groups_mac: None,
					mouse: Some(MouseMotion::LmbDrag),
					label: String::from("Draw Gradient"),
					plus: false,
				},
				HintInfo {
					key_groups: vec![KeysGroup(vec![Key::Shift]).into()],
					key_groups_mac: None,
					mouse: None,
					label: String::from("Snap 15°"),
					plus: true,
				},
			])]),
			GradientToolFsmState::Drawing => HintData(vec![HintGroup(vec![HintInfo {
				key_groups: vec![KeysGroup(vec![Key::Shift]).into()],
				key_groups_mac: None,
				mouse: None,
				label: String::from("Snap 15°"),
				plus: false,
			}])]),
		};

		responses.push_back(FrontendMessage::UpdateInputHints { hint_data }.into());
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.push_back(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default }.into());
	}
}

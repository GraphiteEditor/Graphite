use super::tool_prelude::*;
use crate::application::generate_uuid;
use crate::consts::{COLOR_ACCENT, LINE_ROTATE_SNAP_ANGLE, MANIPULATOR_GROUP_MARKER_SIZE, SELECTION_THRESHOLD};
use crate::messages::tool::common_functionality::graph_modification_utils::get_gradient;
use crate::messages::tool::common_functionality::snapping::SnapManager;

use document_legacy::document_metadata::LayerNodeIdentifier;
use document_legacy::layers::style::{Fill, Gradient, GradientType, PathStyle, RenderData, Stroke};
use document_legacy::LayerId;
use document_legacy::Operation;
use graphene_core::raster::color::Color;

#[derive(Default)]
pub struct GradientTool {
	fsm_state: GradientToolFsmState,
	data: GradientToolData,
	options: GradientOptions,
}

#[derive(Default)]
pub struct GradientOptions {
	gradient_type: GradientType,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Gradient)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize, specta::Type)]
pub enum GradientToolMessage {
	// Standard messages
	#[remain::unsorted]
	Abort,
	#[remain::unsorted]
	DocumentIsDirty,

	// Tool-specific messages
	DeleteStop,
	InsertStop,
	PointerDown,
	PointerMove {
		constrain_axis: Key,
	},
	PointerUp,
	UpdateOptions(GradientOptionsUpdate),
}

#[remain::sorted]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize, specta::Type)]
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

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for GradientTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		let ToolMessage::Gradient(GradientToolMessage::UpdateOptions(action)) = message else {
			self.fsm_state.process_event(message, &mut self.data, tool_data, &self.options, responses, false);
			return;
		};
		match action {
			GradientOptionsUpdate::Type(gradient_type) => {
				self.options.gradient_type = gradient_type;
				if let Some(selected_gradient) = &mut self.data.selected_gradient {
					selected_gradient.gradient.gradient_type = gradient_type;
					selected_gradient.render_gradient(responses);
				}
			}
		}
	}

	advertise_actions!(GradientToolMessageDiscriminant;
		PointerDown,
		PointerUp,
		PointerMove,
		Abort,
		InsertStop,
		DeleteStop,
	);
}

impl LayoutHolder for GradientTool {
	fn layout(&self) -> Layout {
		let gradient_type = RadioInput::new(vec![
			RadioEntryData::new("Linear")
				.value("linear")
				.tooltip("Linear Gradient")
				.on_update(move |_| GradientToolMessage::UpdateOptions(GradientOptionsUpdate::Type(GradientType::Linear)).into()),
			RadioEntryData::new("Radial")
				.value("radial")
				.tooltip("Radial Gradient")
				.on_update(move |_| GradientToolMessage::UpdateOptions(GradientOptionsUpdate::Type(GradientType::Radial)).into()),
		])
		.selected_index(Some((self.selected_gradient().unwrap_or(self.options.gradient_type) == GradientType::Radial) as u32))
		.widget_holder();

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets: vec![gradient_type] }]))
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum GradientToolFsmState {
	#[default]
	Ready,
	Drawing,
}

/// Computes the transform from gradient space to viewport space (where gradient space is 0..1)
fn gradient_space_transform(layer: LayerNodeIdentifier, document: &DocumentMessageHandler) -> DAffine2 {
	let bounds = document.metadata().nonzero_bounding_box(layer);
	let bound_transform = DAffine2::from_scale_angle_translation(bounds[1] - bounds[0], 0., bounds[0]);

	let multiplied = document.metadata().transform_to_viewport(layer);

	multiplied * bound_transform
}

/// Contains info on the overlays for a single gradient
#[derive(Clone, Debug, Default)]
pub struct GradientOverlay {
	pub handles: [Vec<LayerId>; 2],
	pub line: Vec<LayerId>,
	pub steps: Vec<Vec<LayerId>>,
	layer: LayerNodeIdentifier,
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
			style: PathStyle::new(Some(Stroke::new(Some(COLOR_ACCENT), 1.0)), fill),
			insert_index: -1,
		};
		responses.add(DocumentMessage::Overlays(operation.into()));

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
			style: PathStyle::new(Some(Stroke::new(Some(COLOR_ACCENT), 1.0)), Fill::None),
			insert_index: -1,
		};
		responses.add(DocumentMessage::Overlays(operation.into()));

		path
	}

	pub fn new(gradient: Gradient, dragging: Option<GradientDragTarget>, layer: LayerNodeIdentifier, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) -> Self {
		let transform = gradient_space_transform(layer, document);
		let Gradient { start, end, positions, .. } = &gradient;
		let [start, end] = [transform.transform_point2(*start), transform.transform_point2(*end)];

		let line = Self::generate_overlay_line(start, end, responses);
		let handles = [
			Self::generate_overlay_handle(start, responses, dragging == Some(GradientDragTarget::Start)),
			Self::generate_overlay_handle(end, responses, dragging == Some(GradientDragTarget::End)),
		];

		let not_at_end = |(_, x): &(_, f64)| x.abs() > f64::EPSILON * 1000. && (1. - x).abs() > f64::EPSILON * 1000.;
		let create_step = |(index, pos)| Self::generate_overlay_handle(start.lerp(end, pos), responses, dragging == Some(GradientDragTarget::Step(index)));
		let steps = positions.iter().map(|(pos, _)| *pos).enumerate().filter(not_at_end).map(create_step).collect();

		Self {
			handles,
			steps,
			line,
			layer,
			transform,
			gradient,
		}
	}

	pub fn delete_overlays(self, responses: &mut VecDeque<Message>) {
		responses.add(DocumentMessage::Overlays(Operation::DeleteLayer { path: self.line }.into()));
		let [start, end] = self.handles;
		responses.add(DocumentMessage::Overlays(Operation::DeleteLayer { path: start }.into()));
		responses.add(DocumentMessage::Overlays(Operation::DeleteLayer { path: end }.into()));
		for step in self.steps {
			responses.add(DocumentMessage::Overlays(Operation::DeleteLayer { path: step }.into()));
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
	layer: LayerNodeIdentifier,
	transform: DAffine2,
	gradient: Gradient,
	dragging: GradientDragTarget,
}

impl SelectedGradient {
	pub fn new(gradient: Gradient, layer: LayerNodeIdentifier, document: &DocumentMessageHandler) -> Self {
		let transform = gradient_space_transform(layer, document);
		Self {
			layer,
			transform,
			gradient,
			dragging: GradientDragTarget::End,
		}
	}

	/// Update the selected gradient, checking for removal or change of gradient.
	pub fn update(gradient: &mut Option<Self>, document: &DocumentMessageHandler, responses: &mut VecDeque<Message>) {
		let Some(inner_gradient) = gradient else {
			return;
		};

		// Clear the gradient if layer deleted
		if !inner_gradient.layer.exists(document.metadata()) {
			responses.add(ToolMessage::RefreshToolOptions);
			*gradient = None;
			return;
		};

		// Update transform
		inner_gradient.transform = gradient_space_transform(inner_gradient.layer, document);

		// Clear if no longer a gradient
		let Some(gradient) = get_gradient(inner_gradient.layer, &document.document_legacy) else {
			responses.add(ToolMessage::RefreshToolOptions);
			*gradient = None;
			return;
		};

		if gradient.gradient_type != inner_gradient.gradient.gradient_type {
			responses.add(ToolMessage::RefreshToolOptions);
		}
		inner_gradient.gradient = gradient.clone();
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

		let transformed_mouse = self.transform.inverse().transform_point2(mouse);

		match self.dragging {
			GradientDragTarget::Start => self.gradient.start = transformed_mouse,
			GradientDragTarget::End => self.gradient.end = transformed_mouse,
			GradientDragTarget::Step(s) => {
				let (start, end) = (self.transform.transform_point2(self.gradient.start), self.transform.transform_point2(self.gradient.end));

				// Calculate the new position by finding the closest point on the line
				let new_pos = ((end - start).angle_between(mouse - start)).cos() * start.distance(mouse) / start.distance(end);

				// Should not go off end but can swap
				let clamped = new_pos.clamp(0., 1.);
				self.gradient.positions[s].0 = clamped;
				let new_pos = self.gradient.positions[s];

				self.gradient.positions.sort_unstable_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
				self.dragging = GradientDragTarget::Step(self.gradient.positions.iter().position(|x| *x == new_pos).unwrap());
			}
		}
		self.render_gradient(responses);
	}

	/// Update the layer fill to the current gradient
	pub fn render_gradient(&mut self, responses: &mut VecDeque<Message>) {
		self.gradient.transform = self.transform;
		let fill = Fill::Gradient(self.gradient.clone());
		let layer = self.layer.to_path();
		responses.add(GraphOperationMessage::FillSet { layer, fill });
	}
}

impl GradientTool {
	/// Get the gradient type of the selected gradient (if it exists)
	pub fn selected_gradient(&self) -> Option<GradientType> {
		self.data.selected_gradient.as_ref().map(|selected| selected.gradient.gradient_type)
	}
}

impl ToolTransition for GradientTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			document_dirty: Some(GradientToolMessage::DocumentIsDirty.into()),
			tool_abort: Some(GradientToolMessage::Abort.into()),
			selection_changed: Some(GradientToolMessage::DocumentIsDirty.into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Debug, Default)]
struct GradientToolData {
	gradient_overlays: Vec<GradientOverlay>,
	selected_gradient: Option<SelectedGradient>,
	snap_manager: SnapManager,
	drag_start: DVec2,
}

pub fn start_snap(snap_manager: &mut SnapManager, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, render_data: &RenderData) {
	snap_manager.start_snap(document, input, document.bounding_boxes(None, None, render_data), true, true);
	snap_manager.add_all_document_handles(document, input, &[], &[], &[]);
}

impl Fsm for GradientToolFsmState {
	type ToolData = GradientToolData;
	type ToolOptions = GradientOptions;

	fn transition(self, event: ToolMessage, tool_data: &mut Self::ToolData, tool_action_data: &mut ToolActionHandlerData, tool_options: &Self::ToolOptions, responses: &mut VecDeque<Message>) -> Self {
		let ToolActionHandlerData {
			document,
			global_tool_data,
			input,
			render_data,
			..
		} = tool_action_data;

		let ToolMessage::Gradient(event) = event else {
			return self;
		};

		match (self, event) {
			(_, GradientToolMessage::DocumentIsDirty) => {
				while let Some(overlay) = tool_data.gradient_overlays.pop() {
					overlay.delete_overlays(responses);
				}

				if self != GradientToolFsmState::Drawing {
					SelectedGradient::update(&mut tool_data.selected_gradient, document, responses);
				}

				for layer in document.metadata().selected_visible_layers() {
					if let Some(gradient) = get_gradient(layer, &document.document_legacy) {
						let dragging = tool_data
							.selected_gradient
							.as_ref()
							.and_then(|selected| if selected.layer == layer { Some(selected.dragging) } else { None });
						tool_data.gradient_overlays.push(GradientOverlay::new(gradient, dragging, layer, document, responses))
					}
				}

				self
			}
			(GradientToolFsmState::Ready, GradientToolMessage::DeleteStop) => {
				let Some(selected_gradient) = &mut tool_data.selected_gradient else {
					return self;
				};

				// Skip if invalid gradient
				if selected_gradient.gradient.positions.len() < 2 {
					return self;
				}

				// Remove the selected point
				match selected_gradient.dragging {
					GradientDragTarget::Start => selected_gradient.gradient.positions.remove(0),
					GradientDragTarget::End => selected_gradient.gradient.positions.pop().unwrap(),
					GradientDragTarget::Step(index) => selected_gradient.gradient.positions.remove(index),
				};

				// The gradient has only one point and so should become a fill
				if selected_gradient.gradient.positions.len() == 1 {
					let fill = Fill::Solid(selected_gradient.gradient.positions[0].1.unwrap_or(Color::BLACK));
					let layer = selected_gradient.layer.to_path();
					responses.add(GraphOperationMessage::FillSet { layer, fill });
					return self;
				}

				// Find the minimum and maximum positions
				let min_position = selected_gradient.gradient.positions.iter().map(|(pos, _)| *pos).reduce(f64::min).expect("No min");
				let max_position = selected_gradient.gradient.positions.iter().map(|(pos, _)| *pos).reduce(f64::max).expect("No max");

				// Recompute the start and end posiiton of the gradient (in viewport transform)
				let transform = selected_gradient.transform;
				let (start, end) = (transform.transform_point2(selected_gradient.gradient.start), transform.transform_point2(selected_gradient.gradient.end));
				let (new_start, new_end) = (start.lerp(end, min_position), start.lerp(end, max_position));
				selected_gradient.gradient.start = transform.inverse().transform_point2(new_start);
				selected_gradient.gradient.end = transform.inverse().transform_point2(new_end);

				// Remap the positions
				for (position, _) in selected_gradient.gradient.positions.iter_mut() {
					*position = (*position - min_position) / (max_position - min_position);
				}

				// Render the new gradient
				selected_gradient.render_gradient(responses);

				self
			}
			(_, GradientToolMessage::InsertStop) => {
				for overlay in &tool_data.gradient_overlays {
					let mouse = input.mouse.position;
					let (start, end) = (overlay.evaluate_gradient_start(), overlay.evaluate_gradient_end());

					// Compute the distance from the mouse to the gradient line in viewport space
					let distance = (end - start).angle_between(mouse - start).sin() * (mouse - start).length();

					// If click is on the line then insert point
					if distance < SELECTION_THRESHOLD {
						let mut gradient = overlay.gradient.clone();

						// Try and insert the new stop
						if let Some(index) = gradient.insert_stop(mouse, overlay.transform) {
							document.backup_nonmut(responses);

							let mut selected_gradient = SelectedGradient::new(gradient, overlay.layer, document);

							// Select the new point
							selected_gradient.dragging = GradientDragTarget::Step(index);

							// Update the layer fill
							selected_gradient.render_gradient(responses);

							tool_data.selected_gradient = Some(selected_gradient);

							break;
						}
					}
				}

				self
			}
			(GradientToolFsmState::Ready, GradientToolMessage::PointerDown) => {
				responses.add(BroadcastEvent::DocumentIsDirty);

				let mouse = input.mouse.position;
				tool_data.drag_start = mouse;
				let tolerance = MANIPULATOR_GROUP_MARKER_SIZE.powi(2);

				let mut dragging = false;
				for overlay in &tool_data.gradient_overlays {
					// Check for dragging step
					for (index, (pos, _)) in overlay.gradient.positions.iter().enumerate() {
						let pos = overlay.transform.transform_point2(overlay.gradient.start.lerp(overlay.gradient.end, *pos));
						if pos.distance_squared(mouse) < tolerance {
							dragging = true;
							tool_data.selected_gradient = Some(SelectedGradient {
								layer: overlay.layer,
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
							start_snap(&mut tool_data.snap_manager, document, input, render_data);
							tool_data.selected_gradient = Some(SelectedGradient {
								layer: overlay.layer,
								transform: overlay.transform,
								gradient: overlay.gradient.clone(),
								dragging: dragging_target,
							})
						}
					}
				}
				if dragging {
					document.backup_nonmut(responses);
					GradientToolFsmState::Drawing
				} else {
					let selected_layer = document.metadata().click(input.mouse.position, &document.document_legacy.document_network);

					// Apply the gradient to the selected layer
					if let Some(layer) = selected_layer {
						// let is_bitmap = document
						// 	.document_legacy
						// 	.layer(&layer)
						// 	.ok()
						// 	.and_then(|layer| layer.as_layer().ok())
						// 	.map_or(false, |layer| matches!(layer.cached_output_data, CachedOutputData::BlobURL(_) | CachedOutputData::SurfaceId(_)));
						// if is_bitmap {
						// 	return self;
						// }

						if !document.metadata().selected_layers_contains(layer) {
							let replacement_selected_layers = vec![layer.to_path()];

							responses.add(DocumentMessage::SetSelectedLayers { replacement_selected_layers });
						}

						responses.add(DocumentMessage::StartTransaction);

						// Use the already existing gradient if it exists
						let gradient = if let Some(gradient) = get_gradient(layer, &document.document_legacy) {
							gradient.clone()
						} else {
							// Generate a new gradient
							Gradient::new(
								DVec2::ZERO,
								global_tool_data.secondary_color,
								DVec2::ONE,
								global_tool_data.primary_color,
								DAffine2::IDENTITY,
								generate_uuid(),
								tool_options.gradient_type,
							)
						};
						let selected_gradient = SelectedGradient::new(gradient, layer, document).with_gradient_start(input.mouse.position);

						tool_data.selected_gradient = Some(selected_gradient);

						start_snap(&mut tool_data.snap_manager, document, input, render_data);

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
				input.mouse.finish_transaction(tool_data.drag_start, responses);
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
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			GradientToolFsmState::Ready => HintData(vec![HintGroup(vec![
				HintInfo::mouse(MouseMotion::LmbDrag, "Draw Gradient"),
				HintInfo::keys([Key::Shift], "Snap 15°").prepend_plus(),
			])]),
			GradientToolFsmState::Drawing => HintData(vec![HintGroup(vec![HintInfo::keys([Key::Shift], "Snap 15°")])]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}

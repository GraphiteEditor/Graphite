use super::tool_prelude::*;
use crate::consts::{COLOR_OVERLAY_BLUE, DRAG_THRESHOLD, LINE_ROTATE_SNAP_ANGLE, MANIPULATOR_GROUP_MARKER_SIZE, SEGMENT_INSERTION_DISTANCE, SEGMENT_OVERLAY_SIZE, SELECTION_THRESHOLD};
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::graph_modification_utils::{NodeGraphLayer, get_gradient};
use crate::messages::tool::common_functionality::snapping::SnapManager;
use graphene_std::vector::style::{Fill, Gradient, GradientStops, GradientType};

#[derive(Default, ExtractField)]
pub struct GradientTool {
	fsm_state: GradientToolFsmState,
	data: GradientToolData,
	options: GradientOptions,
}

#[derive(Default)]
pub struct GradientOptions {
	gradient_type: GradientType,
}

#[impl_message(Message, ToolMessage, Gradient)]
#[derive(PartialEq, Clone, Debug, Hash, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum GradientToolMessage {
	// Standard messages
	Abort,
	Overlays { context: OverlayContext },
	SelectionChanged,

	// Tool-specific messages
	DeleteStop,
	InsertStop,
	PointerDown,
	PointerMove { constrain_axis: Key },
	PointerOutsideViewport { constrain_axis: Key },
	PointerUp,
	UpdateOptions { options: GradientOptionsUpdate },
}

#[derive(PartialEq, Eq, Clone, Debug, Hash, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum GradientOptionsUpdate {
	Type(GradientType),
}

impl ToolMetadata for GradientTool {
	fn icon_name(&self) -> String {
		"GeneralGradientTool".into()
	}
	fn tooltip_label(&self) -> String {
		"Gradient Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Gradient
	}
}

#[message_handler_data]
impl<'a> MessageHandler<ToolMessage, &mut ToolActionMessageContext<'a>> for GradientTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, context: &mut ToolActionMessageContext<'a>) {
		let ToolMessage::Gradient(GradientToolMessage::UpdateOptions { options }) = message else {
			self.fsm_state.process_event(message, &mut self.data, context, &self.options, responses, false);
			return;
		};
		match options {
			GradientOptionsUpdate::Type(gradient_type) => {
				self.options.gradient_type = gradient_type;
				let selected_layers: Vec<_> = context
					.document
					.network_interface
					.selected_nodes()
					.selected_visible_layers(&context.document.network_interface)
					.collect();

				let mut transaction_started = false;
				for layer in selected_layers {
					if NodeGraphLayer::is_raster_layer(layer, &mut context.document.network_interface) {
						continue;
					}

					if let Some(mut gradient) = get_gradient(layer, &context.document.network_interface)
						&& gradient.gradient_type != gradient_type
					{
						if !transaction_started {
							responses.add(DocumentMessage::StartTransaction);
							transaction_started = true;
						}
						gradient.gradient_type = gradient_type;
						responses.add(GraphOperationMessage::FillSet {
							layer,
							fill: Fill::Gradient(gradient),
						});
					}
				}

				if transaction_started {
					responses.add(DocumentMessage::AddTransaction);
				}
				if let Some(selected_gradient) = &mut self.data.selected_gradient
					&& let Some(layer) = selected_gradient.layer
					&& !NodeGraphLayer::is_raster_layer(layer, &mut context.document.network_interface)
				{
					selected_gradient.gradient.gradient_type = gradient_type;
				}
				responses.add(ToolMessage::UpdateHints);
				responses.add(PropertiesPanelMessage::Refresh);
				responses.add(ToolMessage::UpdateCursor);
				responses.add(ToolMessage::RefreshToolOptions);
			}
		}
	}

	advertise_actions!(GradientToolMessageDiscriminant;
		PointerDown,
		PointerUp,
		PointerMove,
		Abort,
		DeleteStop,
	);
}

impl LayoutHolder for GradientTool {
	fn layout(&self) -> Layout {
		let gradient_type = RadioInput::new(vec![
			RadioEntryData::new("Linear").label("Linear").tooltip_label("Linear Gradient").on_update(move |_| {
				GradientToolMessage::UpdateOptions {
					options: GradientOptionsUpdate::Type(GradientType::Linear),
				}
				.into()
			}),
			RadioEntryData::new("Radial").label("Radial").tooltip_label("Radial Gradient").on_update(move |_| {
				GradientToolMessage::UpdateOptions {
					options: GradientOptionsUpdate::Type(GradientType::Radial),
				}
				.into()
			}),
		])
		.selected_index(Some((self.options.gradient_type == GradientType::Radial) as u32))
		.widget_instance();

		Layout(vec![LayoutGroup::Row { widgets: vec![gradient_type] }])
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GradientToolFsmState {
	Ready { hover_insertion: bool },
	Drawing,
}

impl Default for GradientToolFsmState {
	fn default() -> Self {
		Self::Ready { hover_insertion: false }
	}
}

/// Computes the transform from gradient space to viewport space (where gradient space is 0..1)
fn gradient_space_transform(layer: LayerNodeIdentifier, document: &DocumentMessageHandler) -> DAffine2 {
	let bounds = document.metadata().nonzero_bounding_box(layer);
	let bound_transform = DAffine2::from_scale_angle_translation(bounds[1] - bounds[0], 0., bounds[0]);

	let multiplied = document.metadata().transform_to_viewport(layer);

	multiplied * bound_transform
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, Default)]
pub enum GradientDragTarget {
	Start,
	#[default]
	End,
	Step(usize),
	New,
}

/// Contains information about the selected gradient handle
#[derive(Clone, Debug, Default)]
struct SelectedGradient {
	layer: Option<LayerNodeIdentifier>,
	transform: DAffine2,
	gradient: Gradient,
	dragging: GradientDragTarget,
	initial_gradient: Gradient,
}

fn calculate_insertion(start: DVec2, end: DVec2, stops: &GradientStops, mouse: DVec2) -> Option<f64> {
	let distance = (end - start).angle_to(mouse - start).sin() * (mouse - start).length();
	let projection = ((end - start).angle_to(mouse - start)).cos() * start.distance(mouse) / start.distance(end);

	if distance.abs() < SEGMENT_INSERTION_DISTANCE && (0. ..=1.).contains(&projection) {
		for stop in stops {
			let stop_pos = start.lerp(end, stop.position);
			if stop_pos.distance_squared(mouse) < (MANIPULATOR_GROUP_MARKER_SIZE * 2.).powi(2) {
				return None;
			}
		}
		if start.distance_squared(mouse) < (MANIPULATOR_GROUP_MARKER_SIZE * 2.).powi(2) || end.distance_squared(mouse) < (MANIPULATOR_GROUP_MARKER_SIZE * 2.).powi(2) {
			return None;
		}

		return Some(projection);
	}

	None
}

impl SelectedGradient {
	pub fn new(gradient: Gradient, layer: LayerNodeIdentifier, document: &DocumentMessageHandler) -> Self {
		let transform = gradient_space_transform(layer, document);
		Self {
			layer: Some(layer),
			transform,
			gradient: gradient.clone(),
			dragging: GradientDragTarget::End,
			initial_gradient: gradient,
		}
	}

	pub fn update_gradient(&mut self, mut mouse: DVec2, responses: &mut VecDeque<Message>, snap_rotate: bool, gradient_type: GradientType, drag_start: DVec2) {
		if mouse.distance(drag_start) < DRAG_THRESHOLD {
			self.gradient = self.initial_gradient.clone();
			self.render_gradient(responses);
			return;
		}

		self.gradient.gradient_type = gradient_type;

		if snap_rotate && matches!(self.dragging, GradientDragTarget::End | GradientDragTarget::Start | GradientDragTarget::New) {
			let point = if self.dragging == GradientDragTarget::Start {
				self.transform.transform_point2(self.gradient.end)
			} else if self.dragging == GradientDragTarget::New {
				drag_start
			} else {
				self.transform.transform_point2(self.gradient.start)
			};

			let delta = point - mouse;

			let length = delta.length();
			let mut angle = -delta.angle_to(DVec2::X);

			let snap_resolution = LINE_ROTATE_SNAP_ANGLE.to_radians();
			angle = (angle / snap_resolution).round() * snap_resolution;

			let rotated = DVec2::new(length * angle.cos(), length * angle.sin());
			mouse = point - rotated;
		}

		let transformed_mouse = self.transform.inverse().transform_point2(mouse);

		match self.dragging {
			GradientDragTarget::Start => self.gradient.start = transformed_mouse,
			GradientDragTarget::End => self.gradient.end = transformed_mouse,
			GradientDragTarget::New => {
				self.gradient.start = self.transform.inverse().transform_point2(drag_start);
				self.gradient.end = transformed_mouse;
			}
			GradientDragTarget::Step(s) => {
				let (start, end) = (self.transform.transform_point2(self.gradient.start), self.transform.transform_point2(self.gradient.end));

				// Calculate the new position by finding the closest point on the line
				let new_pos = ((end - start).angle_to(mouse - start)).cos() * start.distance(mouse) / start.distance(end);

				// Should not go off end but can swap
				let clamped = new_pos.clamp(0., 1.);
				self.gradient.stops.position[s] = clamped;
				let new_position = self.gradient.stops.position[s];
				let new_color = self.gradient.stops.color[s];

				self.gradient.stops.sort();
				self.dragging = GradientDragTarget::Step(self.gradient.stops.iter().position(|s| s.position == new_position && s.color == new_color).unwrap());
			}
		}
		self.render_gradient(responses);
	}

	/// Update the layer fill to the current gradient
	pub fn render_gradient(&mut self, responses: &mut VecDeque<Message>) {
		if let Some(layer) = self.layer {
			responses.add(GraphOperationMessage::FillSet {
				layer,
				fill: Fill::Gradient(self.gradient.clone()),
			});
		}
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
			tool_abort: Some(GradientToolMessage::Abort.into()),
			selection_changed: Some(GradientToolMessage::SelectionChanged.into()),
			overlay_provider: Some(|context| GradientToolMessage::Overlays { context }.into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Debug, Default)]
struct GradientToolData {
	selected_gradient: Option<SelectedGradient>,
	snap_manager: SnapManager,
	drag_start: DVec2,
	auto_panning: AutoPanning,
}

impl Fsm for GradientToolFsmState {
	type ToolData = GradientToolData;
	type ToolOptions = GradientOptions;

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		tool_action_data: &mut ToolActionMessageContext,
		tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		let ToolActionMessageContext {
			document,
			global_tool_data,
			input,
			viewport,
			..
		} = tool_action_data;

		let ToolMessage::Gradient(event) = event else { return self };
		match (self, event) {
			(_, GradientToolMessage::Overlays { context: mut overlay_context }) => {
				let selected = tool_data.selected_gradient.as_ref();
				let mouse = input.mouse.position;

				for layer in document.network_interface.selected_nodes().selected_visible_layers(&document.network_interface) {
					let Some(gradient) = get_gradient(layer, &document.network_interface) else { continue };
					let transform = gradient_space_transform(layer, document);
					let dragging = selected
						.filter(|selected| selected.layer.is_some_and(|selected_layer| selected_layer == layer))
						.map(|selected| selected.dragging);

					let gradient = if dragging.is_some()
						&& let Some(selected_gradient) = selected.filter(|s| s.layer == Some(layer))
					{
						&selected_gradient.gradient
					} else {
						&gradient
					};

					let Gradient { start, end, stops, .. } = gradient;
					let (start, end) = (transform.transform_point2(*start), transform.transform_point2(*end));

					fn color_to_hex(color: graphene_std::Color) -> String {
						format!("#{}", color.with_alpha(1.).to_rgba_hex_srgb())
					}

					let start_hex = stops.color.first().map(|&c| color_to_hex(c)).unwrap_or(String::from(COLOR_OVERLAY_BLUE));
					let end_hex = stops.color.last().map(|&c| color_to_hex(c)).unwrap_or(String::from(COLOR_OVERLAY_BLUE));

					overlay_context.line(start, end, None, None);
					overlay_context.gradient_color_stop(start, dragging == Some(GradientDragTarget::Start), &start_hex);
					overlay_context.gradient_color_stop(end, dragging == Some(GradientDragTarget::End), &end_hex);

					for (index, stop) in stops.iter().enumerate() {
						if stop.position.abs() < f64::EPSILON * 1000. || (1. - stop.position).abs() < f64::EPSILON * 1000. {
							continue;
						}
						overlay_context.gradient_color_stop(start.lerp(end, stop.position), dragging == Some(GradientDragTarget::Step(index)), &color_to_hex(stop.color));
					}

					if let (Some(projection), Some(dir)) = (calculate_insertion(start, end, stops, mouse), (end - start).try_normalize()) {
						let perp = dir.perp();
						let point = start.lerp(end, projection);
						overlay_context.line(point - perp * SEGMENT_OVERLAY_SIZE, point + perp * SEGMENT_OVERLAY_SIZE, Some(COLOR_OVERLAY_BLUE), Some(1.));
					}
				}

				self
			}
			(GradientToolFsmState::Ready { .. }, GradientToolMessage::SelectionChanged) => {
				tool_data.selected_gradient = None;
				self
			}
			(GradientToolFsmState::Ready { .. }, GradientToolMessage::DeleteStop) => {
				let Some(selected_gradient) = &mut tool_data.selected_gradient else {
					return self;
				};

				// Skip if invalid gradient
				if selected_gradient.gradient.stops.len() < 2 {
					return self;
				}

				responses.add(DocumentMessage::StartTransaction);

				// Remove the selected point
				match selected_gradient.dragging {
					GradientDragTarget::Start => {
						selected_gradient.gradient.stops.remove(0);
					}
					GradientDragTarget::End => {
						let _ = selected_gradient.gradient.stops.pop();
					}
					GradientDragTarget::Step(index) => {
						selected_gradient.gradient.stops.remove(index);
					}
					GradientDragTarget::New => {}
				};

				// The gradient has only one point and so should become a fill
				if selected_gradient.gradient.stops.len() == 1 {
					if let Some(layer) = selected_gradient.layer {
						responses.add(GraphOperationMessage::FillSet {
							layer,
							fill: Fill::Solid(selected_gradient.gradient.stops.color[0]),
						});
					}
					responses.add(DocumentMessage::CommitTransaction);
					responses.add(PropertiesPanelMessage::Refresh);
					return self;
				}

				// Find the minimum and maximum positions
				let min_position = selected_gradient.gradient.stops.position.iter().copied().reduce(f64::min).expect("No min");
				let max_position = selected_gradient.gradient.stops.position.iter().copied().reduce(f64::max).expect("No max");

				// Recompute the start and end position of the gradient (in viewport transform)
				let transform = selected_gradient.transform;
				let (start, end) = (transform.transform_point2(selected_gradient.gradient.start), transform.transform_point2(selected_gradient.gradient.end));
				let (new_start, new_end) = (start.lerp(end, min_position), start.lerp(end, max_position));
				selected_gradient.gradient.start = transform.inverse().transform_point2(new_start);
				selected_gradient.gradient.end = transform.inverse().transform_point2(new_end);

				// Remap the positions
				for position in selected_gradient.gradient.stops.position.iter_mut() {
					*position = (*position - min_position) / (max_position - min_position);
				}

				// Render the new gradient
				selected_gradient.render_gradient(responses);
				responses.add(DocumentMessage::CommitTransaction);
				responses.add(PropertiesPanelMessage::Refresh);
				tool_data.selected_gradient = None;

				self
			}
			(_, GradientToolMessage::InsertStop) => {
				for layer in document.network_interface.selected_nodes().selected_visible_layers(&document.network_interface) {
					let Some(mut gradient) = get_gradient(layer, &document.network_interface) else { continue };
					// TODO: This transform is incorrect. I think this is since it is based on the Footprint which has not been updated yet
					let transform = gradient_space_transform(layer, document);
					let mouse = input.mouse.position;
					let (start, end) = (transform.transform_point2(gradient.start), transform.transform_point2(gradient.end));

					// Compute the distance from the mouse to the gradient line in viewport space
					let distance = (end - start).angle_to(mouse - start).sin() * (mouse - start).length();

					// If click is on the line then insert point
					if distance < (SELECTION_THRESHOLD * 2.) {
						// Try and insert the new stop
						if let Some(index) = gradient.insert_stop(mouse, transform) {
							responses.add(DocumentMessage::StartTransaction);

							let mut selected_gradient = SelectedGradient::new(gradient, layer, document);

							// Select the new point
							selected_gradient.dragging = GradientDragTarget::Step(index);

							// Update the layer fill
							selected_gradient.render_gradient(responses);

							tool_data.selected_gradient = Some(selected_gradient);
							responses.add(DocumentMessage::CommitTransaction);
							break;
						}
					}
				}

				self
			}
			(GradientToolFsmState::Ready { .. }, GradientToolMessage::PointerDown) => {
				let mouse = input.mouse.position;
				tool_data.drag_start = mouse;
				let tolerance = (MANIPULATOR_GROUP_MARKER_SIZE * 2.).powi(2);

				let mut dragging = false;
				let mut transaction_started = false;
				for layer in document.network_interface.selected_nodes().selected_visible_layers(&document.network_interface) {
					let Some(gradient) = get_gradient(layer, &document.network_interface) else { continue };
					let transform = gradient_space_transform(layer, document);
					// Check for dragging step
					for (index, stop) in gradient.stops.iter().enumerate() {
						let pos = transform.transform_point2(gradient.start.lerp(gradient.end, stop.position));
						if pos.distance_squared(mouse) < tolerance {
							dragging = true;
							tool_data.selected_gradient = Some(SelectedGradient {
								layer: Some(layer),
								transform,
								gradient: gradient.clone(),
								dragging: GradientDragTarget::Step(index),
								initial_gradient: gradient.clone(),
							})
						}
					}

					// Check dragging start or end handle
					for (pos, dragging_target) in [(gradient.start, GradientDragTarget::Start), (gradient.end, GradientDragTarget::End)] {
						let pos = transform.transform_point2(pos);
						if pos.distance_squared(mouse) < tolerance {
							dragging = true;
							tool_data.selected_gradient = Some(SelectedGradient {
								layer: Some(layer),
								transform,
								gradient: gradient.clone(),
								dragging: dragging_target,
								initial_gradient: gradient.clone(),
							})
						}
					}

					// Insert stop if clicking on line
					if !dragging {
						let (start, end) = (transform.transform_point2(gradient.start), transform.transform_point2(gradient.end));
						let distance = (end - start).angle_to(mouse - start).sin() * (mouse - start).length();
						let projection = ((end - start).angle_to(mouse - start)).cos() * start.distance(mouse) / start.distance(end);

						if distance.abs() < SEGMENT_INSERTION_DISTANCE
							&& (0. ..=1.).contains(&projection)
							&& let Some(index) = gradient.clone().insert_stop(mouse, transform)
						{
							responses.add(DocumentMessage::StartTransaction);
							transaction_started = true;
							let mut new_gradient = gradient.clone();
							new_gradient.insert_stop(mouse, transform);

							let mut selected_gradient = SelectedGradient::new(new_gradient, layer, document);
							selected_gradient.dragging = GradientDragTarget::Step(index);
							// No offset when inserting a new stop, it should be exactly under the mouse
							selected_gradient.render_gradient(responses);
							tool_data.selected_gradient = Some(selected_gradient);
							dragging = true;
						}
					}
				}

				let gradient_state = if dragging {
					GradientToolFsmState::Drawing
				} else {
					let selected_layer = document.click(input, viewport);

					// Apply the gradient to the selected layer
					if let Some(layer) = selected_layer {
						// Add check for raster layer
						if NodeGraphLayer::is_raster_layer(layer, &mut document.network_interface) {
							return GradientToolFsmState::Ready { hover_insertion: false };
						}
						if !document.network_interface.selected_nodes().selected_layers_contains(layer, document.metadata()) {
							let nodes = vec![layer.to_node()];

							responses.add(NodeGraphMessage::SelectedNodesSet { nodes });
						}

						// Use the already existing gradient if it exists
						let gradient = if let Some(gradient) = get_gradient(layer, &document.network_interface) {
							gradient.clone()
						} else {
							// Generate a new gradient
							Gradient::new(DVec2::ZERO, global_tool_data.secondary_color, DVec2::ONE, global_tool_data.primary_color, tool_options.gradient_type)
						};
						let mut selected_gradient = SelectedGradient::new(gradient, layer, document);
						selected_gradient.dragging = GradientDragTarget::New;

						tool_data.selected_gradient = Some(selected_gradient);

						GradientToolFsmState::Drawing
					} else {
						GradientToolFsmState::Ready { hover_insertion: false }
					}
				};

				if gradient_state == GradientToolFsmState::Drawing && !transaction_started {
					responses.add(DocumentMessage::StartTransaction);
				}

				responses.add(OverlaysMessage::Draw);

				gradient_state
			}
			(GradientToolFsmState::Drawing, GradientToolMessage::PointerMove { constrain_axis }) => {
				if let Some(selected_gradient) = &mut tool_data.selected_gradient {
					let mouse = input.mouse.position; // tool_data.snap_manager.snap_position(responses, document, input.mouse.position);
					selected_gradient.update_gradient(
						mouse,
						responses,
						input.keyboard.get(constrain_axis as usize),
						selected_gradient.gradient.gradient_type,
						tool_data.drag_start,
					);
				}

				// Auto-panning
				let messages = [
					GradientToolMessage::PointerOutsideViewport { constrain_axis }.into(),
					GradientToolMessage::PointerMove { constrain_axis }.into(),
				];
				tool_data.auto_panning.setup_by_mouse_position(input, viewport, &messages, responses);

				responses.add(OverlaysMessage::Draw);

				GradientToolFsmState::Drawing
			}
			(GradientToolFsmState::Drawing, GradientToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panning
				if let Some(shift) = tool_data.auto_panning.shift_viewport(input, viewport, responses)
					&& let Some(selected_gradient) = &mut tool_data.selected_gradient
				{
					selected_gradient.transform.translation += shift;
				}

				GradientToolFsmState::Drawing
			}
			(state, GradientToolMessage::PointerOutsideViewport { constrain_axis }) => {
				// Auto-panning
				let messages = [
					GradientToolMessage::PointerOutsideViewport { constrain_axis }.into(),
					GradientToolMessage::PointerMove { constrain_axis }.into(),
				];
				tool_data.auto_panning.stop(&messages, responses);

				state
			}
			(GradientToolFsmState::Drawing, GradientToolMessage::PointerUp) => {
				responses.add(DocumentMessage::EndTransaction);
				tool_data.snap_manager.cleanup(responses);
				let was_dragging = tool_data.selected_gradient.is_some();

				if !was_dragging
					&& let Some(selected_layer) = document.click(input, viewport)
					&& let Some(gradient) = get_gradient(selected_layer, &document.network_interface)
				{
					tool_data.selected_gradient = Some(SelectedGradient::new(gradient, selected_layer, document));
				}
				GradientToolFsmState::Ready { hover_insertion: false }
			}
			(GradientToolFsmState::Ready { .. }, GradientToolMessage::PointerMove { .. }) => {
				let mut hover_insertion = false;
				let mouse = input.mouse.position;

				for layer in document.network_interface.selected_nodes().selected_visible_layers(&document.network_interface) {
					let Some(gradient) = get_gradient(layer, &document.network_interface) else { continue };
					let transform = gradient_space_transform(layer, document);
					let start = transform.transform_point2(gradient.start);
					let end = transform.transform_point2(gradient.end);

					if calculate_insertion(start, end, &gradient.stops, mouse).is_some() {
						hover_insertion = true;
						break;
					}
				}

				responses.add(OverlaysMessage::Draw);
				GradientToolFsmState::Ready { hover_insertion }
			}

			(GradientToolFsmState::Drawing, GradientToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);
				tool_data.snap_manager.cleanup(responses);
				tool_data.selected_gradient = None;
				responses.add(OverlaysMessage::Draw);

				GradientToolFsmState::Ready { hover_insertion: false }
			}
			(_, GradientToolMessage::Abort) => GradientToolFsmState::Ready { hover_insertion: false },
			_ => self,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			GradientToolFsmState::Ready { hover_insertion } => {
				let hints = if *hover_insertion {
					vec![HintInfo::mouse(MouseMotion::Lmb, "Insert Color Stop")]
				} else {
					vec![HintInfo::mouse(MouseMotion::LmbDrag, "Draw Gradient"), HintInfo::keys([Key::Shift], "15° Increments").prepend_plus()]
				};
				HintData(vec![HintGroup(hints)])
			}
			GradientToolFsmState::Drawing => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
				HintGroup(vec![HintInfo::keys([Key::Shift], "15° Increments")]),
			]),
		};

		hint_data.send_layout(responses);
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}

#[cfg(test)]
mod test_gradient {
	use crate::messages::input_mapper::utility_types::input_mouse::EditorMouseState;
	use crate::messages::input_mapper::utility_types::input_mouse::ScrollDelta;
	use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
	use crate::messages::portfolio::document::utility_types::misc::GroupFolderType;
	pub use crate::test_utils::test_prelude::*;
	use glam::DAffine2;
	use graphene_std::vector::fill;
	use graphene_std::vector::style::Fill;
	use graphene_std::vector::style::Gradient;

	use super::gradient_space_transform;

	async fn get_fills(editor: &mut EditorTestUtils) -> Vec<(Fill, DAffine2)> {
		let instrumented = match editor.eval_graph().await {
			Ok(instrumented) => instrumented,
			Err(e) => panic!("Failed to evaluate graph: {e}"),
		};

		let document = editor.active_document();
		let layers = document.metadata().all_layers();
		layers
			.filter_map(|layer| {
				let fill = instrumented.grab_input_from_layer::<fill::FillInput<Fill>>(layer, &document.network_interface, &editor.runtime)?;
				let transform = gradient_space_transform(layer, document);
				Some((fill, transform))
			})
			.collect()
	}

	async fn get_gradient(editor: &mut EditorTestUtils) -> (Gradient, DAffine2) {
		let fills = get_fills(editor).await;
		assert_eq!(fills.len(), 1, "Expected 1 gradient fill, found {}", fills.len());

		let (fill, transform) = fills.first().unwrap();
		let gradient = fill.as_gradient().expect("Expected gradient fill type");

		(gradient.clone(), *transform)
	}

	fn assert_stops_at_positions(actual_positions: &[f64], expected_positions: &[f64], tolerance: f64) {
		assert_eq!(
			actual_positions.len(),
			expected_positions.len(),
			"Expected {} stops, found {}",
			expected_positions.len(),
			actual_positions.len()
		);

		for (i, (actual, expected)) in actual_positions.iter().zip(expected_positions.iter()).enumerate() {
			assert!((actual - expected).abs() < tolerance, "Stop {i}: Expected position near {expected}, got {actual}");
		}
	}

	#[tokio::test]
	async fn ignore_artboard() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Artboard, 0., 0., 100., 100., ModifierKeys::empty()).await;
		editor.drag_tool(ToolType::Gradient, 2., 2., 4., 4., ModifierKeys::empty()).await;
		assert!(get_fills(&mut editor).await.is_empty());
	}

	#[tokio::test]
	async fn ignore_raster() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.create_raster_image(Image::new(100, 100, Color::WHITE), Some((0., 0.))).await;
		editor.drag_tool(ToolType::Gradient, 2., 2., 4., 4., ModifierKeys::empty()).await;
		assert!(get_fills(&mut editor).await.is_empty());
	}

	#[tokio::test]
	async fn simple_draw() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Rectangle, -5., -3., 100., 100., ModifierKeys::empty()).await;
		editor.select_primary_color(Color::GREEN).await;
		editor.select_secondary_color(Color::BLUE).await;
		editor.drag_tool(ToolType::Gradient, 2., 3., 24., 4., ModifierKeys::empty()).await;

		let (gradient, transform) = get_gradient(&mut editor).await;

		// Gradient goes from secondary color to primary color
		let stops = gradient.stops.iter().map(|stop| (stop.position, stop.color.to_rgba8_srgb())).collect::<Vec<_>>();
		assert_eq!(stops, vec![(0., Color::BLUE.to_rgba8_srgb()), (1., Color::GREEN.to_rgba8_srgb())]);
		assert!(transform.transform_point2(gradient.start).abs_diff_eq(DVec2::new(2., 3.), 1e-10));
		assert!(transform.transform_point2(gradient.end).abs_diff_eq(DVec2::new(24., 4.), 1e-10));
	}

	#[tokio::test]
	async fn snap_simple_draw() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor
			.handle_message(NavigationMessage::CanvasTiltSet {
				angle_radians: f64::consts::FRAC_PI_8,
			})
			.await;
		let start = DVec2::new(0., 0.);
		let end = DVec2::new(24., 4.);
		editor.drag_tool(ToolType::Rectangle, -5., -3., 100., 100., ModifierKeys::empty()).await;
		editor.drag_tool(ToolType::Gradient, start.x, start.y, end.x, end.y, ModifierKeys::SHIFT).await;

		let (gradient, transform) = get_gradient(&mut editor).await;

		assert!(transform.transform_point2(gradient.start).abs_diff_eq(start, 1e-10));

		// 15 degrees from horizontal
		let angle = f64::to_radians(15.);
		let direction = DVec2::new(angle.cos(), angle.sin());
		let expected = start + direction * (end - start).length();
		assert!(transform.transform_point2(gradient.end).abs_diff_eq(expected, 1e-10));
	}

	#[tokio::test]
	async fn transformed_draw() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor
			.handle_message(NavigationMessage::CanvasTiltSet {
				angle_radians: f64::consts::FRAC_PI_8,
			})
			.await;
		editor.drag_tool(ToolType::Rectangle, -5., -3., 100., 100., ModifierKeys::empty()).await;

		// Group rectangle
		let group_folder_type = GroupFolderType::Layer;
		editor.handle_message(DocumentMessage::GroupSelectedLayers { group_folder_type }).await;
		let metadata = editor.active_document().metadata();
		let mut layers = metadata.all_layers();
		let folder = layers.next().unwrap();
		let rectangle = layers.next().unwrap();
		assert_eq!(rectangle.parent(metadata), Some(folder));

		// Transform the group
		editor
			.handle_message(GraphOperationMessage::TransformSet {
				layer: folder,
				transform: DAffine2::from_scale_angle_translation(DVec2::new(1., 2.), 0., -DVec2::X * 10.),
				transform_in: TransformIn::Local,
				skip_rerender: false,
			})
			.await;

		editor.drag_tool(ToolType::Gradient, 2., 3., 24., 4., ModifierKeys::empty()).await;

		let (gradient, transform) = get_gradient(&mut editor).await;

		assert!(transform.transform_point2(gradient.start).abs_diff_eq(DVec2::new(2., 3.), 1e-10));
		assert!(transform.transform_point2(gradient.end).abs_diff_eq(DVec2::new(24., 4.), 1e-10));
	}

	#[tokio::test]
	async fn click_to_insert_stop() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		editor.drag_tool(ToolType::Rectangle, -5., -3., 100., 100., ModifierKeys::empty()).await;
		editor.select_primary_color(Color::GREEN).await;
		editor.select_secondary_color(Color::BLUE).await;
		editor.drag_tool(ToolType::Gradient, 0., 0., 100., 0., ModifierKeys::empty()).await;

		// Get initial gradient state (should have 2 stops)
		let (initial_gradient, _) = get_gradient(&mut editor).await;
		assert_eq!(initial_gradient.stops.len(), 2, "Expected 2 stops, found {}", initial_gradient.stops.len());

		editor.select_tool(ToolType::Gradient).await;
		editor.move_mouse(50., 0., ModifierKeys::empty(), MouseKeys::empty()).await;
		editor.left_mousedown(50., 0., ModifierKeys::empty()).await;
		editor.left_mouseup(50., 0., ModifierKeys::empty()).await;

		// Check that a new stop has been added
		let (updated_gradient, _) = get_gradient(&mut editor).await;
		assert_eq!(updated_gradient.stops.len(), 3, "Expected 3 stops, found {}", updated_gradient.stops.len());

		let positions: Vec<f64> = updated_gradient.stops.iter().map(|stop| stop.position).collect();
		assert!(
			positions.iter().any(|pos| (pos - 0.5).abs() < 0.1),
			"Expected to find a stop near position 0.5, but found: {positions:?}"
		);
	}

	#[tokio::test]
	async fn dragging_endpoint_sets_correct_point() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		editor.handle_message(NavigationMessage::CanvasZoomSet { zoom_factor: 2. }).await;

		editor.drag_tool(ToolType::Rectangle, -5., -3., 100., 100., ModifierKeys::empty()).await;

		let document = editor.active_document();
		let selected_layer = document.network_interface.selected_nodes().selected_layers(document.metadata()).next().unwrap();
		editor
			.handle_message(GraphOperationMessage::TransformSet {
				layer: selected_layer,
				transform: DAffine2::from_scale_angle_translation(DVec2::new(1.5, 0.8), 0.3, DVec2::new(10., -5.)),
				transform_in: TransformIn::Local,
				skip_rerender: false,
			})
			.await;

		editor.select_primary_color(Color::GREEN).await;
		editor.select_secondary_color(Color::BLUE).await;

		editor.drag_tool(ToolType::Gradient, 0., 0., 100., 0., ModifierKeys::empty()).await;

		// Get the initial gradient state
		let (initial_gradient, transform) = get_gradient(&mut editor).await;
		assert_eq!(initial_gradient.stops.len(), 2, "Expected 2 stops, found {}", initial_gradient.stops.len());

		// Verify initial gradient endpoints in viewport space
		let initial_start = transform.transform_point2(initial_gradient.start);
		let initial_end = transform.transform_point2(initial_gradient.end);
		assert!(initial_start.abs_diff_eq(DVec2::new(0., 0.), 1e-10));
		assert!(initial_end.abs_diff_eq(DVec2::new(100., 0.), 1e-10));

		editor.select_tool(ToolType::Gradient).await;

		// Simulate dragging the end point to a new position (100, 50)
		let start_pos = DVec2::new(100., 0.);
		let end_pos = DVec2::new(100., 50.);

		editor.move_mouse(start_pos.x, start_pos.y, ModifierKeys::empty(), MouseKeys::empty()).await;
		editor.left_mousedown(start_pos.x, start_pos.y, ModifierKeys::empty()).await;
		editor.move_mouse(end_pos.x, end_pos.y, ModifierKeys::empty(), MouseKeys::LEFT).await;
		editor
			.mouseup(
				EditorMouseState {
					editor_position: end_pos,
					mouse_keys: MouseKeys::empty(),
					scroll_delta: ScrollDelta::default(),
				},
				ModifierKeys::empty(),
			)
			.await;

		// Check the updated gradient
		let (updated_gradient, transform) = get_gradient(&mut editor).await;

		// Verify the start point hasn't changed
		let updated_start = transform.transform_point2(updated_gradient.start);
		assert!(updated_start.abs_diff_eq(DVec2::new(0., 0.), 1e-10));

		// Verify the end point has been updated to the new position
		let updated_end = transform.transform_point2(updated_gradient.end);
		assert!(updated_end.abs_diff_eq(DVec2::new(100., 50.), 1e-10), "Expected end point at (100, 50), got {updated_end:?}");
	}

	#[tokio::test]
	async fn dragging_stop_reorders_gradient() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		editor.drag_tool(ToolType::Rectangle, -5., -3., 100., 100., ModifierKeys::empty()).await;
		editor.select_primary_color(Color::GREEN).await;
		editor.select_secondary_color(Color::BLUE).await;
		editor.drag_tool(ToolType::Gradient, 0., 0., 100., 0., ModifierKeys::empty()).await;

		editor.select_tool(ToolType::Gradient).await;

		// Add a middle stop at 50%
		editor.move_mouse(50., 0., ModifierKeys::empty(), MouseKeys::empty()).await;
		editor.left_mousedown(50., 0., ModifierKeys::empty()).await;
		editor.left_mouseup(50., 0., ModifierKeys::empty()).await;

		let (initial_gradient, _) = get_gradient(&mut editor).await;
		assert_eq!(initial_gradient.stops.len(), 3, "Expected 3 stops, found {}", initial_gradient.stops.len());

		// Verify initial stop positions and colors
		let mut stops = initial_gradient.stops.clone();
		stops.sort();

		let positions: Vec<f64> = stops.iter().map(|stop| stop.position).collect();
		assert_stops_at_positions(&positions, &[0., 0.5, 1.], 0.1);

		let middle_color = stops.color[1].to_rgba8_srgb();

		// Simulate dragging the middle stop to position 0.8
		let click_position = DVec2::new(50., 0.);
		editor
			.mousedown(
				EditorMouseState {
					editor_position: click_position,
					mouse_keys: MouseKeys::LEFT,
					scroll_delta: ScrollDelta::default(),
				},
				ModifierKeys::empty(),
			)
			.await;

		let drag_position = DVec2::new(80., 0.);
		editor.move_mouse(drag_position.x, drag_position.y, ModifierKeys::empty(), MouseKeys::LEFT).await;

		editor
			.mouseup(
				EditorMouseState {
					editor_position: drag_position,
					mouse_keys: MouseKeys::empty(),
					scroll_delta: ScrollDelta::default(),
				},
				ModifierKeys::empty(),
			)
			.await;

		let (updated_gradient, _) = get_gradient(&mut editor).await;
		assert_eq!(updated_gradient.stops.len(), 3, "Expected 3 stops after dragging, found {}", updated_gradient.stops.len());

		// Verify updated stop positions and colors
		let mut updated_stops = updated_gradient.stops.clone();
		updated_stops.sort();

		// Check positions are now correctly ordered
		let updated_positions: Vec<f64> = updated_stops.iter().map(|stop| stop.position).collect();
		assert_stops_at_positions(&updated_positions, &[0., 0.8, 1.], 0.1);

		// Colors should maintain their associations with the stop points
		assert_eq!(updated_stops.color[0].to_rgba8_srgb(), Color::BLUE.to_rgba8_srgb());
		assert_eq!(updated_stops.color[1].to_rgba8_srgb(), middle_color);
		assert_eq!(updated_stops.color[2].to_rgba8_srgb(), Color::GREEN.to_rgba8_srgb());
	}

	#[tokio::test]
	async fn select_and_delete_removes_stop() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		editor.drag_tool(ToolType::Rectangle, -5., -3., 100., 100., ModifierKeys::empty()).await;
		editor.select_primary_color(Color::GREEN).await;
		editor.select_secondary_color(Color::BLUE).await;
		editor.drag_tool(ToolType::Gradient, 0., 0., 100., 0., ModifierKeys::empty()).await;

		// Get initial gradient state (should have 2 stops)
		let (initial_gradient, _) = get_gradient(&mut editor).await;
		assert_eq!(initial_gradient.stops.len(), 2, "Expected 2 stops, found {}", initial_gradient.stops.len());

		editor.select_tool(ToolType::Gradient).await;

		// Add two middle stops
		editor.move_mouse(25., 0., ModifierKeys::empty(), MouseKeys::empty()).await;
		editor.left_mousedown(25., 0., ModifierKeys::empty()).await;
		editor.left_mouseup(25., 0., ModifierKeys::empty()).await;

		editor.move_mouse(75., 0., ModifierKeys::empty(), MouseKeys::empty()).await;
		editor.left_mousedown(75., 0., ModifierKeys::empty()).await;
		editor.left_mouseup(75., 0., ModifierKeys::empty()).await;

		let (updated_gradient, _) = get_gradient(&mut editor).await;
		assert_eq!(updated_gradient.stops.len(), 4, "Expected 4 stops, found {}", updated_gradient.stops.len());

		let positions: Vec<f64> = updated_gradient.stops.iter().map(|stop| stop.position).collect();

		// Use helper function to verify positions
		assert_stops_at_positions(&positions, &[0., 0.25, 0.75, 1.], 0.05);

		// Select the stop at position 0.75 and delete it
		let position2 = DVec2::new(75., 0.);
		editor.move_mouse(position2.x, position2.y, ModifierKeys::empty(), MouseKeys::empty()).await;
		editor.left_mousedown(position2.x, position2.y, ModifierKeys::empty()).await;
		editor
			.mouseup(
				EditorMouseState {
					editor_position: position2,
					mouse_keys: MouseKeys::empty(),
					scroll_delta: ScrollDelta::default(),
				},
				ModifierKeys::empty(),
			)
			.await;

		editor.press(Key::Delete, ModifierKeys::empty()).await;

		// Verify we now have 3 stops
		let (final_gradient, _) = get_gradient(&mut editor).await;
		assert_eq!(final_gradient.stops.len(), 3, "Expected 3 stops after deletion, found {}", final_gradient.stops.len());

		let final_positions: Vec<f64> = final_gradient.stops.iter().map(|stop| stop.position).collect();

		// Verify final positions with helper function
		assert_stops_at_positions(&final_positions, &[0., 0.25, 1.], 0.05);

		// Additional verification that 0.75 stop is gone
		assert!(!final_positions.iter().any(|pos| (pos - 0.75).abs() < 0.05), "Stop at position 0.75 should have been deleted");
	}
}

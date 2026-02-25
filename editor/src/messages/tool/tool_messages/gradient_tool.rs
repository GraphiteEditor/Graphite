use super::tool_prelude::*;
use crate::consts::{
	COLOR_OVERLAY_BLUE, DRAG_THRESHOLD, GRADIENT_MIDPOINT_DIAMOND_RADIUS, GRADIENT_MIDPOINT_MAX, GRADIENT_MIDPOINT_MIN, GRADIENT_STOP_MIN_VIEWPORT_GAP, LINE_ROTATE_SNAP_ANGLE,
	MANIPULATOR_GROUP_MARKER_SIZE, SEGMENT_INSERTION_DISTANCE, SEGMENT_OVERLAY_SIZE, SELECTION_THRESHOLD,
};
use crate::messages::portfolio::document::overlays::utility_types::{GizmoEmphasis, OverlayContext};
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::graph_modification_utils::{NodeGraphLayer, get_gradient};
use crate::messages::tool::common_functionality::snapping::{SnapCandidatePoint, SnapConstraint, SnapData, SnapManager, SnapTypeConfiguration};
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
	DoubleClick,
	InsertStop,
	PointerDown,
	PointerMove { constrain_axis: Key, lock_angle: Key },
	PointerOutsideViewport { constrain_axis: Key, lock_angle: Key },
	PointerUp,
	UpdateOptions { options: GradientOptionsUpdate },
}

#[derive(PartialEq, Eq, Clone, Debug, Hash, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum GradientOptionsUpdate {
	Type(GradientType),
	ReverseStops,
	ReverseDirection,
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

			let has_gradient = has_gradient_on_selected_layers(context.document);
			if has_gradient != self.data.has_selected_gradient {
				self.data.has_selected_gradient = has_gradient;
				responses.add(ToolMessage::RefreshToolOptions);
			}

			return;
		};
		match options {
			GradientOptionsUpdate::Type(gradient_type) => {
				self.options.gradient_type = gradient_type;
				apply_gradient_update(&mut self.data, context, responses, |g| g.gradient_type != gradient_type, |g| g.gradient_type = gradient_type);
				responses.add(ToolMessage::UpdateHints);
				responses.add(ToolMessage::UpdateCursor);
			}
			GradientOptionsUpdate::ReverseStops => {
				apply_gradient_update(&mut self.data, context, responses, |_| true, |g| g.stops = g.stops.reversed());
			}
			GradientOptionsUpdate::ReverseDirection => {
				apply_gradient_update(&mut self.data, context, responses, |_| true, |g| std::mem::swap(&mut g.start, &mut g.end));
			}
		}
	}

	advertise_actions!(GradientToolMessageDiscriminant;
		PointerDown,
		PointerUp,
		PointerMove,
		DoubleClick,
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

		let reverse_stops = IconButton::new("Reverse", 24)
			.tooltip_label("Reverse Stops")
			.tooltip_description("Reverse the gradient color stops.")
			.disabled(!self.data.has_selected_gradient)
			.on_update(|_| {
				GradientToolMessage::UpdateOptions {
					options: GradientOptionsUpdate::ReverseStops,
				}
				.into()
			})
			.widget_instance();

		let mut widgets = vec![gradient_type, Separator::new(SeparatorStyle::Unrelated).widget_instance(), reverse_stops];

		if self.options.gradient_type == GradientType::Radial {
			let orientation = self
				.data
				.selected_gradient
				.as_ref()
				.map(|selected_gradient| {
					let (start, end) = (selected_gradient.gradient.start, selected_gradient.gradient.end);
					if (end.x - start.x).abs() > f64::EPSILON * 1e6 {
						end.x > start.x
					} else {
						(start.x + start.y) < (end.x + end.y)
					}
				})
				.unwrap_or(true);

			let reverse_direction = IconButton::new(if orientation { "ReverseRadialGradientToRight" } else { "ReverseRadialGradientToLeft" }, 24)
				.tooltip_label("Reverse Direction")
				.tooltip_description("Reverse which end the gradient radiates from.")
				.disabled(!self.data.has_selected_gradient)
				.on_update(|_| {
					GradientToolMessage::UpdateOptions {
						options: GradientOptionsUpdate::ReverseDirection,
					}
					.into()
				})
				.widget_instance();

			widgets.push(Separator::new(SeparatorStyle::Related).widget_instance());
			widgets.push(reverse_direction);
		}

		Layout(vec![LayoutGroup::Row { widgets }])
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GradientToolFsmState {
	Ready { hovering: GradientHoverTarget, selected: GradientSelectedTarget },
	Drawing { drag_hint: GradientDragHintState },
}

impl Default for GradientToolFsmState {
	fn default() -> Self {
		Self::Ready {
			hovering: GradientHoverTarget::None,
			selected: GradientSelectedTarget::None,
		}
	}
}

/// Computes the transform from gradient space to viewport space (where gradient space is 0..1)
fn gradient_space_transform(layer: LayerNodeIdentifier, document: &DocumentMessageHandler) -> DAffine2 {
	let bounds = document.metadata().nonzero_bounding_box(layer);
	let bound_transform = DAffine2::from_scale_angle_translation(bounds[1] - bounds[0], 0., bounds[0]);

	let multiplied = document.metadata().transform_to_viewport(layer);

	multiplied * bound_transform
}

/// Whether two adjacent stops are too closely packed in viewport space for a midpoint diamond to be shown or interacted with.
fn midpoint_hidden_by_proximity(left_stop_pos: f64, right_stop_pos: f64, viewport_line_length: f64) -> bool {
	(right_stop_pos - left_stop_pos) * viewport_line_length < GRADIENT_STOP_MIN_VIEWPORT_GAP * 2.
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, Default)]
pub enum GradientDragTarget {
	Start,
	#[default]
	End,
	Stop(usize),
	Midpoint(usize),
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

		// Don't insert when clicking near a (currently visible) midpoint diamond
		let line_length = start.distance(end);
		for i in 0..stops.position.len().saturating_sub(1) {
			let left = stops.position[i];
			let right = stops.position[i + 1];

			if midpoint_hidden_by_proximity(left, right, line_length) {
				continue;
			}

			let midpoint_pos = left + stops.midpoint[i] * (right - left);
			let midpoint_viewport = start.lerp(end, midpoint_pos);
			if midpoint_viewport.distance_squared(mouse) < GRADIENT_MIDPOINT_DIAMOND_RADIUS.powi(2) {
				return None;
			}
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

	#[allow(clippy::too_many_arguments)]
	pub fn update_gradient(
		&mut self,
		mut mouse: DVec2,
		responses: &mut VecDeque<Message>,
		snap_rotate: bool,
		lock_angle: bool,
		gradient_type: GradientType,
		drag_start: DVec2,
		snap_data: SnapData,
		snap_manager: &mut SnapManager,
		gradient_angle: &mut f64,
	) {
		if mouse.distance(drag_start) < DRAG_THRESHOLD {
			self.gradient = self.initial_gradient.clone();
			self.render_gradient(responses);
			return;
		}

		self.gradient.gradient_type = gradient_type;

		if (lock_angle || snap_rotate) && matches!(self.dragging, GradientDragTarget::End | GradientDragTarget::Start | GradientDragTarget::New) {
			let point = if self.dragging == GradientDragTarget::Start {
				self.transform.transform_point2(self.gradient.end)
			} else if self.dragging == GradientDragTarget::New {
				drag_start
			} else {
				self.transform.transform_point2(self.gradient.start)
			};

			let delta = point - mouse;

			let mut angle = -delta.angle_to(DVec2::X);

			if lock_angle {
				angle = *gradient_angle;
			} else if snap_rotate {
				let snap_resolution = LINE_ROTATE_SNAP_ANGLE.to_radians();
				angle = (angle / snap_resolution).round() * snap_resolution;
			}

			*gradient_angle = angle;

			if lock_angle {
				let unit_direction = DVec2::new(angle.cos(), angle.sin());
				let length = delta.dot(unit_direction);
				mouse = point - length * unit_direction;
			} else {
				let length = delta.length();
				let rotated = DVec2::new(length * angle.cos(), length * angle.sin());
				mouse = point - rotated;
			}
		} else {
			// Update stored angle even when not constraining (for dragging endpoints and drawing a new gradient)
			if matches!(self.dragging, GradientDragTarget::End | GradientDragTarget::Start | GradientDragTarget::New) {
				let point = if self.dragging == GradientDragTarget::Start {
					self.transform.transform_point2(self.gradient.end)
				} else if self.dragging == GradientDragTarget::New {
					drag_start
				} else {
					self.transform.transform_point2(self.gradient.start)
				};

				let delta = point - mouse;
				*gradient_angle = -delta.angle_to(DVec2::X);
			}

			// Basic point snapping when not angle-constraining
			let document_to_viewport = snap_data.document.metadata().document_to_viewport;
			let document_mouse = document_to_viewport.inverse().transform_point2(mouse);
			let point_candidate = SnapCandidatePoint::gradient_handle(document_mouse);
			let snapped = snap_manager.free_snap(&snap_data, &point_candidate, SnapTypeConfiguration::default());
			if snapped.is_snapped() {
				mouse = document_to_viewport.transform_point2(snapped.snapped_point_document);
			}
			snap_manager.update_indicator(snapped);
		}

		let transformed_mouse = self.transform.inverse().transform_point2(mouse);

		match self.dragging {
			GradientDragTarget::Start => {
				self.gradient.start = transformed_mouse;
			}
			GradientDragTarget::End => {
				self.gradient.end = transformed_mouse;
			}
			GradientDragTarget::New => {
				self.gradient.start = self.transform.inverse().transform_point2(drag_start);
				self.gradient.end = transformed_mouse;
			}
			GradientDragTarget::Stop(s) => {
				let document_to_viewport = snap_data.document.metadata().document_to_viewport;

				let (viewport_start, viewport_end) = (self.transform.transform_point2(self.gradient.start), self.transform.transform_point2(self.gradient.end));

				let line_length = viewport_start.distance(viewport_end);
				if line_length < f64::EPSILON {
					self.render_gradient(responses);
					return;
				}

				let (document_start, document_end) = (
					document_to_viewport.inverse().transform_point2(viewport_start),
					document_to_viewport.inverse().transform_point2(viewport_end),
				);

				let constraint = SnapConstraint::Line {
					origin: document_start,
					direction: document_end - document_start,
				};

				let document_mouse = document_to_viewport.inverse().transform_point2(mouse);
				let point_candidate = SnapCandidatePoint::gradient_handle(document_mouse);

				let snapped = snap_manager.constrained_snap(&snap_data, &point_candidate, constraint, SnapTypeConfiguration::default());

				let projected_mouse_document = if snapped.is_snapped() {
					snapped.snapped_point_document
				} else {
					constraint.projection(document_mouse)
				};
				let projected_mouse = document_to_viewport.transform_point2(projected_mouse_document);
				snap_manager.update_indicator(snapped);

				// Calculate the new position by finding the closest point on the line
				let new_pos = ((viewport_end - viewport_start).angle_to(projected_mouse - viewport_start)).cos() * viewport_start.distance(projected_mouse) / line_length;

				if !new_pos.is_finite() {
					self.render_gradient(responses);
					return;
				}

				// Allow dragging through other stops (they'll reorder via sort), but clamp near
				// the endpoints at 0 and 1 if a different color stop already occupies that position
				let min_gap = GRADIENT_STOP_MIN_VIEWPORT_GAP / line_length;
				let last_index = self.gradient.stops.len() - 1;

				let has_other_stop_at_zero = s != 0 && self.gradient.stops.position.first().is_some_and(|&p| p.abs() < f64::EPSILON * 1000.);
				let has_other_stop_at_one = s != last_index && self.gradient.stops.position.last().is_some_and(|&p| (1. - p).abs() < f64::EPSILON * 1000.);

				let left_bound = if has_other_stop_at_zero { min_gap } else { 0. };
				let right_bound = if has_other_stop_at_one { 1. - min_gap } else { 1. };

				let clamped = new_pos.clamp(left_bound, right_bound);
				self.gradient.stops.position[s] = clamped;
				let new_position = self.gradient.stops.position[s];
				let new_color = self.gradient.stops.color[s];

				self.gradient.stops.sort();
				if let Some(new_index) = self.gradient.stops.iter().position(|s| s.position == new_position && s.color == new_color) {
					self.dragging = GradientDragTarget::Stop(new_index);
				}
			}
			GradientDragTarget::Midpoint(midpoint_index) => {
				let document_to_viewport = snap_data.document.metadata().document_to_viewport;

				let (viewport_start, viewport_end) = (self.transform.transform_point2(self.gradient.start), self.transform.transform_point2(self.gradient.end));

				let line_length = viewport_start.distance(viewport_end);
				if line_length < f64::EPSILON {
					self.render_gradient(responses);
					return;
				}

				let (document_start, document_end) = (
					document_to_viewport.inverse().transform_point2(viewport_start),
					document_to_viewport.inverse().transform_point2(viewport_end),
				);

				let constraint = SnapConstraint::Line {
					origin: document_start,
					direction: document_end - document_start,
				};

				let document_mouse = document_to_viewport.inverse().transform_point2(mouse);
				let point_candidate = SnapCandidatePoint::gradient_handle(document_mouse);

				let snapped = snap_manager.constrained_snap(&snap_data, &point_candidate, constraint, SnapTypeConfiguration::default());

				let projected_mouse_document = if snapped.is_snapped() {
					snapped.snapped_point_document
				} else {
					constraint.projection(document_mouse)
				};
				let projected_mouse = document_to_viewport.transform_point2(projected_mouse_document);
				snap_manager.update_indicator(snapped);

				// Calculate the position along the full gradient (0-1)
				let full_pos = ((viewport_end - viewport_start).angle_to(projected_mouse - viewport_start)).cos() * viewport_start.distance(projected_mouse) / line_length;

				if !full_pos.is_finite() {
					self.render_gradient(responses);
					return;
				}

				// Convert to a midpoint ratio within the interval between the two surrounding stops
				let left_stop = self.gradient.stops.position[midpoint_index];
				let right_stop = self.gradient.stops.position[midpoint_index + 1];
				let range = right_stop - left_stop;
				if range > 0. {
					let midpoint_ratio = ((full_pos - left_stop) / range).clamp(GRADIENT_MIDPOINT_MIN, GRADIENT_MIDPOINT_MAX);
					self.gradient.stops.midpoint[midpoint_index] = midpoint_ratio;
				}
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
	auto_pan_shift: DVec2,
	gradient_angle: f64,
	has_selected_gradient: bool,
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

					let gradient = if matches!(self, GradientToolFsmState::Drawing { .. })
						&& dragging.is_some()
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

					// Check if the first/last stops are at position ~0/~1 (rendered as the endpoint dots rather than as separate stops)
					let first_at_start = stops.position.first().is_some_and(|&p| p.abs() < f64::EPSILON * 1000.);
					let last_at_end = stops.position.last().is_some_and(|&p| (1. - p).abs() < f64::EPSILON * 1000.);

					overlay_context.line(start, end, None, None);

					// Determine which stop is selected (being dragged) and hovered (closest to mouse)
					// so they can be drawn last to appear on top of other overlapping stops
					let selected_stop_id: Option<StopId> = match dragging {
						Some(GradientDragTarget::Start) => Some(StopId::Start),
						Some(GradientDragTarget::End) => Some(StopId::End),
						Some(GradientDragTarget::Stop(0)) if first_at_start => Some(StopId::Start),
						Some(GradientDragTarget::Stop(i)) if last_at_end && i == stops.len() - 1 => Some(StopId::End),
						Some(GradientDragTarget::Stop(i)) => Some(StopId::Middle(i)),
						_ => None,
					};
					let stop_tolerance = (MANIPULATOR_GROUP_MARKER_SIZE * 2.).powi(2);
					let hovered_stop_id: Option<StopId> = if !matches!(self, GradientToolFsmState::Drawing { .. }) {
						// Find the closest stop to the mouse (matching the click detection logic)
						let mut best: Option<(f64, StopId)> = None;
						let mut check = |dist_sq: f64, id: StopId| {
							if dist_sq < stop_tolerance && best.as_ref().is_none_or(|&(d, _)| dist_sq < d) {
								best = Some((dist_sq, id));
							}
						};
						check(start.distance_squared(mouse), StopId::Start);
						check(end.distance_squared(mouse), StopId::End);
						for (index, stop) in stops.iter().enumerate() {
							if stop.position.abs() < f64::EPSILON * 1000. || (1. - stop.position).abs() < f64::EPSILON * 1000. {
								continue;
							}
							check(start.lerp(end, stop.position).distance_squared(mouse), StopId::Middle(index));
						}
						best.map(|(_, id)| id)
					} else {
						None
					};

					// Draw order: regular stops first, then selected, then hovered (so hovered appears on top)
					let is_deferred = |id: StopId| -> bool { Some(id) == selected_stop_id || Some(id) == hovered_stop_id };
					let emphasis_for = |id: StopId| -> GizmoEmphasis {
						if Some(id) == selected_stop_id {
							GizmoEmphasis::Active
						} else if Some(id) == hovered_stop_id {
							GizmoEmphasis::Hovered
						} else {
							GizmoEmphasis::Regular
						}
					};
					let mut draw_stop = |id: StopId, emphasis: GizmoEmphasis| match id {
						StopId::Start => overlay_context.gradient_color_stop(start, emphasis, &start_hex, !first_at_start),
						StopId::End => overlay_context.gradient_color_stop(end, emphasis, &end_hex, !last_at_end),
						StopId::Middle(i) => {
							if let Some(stop) = stops.iter().nth(i) {
								overlay_context.gradient_color_stop(start.lerp(end, stop.position), emphasis, &color_to_hex(stop.color), false);
							}
						}
					};

					// Draw regular (non-deferred) stops
					if !is_deferred(StopId::Start) {
						draw_stop(StopId::Start, emphasis_for(StopId::Start));
					}
					if !is_deferred(StopId::End) {
						draw_stop(StopId::End, emphasis_for(StopId::End));
					}
					for (index, stop) in stops.iter().enumerate() {
						if stop.position.abs() < f64::EPSILON * 1000. || (1. - stop.position).abs() < f64::EPSILON * 1000. {
							continue;
						}
						let id = StopId::Middle(index);
						if !is_deferred(id) {
							draw_stop(id, emphasis_for(id));
						}
					}

					// Draw selected stop (if not also hovered)
					if let Some(selected_id) = selected_stop_id
						&& Some(selected_id) != hovered_stop_id
					{
						draw_stop(selected_id, GizmoEmphasis::Active);
					}

					// Draw hovered stop last (on top of everything)
					if let Some(hov_id) = hovered_stop_id {
						let emphasis = if Some(hov_id) == selected_stop_id { GizmoEmphasis::Active } else { GizmoEmphasis::Hovered };
						draw_stop(hov_id, emphasis);
					}

					// Draw midpoint diamonds between adjacent stops (hidden when stops are too close in viewport space)
					let line_angle = (end - start).to_angle();
					let line_length = start.distance(end);
					let midpoint_tolerance = GRADIENT_MIDPOINT_DIAMOND_RADIUS.powi(2);
					for i in 0..stops.position.len().saturating_sub(1) {
						let left = stops.position[i];
						let right = stops.position[i + 1];

						if midpoint_hidden_by_proximity(left, right, line_length) {
							continue;
						}

						let midpoint_pos = left + stops.midpoint[i] * (right - left);
						let midpoint_viewport = start.lerp(end, midpoint_pos);

						let emphasis = if dragging == Some(GradientDragTarget::Midpoint(i)) {
							GizmoEmphasis::Active
						} else if !matches!(self, GradientToolFsmState::Drawing { .. }) && midpoint_viewport.distance_squared(mouse) < midpoint_tolerance {
							GizmoEmphasis::Hovered
						} else {
							GizmoEmphasis::Regular
						};
						overlay_context.gradient_midpoint(midpoint_viewport, emphasis, line_angle);
					}

					if !matches!(self, GradientToolFsmState::Drawing { .. })
						&& calculate_insertion(start, end, stops, mouse).is_some()
						&& let Some(dir) = (end - start).try_normalize()
					{
						let perp = dir.perp();

						// Snap the insertion point along the gradient line
						let document_to_viewport = document.metadata().document_to_viewport;

						let (document_start, document_end) = (document_to_viewport.inverse().transform_point2(start), document_to_viewport.inverse().transform_point2(end));
						let constraint = SnapConstraint::Line {
							origin: document_start,
							direction: document_end - document_start,
						};

						let document_mouse = document_to_viewport.inverse().transform_point2(mouse);
						let point_candidate = SnapCandidatePoint::gradient_handle(document_mouse);

						let snap_data = SnapData::new(document, input, viewport);
						let snapped = tool_data.snap_manager.constrained_snap(&snap_data, &point_candidate, constraint, SnapTypeConfiguration::default());

						let snapped_point = if snapped.is_snapped() {
							document_to_viewport.transform_point2(snapped.snapped_point_document)
						} else {
							let projected = constraint.projection(document_mouse);
							document_to_viewport.transform_point2(projected)
						};

						overlay_context.line(
							snapped_point - perp * SEGMENT_OVERLAY_SIZE,
							snapped_point + perp * SEGMENT_OVERLAY_SIZE,
							Some(COLOR_OVERLAY_BLUE),
							Some(1.),
						);
					}
				}

				let snap_data = SnapData::new(document, input, viewport);
				tool_data.snap_manager.draw_overlays(snap_data, &mut overlay_context);

				self
			}
			(GradientToolFsmState::Ready { .. }, GradientToolMessage::SelectionChanged) => {
				tool_data.selected_gradient = None;
				GradientToolFsmState::Ready {
					hovering: GradientHoverTarget::None,
					selected: GradientSelectedTarget::None,
				}
			}
			(_, GradientToolMessage::DoubleClick) => {
				// Only reset if the mouse hasn't moved so we don't trigger from a click-then-click-and-drag being reported as a double-click
				let drag_start_viewport = document.metadata().document_to_viewport.transform_point2(tool_data.drag_start);
				if input.mouse.position.distance(drag_start_viewport) <= DRAG_THRESHOLD
					&& let Some(selected_gradient) = &mut tool_data.selected_gradient
					&& let GradientDragTarget::Midpoint(index) = selected_gradient.dragging
				{
					selected_gradient.gradient.stops.midpoint[index] = 0.5;
					selected_gradient.render_gradient(responses);
					responses.add(PropertiesPanelMessage::Refresh);
				}
				self
			}
			(state, GradientToolMessage::DeleteStop) => {
				let ready_default = GradientToolFsmState::Ready {
					hovering: GradientHoverTarget::None,
					selected: GradientSelectedTarget::None,
				};

				let Some(selected_gradient) = &mut tool_data.selected_gradient else {
					return ready_default;
				};

				// Skip if invalid gradient
				if selected_gradient.gradient.stops.len() < 2 {
					return ready_default;
				}

				// If we're in the middle of a drag, abort it first and revert to the initial gradient
				if matches!(state, GradientToolFsmState::Drawing { .. }) {
					selected_gradient.gradient = selected_gradient.initial_gradient.clone();
					selected_gradient.render_gradient(responses);
					responses.add(DocumentMessage::AbortTransaction);
					tool_data.snap_manager.cleanup(responses);
				}

				responses.add(DocumentMessage::StartTransaction);

				// Remove the selected point
				match selected_gradient.dragging {
					GradientDragTarget::Start => {
						// Only delete if there's a real color stop at position ~0 (not the endpoint of the line which isn't itself a color stop)
						if selected_gradient.gradient.stops.position.first().is_some_and(|&p| p.abs() < f64::EPSILON * 1000.) {
							selected_gradient.gradient.stops.remove(0);
						} else {
							responses.add(DocumentMessage::AbortTransaction);
							return ready_default;
						}
					}
					GradientDragTarget::End => {
						// Only delete if there's a real color stop at position ~1 (not the endpoint of the line which isn't itself a color stop)
						if selected_gradient.gradient.stops.position.last().is_some_and(|&p| (1. - p).abs() < f64::EPSILON * 1000.) {
							let _ = selected_gradient.gradient.stops.pop();
						} else {
							responses.add(DocumentMessage::AbortTransaction);
							return ready_default;
						}
					}
					GradientDragTarget::New => {
						responses.add(DocumentMessage::AbortTransaction);
						return ready_default;
					}
					GradientDragTarget::Stop(index) => {
						selected_gradient.gradient.stops.remove(index);
					}
					GradientDragTarget::Midpoint(index) => {
						selected_gradient.gradient.stops.midpoint[index] = 0.5;
						selected_gradient.render_gradient(responses);

						responses.add(DocumentMessage::CommitTransaction);
						responses.add(PropertiesPanelMessage::Refresh);

						return ready_default;
					}
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
					return ready_default;
				}

				// Find the minimum and maximum positions
				let min_position = selected_gradient.gradient.stops.position.iter().copied().reduce(f64::min).expect("No min");
				let max_position = selected_gradient.gradient.stops.position.iter().copied().reduce(f64::max).expect("No max");

				// Recompute the start and end position of the gradient (in viewport transform)
				if let Some(layer) = selected_gradient.layer {
					selected_gradient.transform = gradient_space_transform(layer, document);
				}
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

				ready_default
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
							selected_gradient.dragging = GradientDragTarget::Stop(index);

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
				let document_to_viewport = document.metadata().document_to_viewport;

				let mut mouse = input.mouse.position;

				let snap_data = SnapData::new(document, input, viewport);
				let point = SnapCandidatePoint::gradient_handle(document_to_viewport.inverse().transform_point2(mouse));
				let snapped = tool_data.snap_manager.free_snap(&snap_data, &point, SnapTypeConfiguration::default());

				if snapped.is_snapped() {
					mouse = document_to_viewport.transform_point2(snapped.snapped_point_document);
				}

				tool_data.drag_start = document_to_viewport.inverse().transform_point2(mouse);
				tool_data.auto_pan_shift = DVec2::ZERO;
				let tolerance = (MANIPULATOR_GROUP_MARKER_SIZE * 2.).powi(2);

				let mut drag_hint: Option<GradientDragHintState> = None;
				let mut transaction_started = false;
				for layer in document.network_interface.selected_nodes().selected_visible_layers(&document.network_interface) {
					let Some(gradient) = get_gradient(layer, &document.network_interface) else { continue };
					let transform = gradient_space_transform(layer, document);

					// Check for dragging a midpoint diamond
					if drag_hint.is_none() {
						let (start, end) = (transform.transform_point2(gradient.start), transform.transform_point2(gradient.end));
						let line_length = start.distance(end);
						let midpoint_tolerance = GRADIENT_MIDPOINT_DIAMOND_RADIUS.powi(2);
						for i in 0..gradient.stops.position.len().saturating_sub(1) {
							let left = gradient.stops.position[i];
							let right = gradient.stops.position[i + 1];

							if midpoint_hidden_by_proximity(left, right, line_length) {
								continue;
							}

							let midpoint_pos = left + gradient.stops.midpoint[i] * (right - left);
							let midpoint_viewport = start.lerp(end, midpoint_pos);

							if midpoint_viewport.distance_squared(mouse) < midpoint_tolerance {
								let resettable = midpoint_is_resettable(gradient.stops.midpoint[i]);
								drag_hint = Some(GradientDragHintState::Midpoint { resettable });

								tool_data.selected_gradient = Some(SelectedGradient {
									layer: Some(layer),
									transform,
									gradient: gradient.clone(),
									dragging: GradientDragTarget::Midpoint(i),
									initial_gradient: gradient.clone(),
								});

								break;
							}
						}
					}

					// Check for dragging the closest stop to the mouse pointer
					if drag_hint.is_none() {
						let mut best: Option<(f64, usize)> = None;
						for (index, stop) in gradient.stops.iter().enumerate() {
							let pos = transform.transform_point2(gradient.start.lerp(gradient.end, stop.position));
							let dist_sq = pos.distance_squared(mouse);
							if dist_sq < tolerance && best.as_ref().is_none_or(|&(best_dist, _)| dist_sq < best_dist) {
								best = Some((dist_sq, index));
							}
						}
						if let Some((_, index)) = best {
							let stop_position = gradient.stops.position[index];
							// Stops at position 0 or 1 are locked endpoints: dragging moves the
							// gradient line endpoint geometry (start/end) instead of stop position
							let drag_target = if stop_position.abs() < f64::EPSILON * 1000. {
								GradientDragTarget::Start
							} else if (1. - stop_position).abs() < f64::EPSILON * 1000. {
								GradientDragTarget::End
							} else {
								GradientDragTarget::Stop(index)
							};

							drag_hint = Some(match drag_target {
								GradientDragTarget::Start | GradientDragTarget::End => GradientDragHintState::EndStop,
								_ => GradientDragHintState::Stop,
							});

							tool_data.selected_gradient = Some(SelectedGradient {
								layer: Some(layer),
								transform,
								gradient: gradient.clone(),
								dragging: drag_target,
								initial_gradient: gradient.clone(),
							});
						}
					}

					// Check dragging start or end handle
					if drag_hint.is_none() {
						for (pos, dragging_target) in [(gradient.start, GradientDragTarget::Start), (gradient.end, GradientDragTarget::End)] {
							let pos = transform.transform_point2(pos);
							if pos.distance_squared(mouse) < tolerance {
								drag_hint = Some(GradientDragHintState::Endpoint);
								tool_data.selected_gradient = Some(SelectedGradient {
									layer: Some(layer),
									transform,
									gradient: gradient.clone(),
									dragging: dragging_target,
									initial_gradient: gradient.clone(),
								})
							}
						}
					}

					// Insert stop if clicking on line
					if drag_hint.is_none() {
						let (start, end) = (transform.transform_point2(gradient.start), transform.transform_point2(gradient.end));
						let distance = (end - start).angle_to(mouse - start).sin() * (mouse - start).length();
						let projection = ((end - start).angle_to(mouse - start)).cos() * start.distance(mouse) / start.distance(end);

						if distance.abs() < SEGMENT_INSERTION_DISTANCE && (0. ..=1.).contains(&projection) {
							let mut new_gradient = gradient.clone();
							if let Some(index) = new_gradient.insert_stop(mouse, transform) {
								responses.add(DocumentMessage::StartTransaction);
								transaction_started = true;

								let mut selected_gradient = SelectedGradient::new(new_gradient, layer, document);
								selected_gradient.dragging = GradientDragTarget::Stop(index);
								// No offset when inserting a new stop, it should be exactly under the mouse
								selected_gradient.render_gradient(responses);
								tool_data.selected_gradient = Some(selected_gradient);
								drag_hint = Some(GradientDragHintState::Stop);
							}
						}
					}
				}

				// Initialize `gradient_angle` from the existing gradient so Ctrl (lock angle) works from the first mouse move
				if let Some(selected_gradient) = &tool_data.selected_gradient {
					let (vp_start, vp_end) = (
						selected_gradient.transform.transform_point2(selected_gradient.gradient.start),
						selected_gradient.transform.transform_point2(selected_gradient.gradient.end),
					);
					let delta = match selected_gradient.dragging {
						// When dragging End, the fixed point is start and the mouse begins at end
						GradientDragTarget::End => vp_start - vp_end,
						// When dragging Start, the fixed point is end and the mouse begins at start
						GradientDragTarget::Start => vp_end - vp_start,
						_ => vp_start - vp_end,
					};
					tool_data.gradient_angle = -delta.angle_to(DVec2::X);
				}

				let gradient_state = if let Some(hint) = drag_hint {
					GradientToolFsmState::Drawing { drag_hint: hint }
				} else {
					let document_mouse = document.metadata().document_to_viewport.inverse().transform_point2(mouse);
					let selected_layer = document.click_based_on_position(document_mouse);

					// Apply the gradient to the selected layer
					if let Some(layer) = selected_layer {
						// Add check for raster layer
						if NodeGraphLayer::is_raster_layer(layer, &mut document.network_interface) {
							return GradientToolFsmState::Ready {
								hovering: GradientHoverTarget::None,
								selected: GradientSelectedTarget::None,
							};
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

						GradientToolFsmState::Drawing {
							drag_hint: GradientDragHintState::NewGradient,
						}
					} else {
						GradientToolFsmState::Ready {
							hovering: GradientHoverTarget::None,
							selected: GradientSelectedTarget::None,
						}
					}
				};

				if matches!(gradient_state, GradientToolFsmState::Drawing { .. }) && !transaction_started {
					responses.add(DocumentMessage::StartTransaction);
				}

				responses.add(OverlaysMessage::Draw);

				gradient_state
			}
			(GradientToolFsmState::Drawing { drag_hint }, GradientToolMessage::PointerMove { constrain_axis, lock_angle }) => {
				if let Some(selected_gradient) = &mut tool_data.selected_gradient {
					let mouse = input.mouse.position;
					let snap_data = SnapData::new(document, input, viewport);

					// Recompute the gradient-to-viewport transform fresh each frame so zoom/pan mid-drag works correctly
					if let Some(layer) = selected_gradient.layer {
						selected_gradient.transform = gradient_space_transform(layer, document);
						selected_gradient.transform.translation += tool_data.auto_pan_shift;
					}

					// Convert drag_start from document space to effective viewport space
					let d2v = document.metadata().document_to_viewport;
					let drag_start_viewport = d2v.transform_point2(tool_data.drag_start) + tool_data.auto_pan_shift;
					tool_data.auto_pan_shift = DVec2::ZERO;

					selected_gradient.update_gradient(
						mouse,
						responses,
						input.keyboard.get(constrain_axis as usize),
						input.keyboard.get(lock_angle as usize),
						selected_gradient.gradient.gradient_type,
						drag_start_viewport,
						snap_data,
						&mut tool_data.snap_manager,
						&mut tool_data.gradient_angle,
					);
				}

				// Auto-panning
				let messages = [
					GradientToolMessage::PointerOutsideViewport { constrain_axis, lock_angle }.into(),
					GradientToolMessage::PointerMove { constrain_axis, lock_angle }.into(),
				];
				tool_data.auto_panning.setup_by_mouse_position(input, viewport, &messages, responses);

				responses.add(OverlaysMessage::Draw);

				GradientToolFsmState::Drawing { drag_hint }
			}
			(GradientToolFsmState::Drawing { drag_hint }, GradientToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panning
				if let Some(shift) = tool_data.auto_panning.shift_viewport(input, viewport, responses) {
					tool_data.auto_pan_shift += shift;
				}

				GradientToolFsmState::Drawing { drag_hint }
			}
			(state, GradientToolMessage::PointerOutsideViewport { constrain_axis, lock_angle }) => {
				// Auto-panning
				let messages = [
					GradientToolMessage::PointerOutsideViewport { constrain_axis, lock_angle }.into(),
					GradientToolMessage::PointerMove { constrain_axis, lock_angle }.into(),
				];
				tool_data.auto_panning.stop(&messages, responses);

				state
			}
			(GradientToolFsmState::Drawing { .. }, GradientToolMessage::PointerUp) => {
				responses.add(DocumentMessage::EndTransaction);
				tool_data.snap_manager.cleanup(responses);

				// Clear the selection if we were dragging an endpoint of the gradient which isn't a stop
				if tool_data.selected_gradient.as_ref().is_some_and(|s| match s.dragging {
					GradientDragTarget::Start => !s.gradient.stops.position.first().is_some_and(|&p| p.abs() < f64::EPSILON * 1000.),
					GradientDragTarget::End => !s.gradient.stops.position.last().is_some_and(|&p| (1. - p).abs() < f64::EPSILON * 1000.),
					_ => false,
				}) {
					tool_data.selected_gradient = None;
				}

				let selected = compute_selected_target(tool_data);
				GradientToolFsmState::Ready {
					hovering: GradientHoverTarget::None,
					selected,
				}
			}
			(GradientToolFsmState::Ready { .. }, GradientToolMessage::PointerMove { .. }) => {
				let mouse = input.mouse.position;
				let hovering = detect_hover_target(mouse, document);
				let selected = compute_selected_target(tool_data);

				let snap_data = SnapData::new(document, input, viewport);
				tool_data.snap_manager.preview_draw_gradient(&snap_data, mouse);

				responses.add(OverlaysMessage::Draw);
				GradientToolFsmState::Ready { hovering, selected }
			}

			(GradientToolFsmState::Drawing { .. }, GradientToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);
				tool_data.snap_manager.cleanup(responses);
				tool_data.selected_gradient = None;
				responses.add(OverlaysMessage::Draw);

				GradientToolFsmState::Ready {
					hovering: GradientHoverTarget::None,
					selected: GradientSelectedTarget::None,
				}
			}
			(_, GradientToolMessage::Abort) => GradientToolFsmState::Ready {
				hovering: GradientHoverTarget::None,
				selected: GradientSelectedTarget::None,
			},
			_ => self,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			GradientToolFsmState::Ready { hovering, selected } => {
				let mut groups = Vec::new();

				// Primary hints based on hover target
				match hovering {
					GradientHoverTarget::None => {
						groups.push(HintGroup(vec![
							HintInfo::mouse(MouseMotion::LmbDrag, "Draw Gradient"),
							HintInfo::keys([Key::Shift], "15° Increments").prepend_plus(),
							HintInfo::keys([Key::Control], "Lock Angle").prepend_plus(),
						]));
					}
					GradientHoverTarget::InsertionPoint => {
						groups.push(HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Insert Color Stop")]));
					}
					GradientHoverTarget::Stop => {
						groups.push(HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Move Color Stop")]));
					}
					GradientHoverTarget::Endpoint => {
						groups.push(HintGroup(vec![
							HintInfo::mouse(MouseMotion::LmbDrag, "Move Gradient End"),
							HintInfo::keys([Key::Shift], "15° Increments").prepend_plus(),
							HintInfo::keys([Key::Control], "Lock Angle").prepend_plus(),
						]));
					}
					GradientHoverTarget::Midpoint { resettable } => {
						groups.push(HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Move Midpoint")]));
						if *resettable {
							groups.push(HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDouble, "Reset Midpoint")]));
						}
					}
				}

				// Delete/reset hint based on selection
				match selected {
					GradientSelectedTarget::Stop => {
						groups.push(HintGroup(vec![HintInfo::keys([Key::Backspace], "Delete Color Stop")]));
					}
					GradientSelectedTarget::Midpoint { resettable: true } => {
						groups.push(HintGroup(vec![HintInfo::keys([Key::Backspace], "Reset Midpoint")]));
					}
					_ => {}
				}

				HintData(groups)
			}
			GradientToolFsmState::Drawing { drag_hint } => {
				let mut groups = Vec::new();

				// Abort hints
				groups.push(HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]));

				// Angle constraint hint (only for endpoint/end color stop/new gradient dragging)
				if matches!(drag_hint, GradientDragHintState::NewGradient | GradientDragHintState::Endpoint | GradientDragHintState::EndStop) {
					groups.push(HintGroup(vec![HintInfo::keys([Key::Shift], "15° Increments"), HintInfo::keys([Key::Control], "Lock Angle")]));
				}

				// Delete/reset hint while dragging
				match drag_hint {
					GradientDragHintState::EndStop | GradientDragHintState::Stop => {
						groups.push(HintGroup(vec![HintInfo::keys([Key::Backspace], "Delete Color Stop")]));
					}
					GradientDragHintState::Midpoint { resettable: true } => {
						groups.push(HintGroup(vec![HintInfo::keys([Key::Backspace], "Reset Midpoint")]));
					}
					_ => {}
				}

				HintData(groups)
			}
		};

		hint_data.send_layout(responses);
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}

fn detect_hover_target(mouse: DVec2, document: &DocumentMessageHandler) -> GradientHoverTarget {
	let stop_tolerance = (MANIPULATOR_GROUP_MARKER_SIZE * 2.).powi(2);
	let midpoint_tolerance = GRADIENT_MIDPOINT_DIAMOND_RADIUS.powi(2);

	for layer in document.network_interface.selected_nodes().selected_visible_layers(&document.network_interface) {
		let Some(gradient) = get_gradient(layer, &document.network_interface) else { continue };
		let transform = gradient_space_transform(layer, document);
		let (start, end) = (transform.transform_point2(gradient.start), transform.transform_point2(gradient.end));
		let line_length = start.distance(end);

		// Check midpoint diamonds first (smaller hit area, higher priority)
		for i in 0..gradient.stops.position.len().saturating_sub(1) {
			let left = gradient.stops.position[i];
			let right = gradient.stops.position[i + 1];
			if midpoint_hidden_by_proximity(left, right, line_length) {
				continue;
			}

			let midpoint_position = left + gradient.stops.midpoint[i] * (right - left);
			let midpoint_viewport = start.lerp(end, midpoint_position);

			if midpoint_viewport.distance_squared(mouse) < midpoint_tolerance {
				let resettable = midpoint_is_resettable(gradient.stops.midpoint[i]);
				return GradientHoverTarget::Midpoint { resettable };
			}
		}

		// Check stops
		for stop in gradient.stops.iter() {
			let pos = transform.transform_point2(gradient.start.lerp(gradient.end, stop.position));
			if pos.distance_squared(mouse) < stop_tolerance {
				return if stop.position.abs() < f64::EPSILON * 1000. || (1. - stop.position).abs() < f64::EPSILON * 1000. {
					GradientHoverTarget::Endpoint
				} else {
					GradientHoverTarget::Stop
				};
			}
		}

		// Check start/end handles (pure endpoints without stops)
		for endpoint_position in [gradient.start, gradient.end] {
			let endpoint_position = transform.transform_point2(endpoint_position);
			if endpoint_position.distance_squared(mouse) < stop_tolerance {
				return GradientHoverTarget::Endpoint;
			}
		}

		// Check insertion point on line
		if calculate_insertion(start, end, &gradient.stops, mouse).is_some() {
			return GradientHoverTarget::InsertionPoint;
		}
	}

	GradientHoverTarget::None
}

fn compute_selected_target(tool_data: &GradientToolData) -> GradientSelectedTarget {
	let Some(selected_gradient) = &tool_data.selected_gradient else {
		return GradientSelectedTarget::None;
	};

	match selected_gradient.dragging {
		GradientDragTarget::Stop(_) | GradientDragTarget::Start | GradientDragTarget::End => GradientSelectedTarget::Stop,
		GradientDragTarget::Midpoint(i) => {
			let resettable = selected_gradient.gradient.stops.midpoint.get(i).is_some_and(|&midpoint_value| midpoint_is_resettable(midpoint_value));
			GradientSelectedTarget::Midpoint { resettable }
		}
		GradientDragTarget::New => GradientSelectedTarget::None,
	}
}

fn apply_gradient_update(
	data: &mut GradientToolData,
	context: &mut ToolActionMessageContext,
	responses: &mut VecDeque<Message>,
	condition: impl Fn(&Gradient) -> bool,
	update: impl Fn(&mut Gradient),
) {
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
			&& condition(&gradient)
		{
			if !transaction_started {
				responses.add(DocumentMessage::StartTransaction);
				transaction_started = true;
			}
			update(&mut gradient);
			responses.add(GraphOperationMessage::FillSet {
				layer,
				fill: Fill::Gradient(gradient),
			});
		}
	}

	if transaction_started {
		responses.add(DocumentMessage::AddTransaction);
	}
	if let Some(selected_gradient) = &mut data.selected_gradient
		&& let Some(layer) = selected_gradient.layer
		&& !NodeGraphLayer::is_raster_layer(layer, &mut context.document.network_interface)
	{
		update(&mut selected_gradient.gradient);
	}
	responses.add(PropertiesPanelMessage::Refresh);
	data.has_selected_gradient = has_gradient_on_selected_layers(context.document);
	responses.add(ToolMessage::RefreshToolOptions);
}

fn has_gradient_on_selected_layers(document: &DocumentMessageHandler) -> bool {
	document
		.network_interface
		.selected_nodes()
		.selected_visible_layers(&document.network_interface)
		.any(|layer| get_gradient(layer, &document.network_interface).is_some())
}

#[inline(always)]
fn midpoint_is_resettable(value: f64) -> bool {
	(value - 0.5).abs() >= f64::EPSILON * 1000.
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StopId {
	Start,
	End,
	Middle(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum GradientHoverTarget {
	#[default]
	None,
	InsertionPoint,
	Stop,
	Endpoint,
	Midpoint {
		resettable: bool,
	},
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum GradientSelectedTarget {
	#[default]
	None,
	Stop,
	Midpoint {
		resettable: bool,
	},
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum GradientDragHintState {
	#[default]
	NewGradient,
	Endpoint,
	EndStop,
	Stop,
	Midpoint {
		resettable: bool,
	},
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
		editor.move_mouse(25., 0., ModifierKeys::empty(), MouseKeys::empty()).await;
		editor.left_mousedown(25., 0., ModifierKeys::empty()).await;
		editor.left_mouseup(25., 0., ModifierKeys::empty()).await;

		// Check that a new stop has been added
		let (updated_gradient, _) = get_gradient(&mut editor).await;
		assert_eq!(updated_gradient.stops.len(), 3, "Expected 3 stops, found {}", updated_gradient.stops.len());

		let positions: Vec<f64> = updated_gradient.stops.iter().map(|stop| stop.position).collect();
		assert!(
			positions.iter().any(|pos| (pos - 0.25).abs() < 0.1),
			"Expected to find a stop near position 0.25, but found: {positions:?}"
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

		// Add a middle stop at 25%
		editor.move_mouse(25., 0., ModifierKeys::empty(), MouseKeys::empty()).await;
		editor.left_mousedown(25., 0., ModifierKeys::empty()).await;
		editor.left_mouseup(25., 0., ModifierKeys::empty()).await;

		let (initial_gradient, _) = get_gradient(&mut editor).await;
		assert_eq!(initial_gradient.stops.len(), 3, "Expected 3 stops, found {}", initial_gradient.stops.len());

		// Verify initial stop positions and colors
		let mut stops = initial_gradient.stops.clone();
		stops.sort();

		let positions: Vec<f64> = stops.iter().map(|stop| stop.position).collect();
		assert_stops_at_positions(&positions, &[0., 0.25, 1.], 0.1);

		let middle_color = stops.color[1].to_rgba8_srgb();

		// Simulate dragging the middle stop to position 0.8
		let click_position = DVec2::new(25., 0.);
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

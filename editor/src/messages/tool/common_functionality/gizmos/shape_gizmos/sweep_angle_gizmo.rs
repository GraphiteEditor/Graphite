use crate::consts::{ARC_SNAP_THRESHOLD, GIZMO_HIDE_THRESHOLD};
use crate::messages::message::Message;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::prelude::DocumentMessageHandler;
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::shapes::shape_utility::{arc_end_points, calculate_arc_text_transform, extract_arc_parameters, format_rounded};
use crate::messages::tool::tool_messages::tool_prelude::*;
use glam::DVec2;
use graph_craft::document::value::TaggedValue;
use graph_craft::document::{NodeId, NodeInput};
use std::collections::VecDeque;
use std::f64::consts::FRAC_PI_4;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum SweepAngleGizmoState {
	#[default]
	Inactive,
	Hover,
	Dragging,
	Snapped,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum EndpointType {
	#[default]
	None,
	Start,
	End,
}

#[derive(Clone, Debug, Default)]
pub struct SweepAngleGizmo {
	pub layer: Option<LayerNodeIdentifier>,
	endpoint: EndpointType,
	initial_start_angle: f64,
	initial_sweep_angle: f64,
	position_before_rotation: DVec2,
	previous_mouse_position: DVec2,
	total_angle_delta: f64,
	snap_angles: Vec<f64>,
	handle_state: SweepAngleGizmoState,
}

impl SweepAngleGizmo {
	pub fn hovered(&self) -> bool {
		self.handle_state == SweepAngleGizmoState::Hover
	}

	pub fn update_state(&mut self, state: SweepAngleGizmoState) {
		self.handle_state = state;
	}

	pub fn is_dragging_or_snapped(&self) -> bool {
		self.handle_state == SweepAngleGizmoState::Dragging || self.handle_state == SweepAngleGizmoState::Snapped
	}

	pub fn handle_actions(&mut self, layer: LayerNodeIdentifier, document: &DocumentMessageHandler, mouse_position: DVec2) {
		if self.handle_state == SweepAngleGizmoState::Inactive {
			let Some((start, end)) = arc_end_points(Some(layer), document) else { return };
			let Some((_, start_angle, sweep_angle, _)) = extract_arc_parameters(Some(layer), document) else {
				return;
			};

			let center = document.metadata().transform_to_viewport(layer).transform_point2(DVec2::ZERO);

			if center.distance(start) < GIZMO_HIDE_THRESHOLD {
				return;
			}

			let (close_to_gizmo, endpoint_type) = if mouse_position.distance(start) < 5. {
				(true, EndpointType::Start)
			} else if mouse_position.distance(end) < 5. {
				(true, EndpointType::End)
			} else {
				(false, EndpointType::None)
			};

			if close_to_gizmo {
				self.layer = Some(layer);
				self.initial_start_angle = start_angle;
				self.initial_sweep_angle = sweep_angle;
				self.previous_mouse_position = mouse_position;
				self.total_angle_delta = 0.;
				self.position_before_rotation = if endpoint_type == EndpointType::End { end } else { start };
				self.endpoint = endpoint_type;
				self.snap_angles = Self::calculate_snap_angles();

				self.update_state(SweepAngleGizmoState::Hover);
			}
		}
	}

	pub fn overlays(
		&self,
		selected_arc_layer: Option<LayerNodeIdentifier>,
		document: &DocumentMessageHandler,
		_input: &InputPreprocessorMessageHandler,
		_mouse_position: DVec2,
		overlay_context: &mut OverlayContext,
	) {
		let tilt_offset = document.document_ptz.unmodified_tilt();

		match self.handle_state {
			SweepAngleGizmoState::Inactive => {
				let Some((point1, point2)) = arc_end_points(selected_arc_layer, document) else { return };
				overlay_context.manipulator_handle(point1, false, None);
				overlay_context.manipulator_handle(point2, false, None);
			}
			SweepAngleGizmoState::Hover => {
				// Highlight the currently hovered endpoint only
				let Some((point1, point2)) = arc_end_points(self.layer, document) else { return };

				let (point, other_point) = if self.endpoint == EndpointType::Start { (point1, point2) } else { (point2, point1) };
				overlay_context.manipulator_handle(point, true, None);
				overlay_context.manipulator_handle(other_point, false, None);
			}
			SweepAngleGizmoState::Dragging => {
				// Show snapping guides and angle arc while dragging
				let Some(layer) = self.layer else { return };
				let Some((current_start, current_end)) = arc_end_points(self.layer, document) else { return };
				let viewport = document.metadata().transform_to_viewport(layer);

				// Depending on which endpoint is being dragged, draw guides relative to the static point
				let (point, other_point) = if self.endpoint == EndpointType::End {
					(current_end, current_start)
				} else {
					(current_start, current_end)
				};

				// Draw the dashed line from center to drag start position
				overlay_context.dashed_line(self.position_before_rotation, viewport.transform_point2(DVec2::ZERO), None, None, Some(5.), Some(5.), Some(0.5));

				overlay_context.manipulator_handle(other_point, false, None);

				// Draw the angle, text and the bold line
				self.dragging_snapping_overlays(self.position_before_rotation, point, tilt_offset, viewport, overlay_context);
			}
			SweepAngleGizmoState::Snapped => {
				// When snapping is active, draw snapping arcs and angular guidelines
				let Some((start, end)) = arc_end_points(self.layer, document) else { return };
				let Some(layer) = self.layer else { return };
				let viewport = document.metadata().transform_to_viewport(layer);
				let center = viewport.transform_point2(DVec2::ZERO);

				// Draw snapping arc and angle overlays between the two points
				let (a, b) = if self.endpoint == EndpointType::Start { (end, start) } else { (start, end) };
				self.dragging_snapping_overlays(a, b, tilt_offset, viewport, overlay_context);

				// Draw lines from endpoints to the arc center
				overlay_context.line(start, center, None, Some(2.));
				overlay_context.line(end, center, None, Some(2.));

				// Draw the line from drag start to arc center
				overlay_context.dashed_line(self.position_before_rotation, center, None, None, Some(5.), Some(5.), Some(0.5));
			}
		}
	}

	/// Draws the visual overlay during arc handle dragging or snapping interactions.
	/// This includes the dynamic arc sweep, angle label, and visual guides centered around the arc's origin.
	pub fn dragging_snapping_overlays(&self, initial_point: DVec2, final_point: DVec2, tilt_offset: f64, viewport: DAffine2, overlay_context: &mut OverlayContext) {
		let center = viewport.transform_point2(DVec2::ZERO);
		let initial_vector = initial_point - center;
		let final_vector = final_point - center;
		let offset_angle = initial_vector.to_angle() + tilt_offset;

		let bold_radius = final_point.distance(center);

		let angle = initial_vector.angle_to(final_vector).to_degrees();
		let display_angle = viewport
			.inverse()
			.transform_point2(final_point)
			.angle_to(viewport.inverse().transform_point2(initial_point))
			.to_degrees();

		let text = format!("{}°", format_rounded(display_angle, 2));
		let text_texture_width = overlay_context.get_width(&text) / 2.;

		let transform = calculate_arc_text_transform(angle, offset_angle, center, text_texture_width);

		overlay_context.arc_sweep_angle(offset_angle, angle, final_point, bold_radius, center, &text, transform);
	}

	pub fn update_arc(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		let Some(layer) = self.layer else { return };
		let Some((_, current_start_angle, current_sweep_angle, _)) = extract_arc_parameters(Some(layer), document) else {
			return;
		};

		let viewport = document.metadata().transform_to_viewport(layer);
		let angle_delta = viewport
			.inverse()
			.transform_point2(self.previous_mouse_position)
			.angle_to(viewport.inverse().transform_point2(input.mouse.position))
			.to_degrees();
		let angle = self.total_angle_delta + angle_delta;

		let Some(node_id) = graph_modification_utils::get_arc_id(layer, &document.network_interface) else {
			return;
		};

		self.update_state(SweepAngleGizmoState::Dragging);

		match self.endpoint {
			EndpointType::Start => {
				// Dragging start changes both start and sweep

				let sign = -angle.signum();
				let mut total = angle;

				let new_start_angle = self.initial_start_angle + total;
				let new_sweep_angle = self.initial_sweep_angle + total.abs() * sign;

				match () {
					// Clamp sweep angle to 360°
					() if new_sweep_angle > 360. => {
						let wrapped = new_sweep_angle % 360.;
						self.total_angle_delta = -wrapped;

						self.endpoint = EndpointType::End;

						self.initial_sweep_angle = 360.;
						self.initial_start_angle = current_start_angle;
						self.update_state(SweepAngleGizmoState::Snapped);

						self.apply_arc_update(node_id, self.initial_start_angle, self.initial_sweep_angle - wrapped, input, responses);
					}
					() if new_sweep_angle < 0. => {
						let rest_angle = angle_delta + new_sweep_angle;

						self.total_angle_delta = new_sweep_angle.abs();
						self.endpoint = EndpointType::End;

						self.initial_sweep_angle = 0.;
						self.initial_start_angle = current_start_angle + rest_angle;

						self.apply_arc_update(node_id, self.initial_start_angle, new_sweep_angle.abs(), input, responses);
					}
					// Wrap start angle > 180° back into [-180°, 180°] and adjust sweep
					() if new_start_angle > 180. => {
						let overflow = new_start_angle % 180.;
						let rest_angle = angle_delta - overflow;

						// We wrap the angle back into [-180°, 180°] range by jumping from +180° to -180°
						// Example: dragging past 190° becomes -170°, and we subtract the overshoot from sweep
						// Sweep angle must shrink to maintain consistent arc
						self.total_angle_delta = rest_angle;
						self.initial_start_angle = -180.;
						self.initial_sweep_angle = current_sweep_angle - rest_angle;

						self.apply_arc_update(node_id, self.initial_start_angle + overflow, self.initial_sweep_angle - overflow, input, responses);
					}
					// Wrap start angle < -180° back into [-180°, 180°] and adjust sweep
					() if new_start_angle < -180. => {
						let underflow = new_start_angle % 180.;
						let rest_angle = angle_delta - underflow;

						// We wrap the angle back into [-180°, 180°] by jumping from -190° to +170°
						// Sweep must grow to reflect continued clockwise drag past -180°
						// Start angle flips from -190° to +170°, and sweep increases accordingly
						self.total_angle_delta = underflow;
						self.initial_start_angle = 180.;
						self.initial_sweep_angle = current_sweep_angle + rest_angle.abs();

						self.apply_arc_update(node_id, self.initial_start_angle + underflow, self.initial_sweep_angle + underflow.abs(), input, responses);
					}
					_ => {
						if let Some(snapped_delta) = self.check_snapping(self.initial_sweep_angle + total.abs() * sign) {
							total += snapped_delta;
							self.update_state(SweepAngleGizmoState::Snapped);
						}

						self.total_angle_delta = angle;
						self.apply_arc_update(node_id, self.initial_start_angle + total, self.initial_sweep_angle + total.abs() * sign, input, responses);
					}
				}
			}
			EndpointType::End => {
				// Dragging the end only changes sweep angle

				let mut total = angle;
				let new_sweep_angle = self.initial_sweep_angle + angle;

				match () {
					// Clamp sweep angle below 0°, switch to start
					() if new_sweep_angle < 0. => {
						let delta = angle_delta - current_sweep_angle;
						let sign = -delta.signum();

						self.initial_sweep_angle = 0.;
						self.total_angle_delta = delta;
						self.endpoint = EndpointType::Start;

						self.apply_arc_update(node_id, self.initial_start_angle + delta, self.initial_sweep_angle + delta.abs() * sign, input, responses);
					}
					// Clamp sweep angle above 360°, switch to start
					() if new_sweep_angle > 360. => {
						let delta = angle_delta - (360. - new_sweep_angle);
						let sign = -delta.signum();

						self.total_angle_delta = angle_delta - (360. - new_sweep_angle);
						self.initial_sweep_angle = 360.;
						self.endpoint = EndpointType::Start;
						self.update_state(SweepAngleGizmoState::Snapped);

						self.apply_arc_update(node_id, self.initial_start_angle + angle_delta, self.initial_sweep_angle + angle_delta.abs() * sign, input, responses);
					}
					_ => {
						if let Some(snapped_delta) = self.check_snapping(self.initial_sweep_angle + angle) {
							total += snapped_delta;
							self.update_state(SweepAngleGizmoState::Snapped);
						}

						self.total_angle_delta = angle;
						self.apply_arc_update(node_id, self.initial_start_angle, self.initial_sweep_angle + total, input, responses);
					}
				}
			}
			EndpointType::None => {}
		}
	}

	/// Applies the updated start and sweep angles to the arc.
	fn apply_arc_update(&mut self, node_id: NodeId, start_angle: f64, sweep_angle: f64, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		self.snap_angles = Self::calculate_snap_angles();

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, 2),
			input: NodeInput::value(TaggedValue::F64(start_angle), false),
		});
		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, 3),
			input: NodeInput::value(TaggedValue::F64(sweep_angle), false),
		});

		self.previous_mouse_position = input.mouse.position;
		responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	pub fn check_snapping(&self, new_sweep_angle: f64) -> Option<f64> {
		self.snap_angles.iter().find(|angle| (**angle - new_sweep_angle).abs() <= ARC_SNAP_THRESHOLD).map(|angle| {
			let delta = angle - new_sweep_angle;
			if self.endpoint == EndpointType::End { delta } else { -delta }
		})
	}

	pub fn calculate_snap_angles() -> Vec<f64> {
		let mut snap_points = Vec::new();

		for i in 0..=8 {
			let snap_point = i as f64 * FRAC_PI_4;
			snap_points.push(snap_point.to_degrees());
		}

		snap_points
	}

	pub fn cleanup(&mut self) {
		self.layer = None;
		self.endpoint = EndpointType::None;
		self.handle_state = SweepAngleGizmoState::Inactive;
	}
}

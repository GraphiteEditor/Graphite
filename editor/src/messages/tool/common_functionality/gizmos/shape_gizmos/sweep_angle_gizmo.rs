use crate::consts::{ARC_SNAP_THRESHOLD, COLOR_OVERLAY_RED, GIZMO_HIDE_THRESHOLD};
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::shapes::shape_utility::{
	arc_end_points, arc_end_points_ignore_layer, calculate_arc_text_transform, calculate_display_angle, extract_arc_parameters, wrap_to_tau,
};
use crate::messages::tool::tool_messages::tool_prelude::*;
use crate::messages::{
	frontend::utility_types::MouseCursorIcon,
	message::Message,
	prelude::{DocumentMessageHandler, FrontendMessage},
};
use glam::DVec2;
use graph_craft::document::NodeId;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
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
	initial_start_point: DVec2,
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

	pub fn handle_actions(&mut self, layer: LayerNodeIdentifier, document: &DocumentMessageHandler, mouse_position: DVec2, responses: &mut VecDeque<Message>) {
		match self.handle_state {
			SweepAngleGizmoState::Inactive => {
				let Some((start, end)) = arc_end_points(Some(layer), document) else { return };
				let Some((_, start_angle, sweep_angle, _)) = extract_arc_parameters(Some(layer), document) else {
					return;
				};

				let center = document.metadata().transform_to_viewport(layer).transform_point2(DVec2::ZERO);

				if center.distance(start) < GIZMO_HIDE_THRESHOLD {
					return;
				}

				if mouse_position.distance(start) < 5. {
					self.layer = Some(layer);
					self.initial_start_angle = start_angle;
					self.initial_sweep_angle = sweep_angle;
					self.previous_mouse_position = mouse_position;
					self.total_angle_delta = 0.;
					self.endpoint = EndpointType::Start;
					self.snap_angles = self.calculate_snap_angles(start_angle, sweep_angle);
					self.update_state(SweepAngleGizmoState::Hover);
					responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
					return;
				}

				if mouse_position.distance(end) < 5. {
					self.layer = Some(layer);
					self.initial_start_angle = start_angle;
					self.initial_sweep_angle = sweep_angle;
					self.previous_mouse_position = mouse_position;
					self.total_angle_delta = 0.;
					self.endpoint = EndpointType::End;
					self.snap_angles = self.calculate_snap_angles(start_angle, sweep_angle);
					self.update_state(SweepAngleGizmoState::Hover);
					responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });

					return;
				}
			}

			SweepAngleGizmoState::Hover => {}
			SweepAngleGizmoState::Dragging => {}
			SweepAngleGizmoState::Snapped => {}
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
		let format_rounded = |value: f64, precision: usize| format!("{:.*}", precision, value).trim_end_matches('0').trim_end_matches('.').to_string();
		let tilt_offset = document.document_ptz.unmodified_tilt();

		match self.handle_state {
			SweepAngleGizmoState::Inactive => {
				let Some((point1, point2)) = arc_end_points(selected_arc_layer, document) else { return };
				overlay_context.manipulator_handle(point1, false, Some(COLOR_OVERLAY_RED));
				overlay_context.manipulator_handle(point2, false, Some(COLOR_OVERLAY_RED));
			}
			SweepAngleGizmoState::Hover => {
				let Some((point1, point2)) = arc_end_points(self.layer, document) else { return };

				if matches!(self.endpoint, EndpointType::Start) {
					overlay_context.manipulator_handle(point1, true, Some(COLOR_OVERLAY_RED));
				} else {
					overlay_context.manipulator_handle(point2, true, Some(COLOR_OVERLAY_RED));
				}
			}
			SweepAngleGizmoState::Dragging => {
				let Some(layer) = self.layer else { return };
				let Some((start, end)) = arc_end_points(self.layer, document) else { return };

				let viewport = document.metadata().transform_to_viewport(layer);
				let center = viewport.transform_point2(DVec2::ZERO);

				let Some((radius, _, _, _)) = extract_arc_parameters(self.layer, document) else {
					return;
				};

				let Some((initial_start, initial_end)) = arc_end_points_ignore_layer(radius, self.initial_start_angle, self.initial_sweep_angle, Some(viewport)) else {
					return;
				};

				let angle = self.total_angle_delta;

				let display_angle = calculate_display_angle(angle);

				let text = format!("{}°", format_rounded(display_angle, 2));
				let text_texture_width = overlay_context.get_width(&text) / 2.;

				if self.endpoint == EndpointType::End {
					let initial_vector = initial_end - center;
					let offset_angle = initial_vector.to_angle() + tilt_offset;

					let transform = calculate_arc_text_transform(angle, offset_angle, center, text_texture_width);

					overlay_context.arc_sweep_angle(offset_angle, angle, end, radius, center, &text, transform);
				} else {
					let initial_vector = initial_start - center;
					let offset_angle = initial_vector.to_angle() + tilt_offset;

					let transform = calculate_arc_text_transform(angle, offset_angle, center, text_texture_width);

					overlay_context.arc_sweep_angle(offset_angle, angle, start, radius, center, &text, transform);
				}
			}

			SweepAngleGizmoState::Snapped => {
				let Some((current_start, current_end)) = arc_end_points(self.layer, document) else {
					return;
				};
				let Some((radius, _, _, _)) = extract_arc_parameters(self.layer, document) else { return };
				let Some(layer) = self.layer else { return };
				let viewport = document.metadata().transform_to_viewport(layer);

				let center = viewport.transform_point2(DVec2::ZERO);

				if self.endpoint == EndpointType::Start {
					let initial_vector = current_end - center;
					let final_vector = current_start - center;
					let offset_angle = initial_vector.to_angle() + tilt_offset;

					let angle = initial_vector.angle_to(final_vector).to_degrees();
					let display_angle = calculate_display_angle(angle);

					let text = format!("{}°", format_rounded(display_angle, 2));
					let text_texture_width = overlay_context.get_width(&text) / 2.;

					let transform = calculate_arc_text_transform(angle, offset_angle, center, text_texture_width);

					overlay_context.arc_sweep_angle(offset_angle, angle, current_start, radius, center, &text, transform);
				} else {
					let initial_vector = current_start - center;
					let final_vector = current_end - center;
					let offset_angle = initial_vector.to_angle() + tilt_offset;

					let angle = initial_vector.angle_to(final_vector).to_degrees();
					log::info!("angle {:?}", angle);
					let display_angle = calculate_display_angle(angle);

					let text = format!("{}°", format_rounded(display_angle, 2));
					let text_texture_width = overlay_context.get_width(&text) / 2.;

					let transform = calculate_arc_text_transform(angle, offset_angle, center, text_texture_width);

					overlay_context.arc_sweep_angle(offset_angle, angle, current_end, radius, center, &text, transform);
				}

				overlay_context.line(current_start, center, Some(COLOR_OVERLAY_RED), Some(2.0));
				overlay_context.line(current_end, center, Some(COLOR_OVERLAY_RED), Some(2.0));
			}
		}
	}

	pub fn update_arc(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		let Some(layer) = self.layer else {
			return;
		};

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

				let sign = angle.signum() * -1.;
				let mut total = angle;

				let new_start_angle = self.initial_start_angle + total;
				let new_sweep_angle = self.initial_sweep_angle + total.abs() * sign;

				// Clamp sweep angle to 360°
				if new_sweep_angle > 360. {
					let wrapped = new_sweep_angle % 360.;
					self.total_angle_delta = -wrapped;

					// Remaining drag gets passed to the end endpoint
					let rest_angle = angle_delta + wrapped;
					self.endpoint = EndpointType::End;

					self.initial_sweep_angle = 360.;
					self.initial_start_angle = current_start_angle + rest_angle;

					self.apply_arc_update(node_id, self.initial_start_angle, self.initial_sweep_angle - wrapped, input, responses);
					return;
				}

				if new_sweep_angle < 0. {
					let rest_angle = angle_delta + new_sweep_angle;

					self.total_angle_delta = new_sweep_angle.abs();
					self.endpoint = EndpointType::End;

					self.initial_sweep_angle = 0.;
					self.initial_start_angle = current_start_angle + rest_angle;

					self.apply_arc_update(node_id, self.initial_start_angle, new_sweep_angle.abs(), input, responses);
					return;
				}

				// Wrap start angle > 180° back into [-180°, 180°] and adjust sweep
				if new_start_angle > 180. {
					let overflow = new_start_angle % 180.;
					let rest_angle = angle_delta - overflow;

					// We wrap the angle back into [-180°, 180°] range by jumping from +180° to -180°
					// Example: dragging past 190° becomes -170°, and we subtract the overshoot from sweep
					// Sweep angle must shrink to maintain consistent arc
					self.total_angle_delta = rest_angle;
					self.initial_start_angle = -180.;
					self.initial_sweep_angle = current_sweep_angle - rest_angle;

					self.apply_arc_update(node_id, self.initial_start_angle + overflow, self.initial_sweep_angle - overflow, input, responses);
					return;
				}

				// Wrap start angle < -180° back into [-180°, 180°] and adjust sweep
				if new_start_angle < -180. {
					let underflow = new_start_angle % 180.;
					let rest_angle = angle_delta - underflow;

					// We wrap the angle back into [-180°, 180°] by jumping from -190° to +170°
					// Sweep must grow to reflect continued clockwise drag past -180°
					// Start angle flips from -190° to +170°, and sweep increases accordingly
					self.total_angle_delta = underflow;
					self.initial_start_angle = 180.;
					self.initial_sweep_angle = current_sweep_angle + rest_angle.abs();

					self.apply_arc_update(node_id, self.initial_start_angle + underflow, self.initial_sweep_angle + underflow.abs(), input, responses);
					return;
				}

				if let Some(snapped_delta) = self.check_snapping(self.initial_start_angle + angle, self.initial_sweep_angle + total.abs() * sign) {
					total += snapped_delta;
					self.update_state(SweepAngleGizmoState::Snapped);
				}

				self.total_angle_delta = angle;
				self.apply_arc_update(node_id, self.initial_start_angle + total, self.initial_sweep_angle + total.abs() * sign, input, responses);
			}
			EndpointType::End => {
				// Dragging the end only changes sweep angle

				let mut total = angle;
				let new_sweep_angle = self.initial_sweep_angle + angle;

				// Clamp sweep angle below 0°, switch to start
				if new_sweep_angle < 0. {
					let delta = angle_delta - current_sweep_angle;
					let sign = delta.signum() * -1.;

					self.initial_sweep_angle = 0.;
					self.total_angle_delta = delta;
					self.endpoint = EndpointType::Start;

					self.apply_arc_update(node_id, self.initial_start_angle + delta, self.initial_sweep_angle + delta.abs() * sign, input, responses);
					return;
				}

				// Clamp sweep angle above 360°, switch to start
				if new_sweep_angle > 360. {
					let delta = angle_delta - (360. - current_sweep_angle);
					let sign = delta.signum() * -1.;

					self.total_angle_delta = angle_delta;
					self.initial_sweep_angle = 360.;
					self.endpoint = EndpointType::Start;

					self.apply_arc_update(node_id, self.initial_start_angle + angle_delta, self.initial_sweep_angle + angle_delta.abs() * sign, input, responses);
					return;
				}

				if let Some(snapped_delta) = self.check_snapping(self.initial_start_angle, self.initial_sweep_angle + angle) {
					total += snapped_delta;
					self.update_state(SweepAngleGizmoState::Snapped);
				}

				self.total_angle_delta = angle;
				self.apply_arc_update(node_id, self.initial_start_angle, self.initial_sweep_angle + total, input, responses);
			}
			EndpointType::None => {}
		}
	}

	/// Applies the updated start and sweep angles to the arc.
	fn apply_arc_update(&mut self, node_id: NodeId, start_angle: f64, sweep_angle: f64, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>) {
		self.snap_angles = self.calculate_snap_angles(start_angle, sweep_angle);

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

	pub fn check_snapping(&self, new_start_angle: f64, new_sweep_angle: f64) -> Option<f64> {
		let wrapped_sweep_angle = wrap_to_tau(new_sweep_angle.to_radians()).to_degrees();
		let wrapped_start_angle = wrap_to_tau(new_start_angle.to_radians()).to_degrees();
		if self.endpoint == EndpointType::End {
			return self
				.snap_angles
				.iter()
				.find(|angle| ((**angle) - (wrapped_sweep_angle)).abs() < ARC_SNAP_THRESHOLD)
				.map(|angle| angle - wrapped_sweep_angle);
		} else {
			return self
				.snap_angles
				.iter()
				.find(|angle| ((**angle) - (wrapped_start_angle)).abs() < ARC_SNAP_THRESHOLD)
				.map(|angle| angle - wrapped_start_angle);
		}
	}

	pub fn calculate_snap_angles(&self, initial_start_angle: f64, initial_sweep_angle: f64) -> Vec<f64> {
		let mut snap_points = Vec::new();
		let sign = initial_start_angle.signum() * -1.;
		let end_angle = initial_start_angle.abs().to_radians() * sign - initial_sweep_angle.to_radians();
		let wrapped_end_angle = wrap_to_tau(-end_angle);

		if self.endpoint == EndpointType::End {
			for i in 0..8 {
				let snap_point = wrap_to_tau(i as f64 * FRAC_PI_4 + initial_start_angle);
				snap_points.push(snap_point.to_degrees());
			}
		}

		if self.endpoint == EndpointType::Start {
			for i in 0..8 {
				let snap_point = wrap_to_tau(wrapped_end_angle + i as f64 * FRAC_PI_4);
				snap_points.push(snap_point.to_degrees());
			}
		}

		snap_points
	}

	pub fn cleanup(&mut self) {
		self.layer = None;
		self.endpoint = EndpointType::None;
		self.handle_state = SweepAngleGizmoState::Inactive;
	}
}

use crate::{
	consts::{COLOR_OVERLAY_RED, POINT_RADIUS_HANDLE_SNAP_THRESHOLD},
	messages::{
		frontend::utility_types::MouseCursorIcon,
		prelude::FrontendMessage,
		tool::common_functionality::shapes::shape_utility::{calculate_polygon_vertex_position, draw_snapping_ticks, extract_polygon_parameters, polygon_outline, star_outline},
	},
};
use std::{
	collections::VecDeque,
	f64::consts::{FRAC_1_SQRT_2, FRAC_PI_4, PI, SQRT_2},
};

use crate::messages::{portfolio::document::utility_types::document_metadata::LayerNodeIdentifier, prelude::Responses};
use glam::DVec2;
use graph_craft::document::{NodeInput, value::TaggedValue};

use crate::{
	consts::GIZMO_HIDE_THRESHOLD,
	messages::{
		message::Message,
		portfolio::document::{overlays::utility_types::OverlayContext, utility_types::network_interface::InputConnector},
		prelude::{DocumentMessageHandler, InputPreprocessorMessageHandler, NodeGraphMessage},
		tool::common_functionality::{
			graph_modification_utils,
			graph_modification_utils::NodeGraphLayer,
			shapes::shape_utility::{calculate_star_vertex_position, extract_star_parameters},
		},
	},
};

#[derive(Clone, Debug, Default, PartialEq)]
pub enum PointRadiusHandleState {
	#[default]
	Inactive,
	Hover,
	Dragging,
	Snapped(usize),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PointRadiusHandle {
	pub layer: Option<LayerNodeIdentifier>,
	point: u32,
	radius_index: usize,
	snap_radii: Vec<f64>,
	initial_radius: f64,
	handle_state: PointRadiusHandleState,
}

impl PointRadiusHandle {
	pub fn cleanup(&mut self) {
		self.handle_state = PointRadiusHandleState::Inactive;
		self.snap_radii.clear();
		self.layer = None;
	}

	pub fn hovered(&self) -> bool {
		self.handle_state == PointRadiusHandleState::Hover
	}

	pub fn is_dragging_or_snapped(&self) -> bool {
		self.handle_state == PointRadiusHandleState::Dragging || matches!(self.handle_state, PointRadiusHandleState::Snapped(_))
	}

	pub fn update_state(&mut self, state: PointRadiusHandleState) {
		self.handle_state = state;
	}

	pub fn handle_actions(&mut self, layer: LayerNodeIdentifier, document: &DocumentMessageHandler, mouse_position: DVec2, responses: &mut VecDeque<Message>) {
		match &self.handle_state {
			PointRadiusHandleState::Inactive => {
				if let Some((n, radius1, radius2)) = extract_star_parameters(Some(layer), document) {
					let viewport = document.metadata().transform_to_viewport(layer);

					for i in 0..2 * n {
						let (radius, radius_index) = if i % 2 == 0 { (radius1, 2) } else { (radius2, 3) };
						let point = calculate_star_vertex_position(viewport, i as i32, n, radius1, radius2);
						let center = viewport.transform_point2(DVec2::ZERO);

						// If the user zooms out such that shape is very small hide the gizmo
						if point.distance(center) < GIZMO_HIDE_THRESHOLD {
							return;
						}

						if point.distance(mouse_position) < 5. {
							self.radius_index = radius_index;
							self.layer = Some(layer);
							self.point = i;
							self.snap_radii = Self::calculate_snap_radii(document, layer, radius_index);
							self.initial_radius = radius;
							responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
							self.update_state(PointRadiusHandleState::Hover);

							return;
						}
					}
				}

				// Draw the point handle gizmo for the Polygon shape
				if let Some((n, radius)) = extract_polygon_parameters(Some(layer), document) {
					let viewport = document.metadata().transform_to_viewport(layer);

					for i in 0..n {
						let point = calculate_polygon_vertex_position(viewport, i as i32, n, radius);
						let center = viewport.transform_point2(DVec2::ZERO);

						// If the user zooms out such that shape is very small hide the gizmo
						if point.distance(center) < GIZMO_HIDE_THRESHOLD {
							return;
						}

						if point.distance(mouse_position) < 5.0 {
							self.radius_index = 2;
							self.layer = Some(layer);
							self.point = i;
							self.snap_radii.clear();
							self.initial_radius = radius;
							self.update_state(PointRadiusHandleState::Hover);
							responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
							return;
						}
					}
				}
			}

			PointRadiusHandleState::Dragging | PointRadiusHandleState::Hover => {
				let Some(layer) = self.layer else {
					return;
				};

				let viewport = document.metadata().transform_to_viewport(layer);

				if let Some((n, radius1, radius2)) = extract_star_parameters(Some(layer), document) {
					let point = calculate_star_vertex_position(viewport, self.point as i32, n, radius1, radius2);

					if matches!(&self.handle_state, PointRadiusHandleState::Hover) {
						if (mouse_position - point).length() > 5.0 {
							self.update_state(PointRadiusHandleState::Inactive);
							self.layer = None;
							return;
						}
					}
				}

				if let Some((n, radius)) = extract_polygon_parameters(Some(layer), document) {
					let point = calculate_polygon_vertex_position(viewport, self.point as i32, n, radius);

					if matches!(&self.handle_state, PointRadiusHandleState::Hover) {
						if (mouse_position - point).length() > 5. {
							self.update_state(PointRadiusHandleState::Inactive);
							self.layer = None;
							return;
						}
					}
				}
			}
			PointRadiusHandleState::Snapped(_) => {}
		}
	}

	pub fn overlays(
		&self,
		selected_star_layer: Option<LayerNodeIdentifier>,
		document: &DocumentMessageHandler,
		input: &InputPreprocessorMessageHandler,
		mouse_position: DVec2,
		overlay_context: &mut OverlayContext,
	) {
		match &self.handle_state {
			PointRadiusHandleState::Inactive => {
				let Some(layer) = selected_star_layer else { return };

				// Draw the point handle gizmo for the star shape
				if let Some((n, radius1, radius2)) = extract_star_parameters(Some(layer), document) {
					let viewport = document.metadata().transform_to_viewport(layer);

					for i in 0..2 * n {
						let point = calculate_star_vertex_position(viewport, i as i32, n, radius1, radius2);
						let center = viewport.transform_point2(DVec2::ZERO);
						let viewport_diagonal = input.viewport_bounds.size().length();

						// If the user zooms out such that shape is very small hide the gizmo
						if point.distance(center) < GIZMO_HIDE_THRESHOLD {
							return;
						}

						if point.distance(mouse_position) < 5. {
							let Some(direction) = (point - center).try_normalize() else {
								continue;
							};

							overlay_context.manipulator_handle(point, true, None);
							let angle = ((i as f64) * PI) / (n as f64);
							overlay_context.line(center, center + direction * viewport_diagonal, None, None);

							draw_snapping_ticks(&self.snap_radii, direction, viewport, angle, overlay_context);

							return;
						}

						overlay_context.manipulator_handle(point, false, None);
					}
				}

				// Draw the point handle gizmo for the Polygon shape
				if let Some((n, radius)) = extract_polygon_parameters(Some(layer), document) {
					let viewport = document.metadata().transform_to_viewport(layer);

					for i in 0..n {
						let point = calculate_polygon_vertex_position(viewport, i as i32, n, radius);
						let center = viewport.transform_point2(DVec2::ZERO);
						let viewport_diagonal = input.viewport_bounds.size().length();

						// If the user zooms out such that shape is very small hide the gizmo
						if point.distance(center) < GIZMO_HIDE_THRESHOLD {
							return;
						}

						if point.distance(mouse_position) < 5. {
							let Some(direction) = (point - center).try_normalize() else {
								continue;
							};

							overlay_context.manipulator_handle(point, true, None);
							overlay_context.line(center, center + direction * viewport_diagonal, None, None);

							return;
						}

						overlay_context.manipulator_handle(point, false, None);
					}
				}
			}

			PointRadiusHandleState::Dragging | PointRadiusHandleState::Hover => {
				let Some(layer) = self.layer else {
					return;
				};

				let viewport = document.metadata().transform_to_viewport(layer);
				let center = viewport.transform_point2(DVec2::ZERO);
				let viewport_diagonal = input.viewport_bounds.size().length();

				if let Some((n, radius1, radius2)) = extract_star_parameters(Some(layer), document) {
					let angle = ((self.point as f64) * PI) / (n as f64);
					let point = calculate_star_vertex_position(viewport, self.point as i32, n, radius1, radius2);

					let Some(direction) = (point - center).try_normalize() else {
						return;
					};

					// Draws the ray from the center to the dragging point extending till the viewport
					overlay_context.manipulator_handle(point, true, None);
					overlay_context.line(center, center + direction * viewport_diagonal, None, None);
					star_outline(Some(layer), document, overlay_context);

					// makes the ticks for snapping

					// if dragging to make radius negative don't show the
					if (mouse_position - center).dot(direction) < 0.0 {
						return;
					}
					draw_snapping_ticks(&self.snap_radii, direction, viewport, angle, overlay_context);

					return;
				}

				if let Some((n, radius)) = extract_polygon_parameters(Some(layer), document) {
					let point = calculate_polygon_vertex_position(viewport, self.point as i32, n, radius);

					let Some(direction) = (point - center).try_normalize() else {
						return;
					};

					// Draws the ray from the center to the dragging point extending till the viewport
					overlay_context.manipulator_handle(point, true, None);
					overlay_context.line(center, center + direction * viewport_diagonal, None, None);

					polygon_outline(Some(layer), document, overlay_context);
				}
			}
			PointRadiusHandleState::Snapped(snapping_index) => {
				let Some(layer) = self.layer else {
					return;
				};

				let Some((n, radius1, radius2)) = extract_star_parameters(Some(layer), document) else {
					return;
				};

				let viewport = document.metadata().transform_to_viewport(layer);
				let center = viewport.transform_point2(DVec2::ZERO);

				match snapping_index {
					//Make a triangle with previous two points
					0 => {
						let before_outer_position = calculate_star_vertex_position(viewport, (self.point as i32) - 2, n, radius1, radius2);
						let outer_position = calculate_star_vertex_position(viewport, (self.point as i32) - 1, n, radius1, radius2);
						let point_position = calculate_star_vertex_position(viewport, self.point as i32, n, radius1, radius2);

						overlay_context.line(before_outer_position, outer_position, Some(COLOR_OVERLAY_RED), Some(3.0));
						overlay_context.line(outer_position, point_position, Some(COLOR_OVERLAY_RED), Some(3.0));

						let l1 = (before_outer_position - outer_position).length() * 0.2;
						let Some(l1_direction) = (before_outer_position - outer_position).try_normalize() else {
							return;
						};

						let Some(l2_direction) = (point_position - outer_position).try_normalize() else {
							return;
						};

						let Some(direction) = (center - outer_position).try_normalize() else {
							return;
						};

						let new_point = SQRT_2 * l1 * direction + outer_position;

						let before_outer_position = l1 * l1_direction + outer_position;
						let point_position = l1 * l2_direction + outer_position;

						overlay_context.line(before_outer_position, new_point, Some(COLOR_OVERLAY_RED), Some(3.0));
						overlay_context.line(new_point, point_position, Some(COLOR_OVERLAY_RED), Some(3.0));
					}
					1 => {
						let before_outer_position = calculate_star_vertex_position(viewport, (self.point as i32) - 1, n, radius1, radius2);

						let after_point_position = calculate_star_vertex_position(viewport, (self.point as i32) + 1, n, radius1, radius2);

						let point_position = calculate_star_vertex_position(viewport, self.point as i32, n, radius1, radius2);

						overlay_context.line(before_outer_position, point_position, Some(COLOR_OVERLAY_RED), Some(3.0));
						overlay_context.line(point_position, after_point_position, Some(COLOR_OVERLAY_RED), Some(3.0));

						let l1 = (before_outer_position - point_position).length() * 0.2;
						let Some(l1_direction) = (before_outer_position - point_position).try_normalize() else {
							return;
						};

						let Some(l2_direction) = (after_point_position - point_position).try_normalize() else {
							return;
						};

						let Some(direction) = (center - point_position).try_normalize() else {
							return;
						};

						let new_point = SQRT_2 * l1 * direction + point_position;

						let before_outer_position = l1 * l1_direction + point_position;
						let after_point_position = l1 * l2_direction + point_position;

						overlay_context.line(before_outer_position, new_point, Some(COLOR_OVERLAY_RED), Some(3.0));
						overlay_context.line(new_point, after_point_position, Some(COLOR_OVERLAY_RED), Some(3.0));
					}
					i => {
						// use 'self.point' as absolute reference as it match the index of vertices of star starting from 0
						if i % 2 != 0 {
							// flipped case
							let point_position = calculate_star_vertex_position(viewport, self.point as i32, n, radius1, radius2);
							let target_index = (1 - (*i as i32)).abs() + (self.point as i32);
							let target_point_position = calculate_star_vertex_position(viewport, target_index, n, radius1, radius2);

							let mirrored_index = 2 * (self.point as i32) - target_index;
							let mirrored = calculate_star_vertex_position(viewport, mirrored_index, n, radius1, radius2);

							overlay_context.line(point_position, target_point_position, Some(COLOR_OVERLAY_RED), Some(3.0));
							overlay_context.line(point_position, mirrored, Some(COLOR_OVERLAY_RED), Some(3.0));
						} else {
							let outer_index = (self.point as i32) - 1;
							let outer_position = calculate_star_vertex_position(viewport, outer_index, n, radius1, radius2);

							// the vertex which is colinear with the point we are dragging and its previous outer vertex
							let target_index = (self.point as i32) + (*i as i32) - 1;
							let target_point_position = calculate_star_vertex_position(viewport, target_index, n, radius1, radius2);

							let mirrored_index = 2 * outer_index - target_index;

							let mirrored = calculate_star_vertex_position(viewport, mirrored_index, n, radius1, radius2);

							overlay_context.line(outer_position, target_point_position, Some(COLOR_OVERLAY_RED), Some(3.0));
							overlay_context.line(outer_position, mirrored, Some(COLOR_OVERLAY_RED), Some(3.0));
						}
					}
				}

				star_outline(Some(layer), document, overlay_context);

				// 0,1 90
			}
		}
	}

	fn calculate_snap_radii(document: &DocumentMessageHandler, layer: LayerNodeIdentifier, radius_index: usize) -> Vec<f64> {
		let mut snap_radii = Vec::new();

		let Some(node_inputs) = NodeGraphLayer::new(layer, &document.network_interface).find_node_inputs("Star") else {
			return snap_radii;
		};

		let other_index = if radius_index == 3 { 2 } else { 3 };

		let Some(&TaggedValue::F64(other_radius)) = node_inputs[other_index].as_value() else {
			return snap_radii;
		};

		let Some(&TaggedValue::U32(n)) = node_inputs[1].as_value() else {
			return snap_radii;
		};

		// inner radius for 90
		let b = FRAC_PI_4 * 3. - PI / (n as f64);
		let angle = b.sin();
		let required_radius = (other_radius / angle) * FRAC_1_SQRT_2;

		snap_radii.push(required_radius);

		// also push the case when the when it length increases more than the other

		let flipped = other_radius * angle * SQRT_2;

		snap_radii.push(flipped);

		for i in 1..n {
			let n = n as f64;
			let i = i as f64;
			let denominator = 2. * ((PI * (i - 1.0)) / n).cos() * ((PI * i) / n).sin();
			let numerator = ((2. * PI * i) / n).sin();
			let factor = numerator / denominator;

			if factor < 0.0 {
				break;
			}

			if other_radius * factor > 1e-6 {
				snap_radii.push(other_radius * factor);
			}

			snap_radii.push((other_radius * 1.0) / factor);
		}

		snap_radii
	}

	fn check_snapping(&self, new_radius: f64, original_radius: f64) -> Option<(usize, f64)> {
		self.snap_radii
			.iter()
			.enumerate()
			.filter(|(_, rad)| (**rad - new_radius).abs() < POINT_RADIUS_HANDLE_SNAP_THRESHOLD)
			.min_by(|(i_a, a), (i_b, b)| {
				let dist_a = (**a - new_radius).abs();
				let dist_b = (**b - new_radius).abs();

				// Check if either index is 0 or 1 and prioritize them
				match (*i_a == 0 || *i_a == 1, *i_b == 0 || *i_b == 1) {
					(true, false) => std::cmp::Ordering::Less,                             // a is priority index, b is not
					(false, true) => std::cmp::Ordering::Greater,                          // b is priority index, a is not
					_ => dist_a.partial_cmp(&dist_b).unwrap_or(std::cmp::Ordering::Equal), // normal comparison
				}
			})
			.map(|(i, rad)| (i, *rad - original_radius))
	}

	pub fn update_inner_radius(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, responses: &mut VecDeque<Message>, drag_start: DVec2) {
		let Some(layer) = self.layer else { return };

		let Some(node_id) = graph_modification_utils::get_star_id(layer, &document.network_interface).or(graph_modification_utils::get_polygon_id(layer, &document.network_interface)) else {
			return;
		};

		let viewport_transform = document.network_interface.document_metadata().transform_to_viewport(layer);
		let document_transform = document.network_interface.document_metadata().transform_to_document(layer);
		let center = viewport_transform.transform_point2(DVec2::ZERO);
		let radius_index = self.radius_index;

		let original_radius = self.initial_radius;

		let delta = viewport_transform.inverse().transform_point2(input.mouse.position) - document_transform.inverse().transform_point2(drag_start);
		let radius = document.metadata().document_to_viewport.transform_point2(drag_start) - center;
		let projection = delta.project_onto(radius);
		let sign = radius.dot(delta).signum();

		let mut net_delta = projection.length() * sign;
		let new_radius = original_radius + net_delta;

		self.update_state(PointRadiusHandleState::Dragging);
		if let Some((index, snapped_delta)) = self.check_snapping(new_radius, original_radius) {
			net_delta = snapped_delta;
			self.update_state(PointRadiusHandleState::Snapped(index));
		}

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, radius_index),
			input: NodeInput::value(TaggedValue::F64(original_radius + net_delta), false),
		});
		responses.add(NodeGraphMessage::RunDocumentGraph);
	}
}

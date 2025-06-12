use super::line_shape::NodeGraphLayer;
use super::shape_utility::{ShapeToolModifierKey, update_radius_sign};
use super::*;
use crate::consts::{COLOR_OVERLAY_RED, POINT_RADIUS_HANDLE_SNAP_THRESHOLD};
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::shapes::shape_utility::points_on_inner_circle;
use crate::messages::tool::tool_messages::tool_prelude::*;
use core::f64;
use glam::DAffine2;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use std::collections::VecDeque;
use std::f64::consts::FRAC_PI_4;
use std::f64::consts::{FRAC_1_SQRT_2, PI, SQRT_2};

#[derive(Default)]
pub struct Star;

#[derive(Clone, Debug, Default, PartialEq)]
enum PointRadiusHandleState {
	#[default]
	Inactive,
	Hover,
	Dragging,
	Snapped(usize),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PointRadiusHandle {
	layer: LayerNodeIdentifier,
	pub point: u32,
	pub index: usize,
	pub snap_radii: Vec<f64>,
	initial_radii: f64,
	handle_state: PointRadiusHandleState,
}

#[derive(Clone, Debug, Default)]
enum NumberOfPointsHandleState {
	#[default]
	Inactive,
	Dragging,
}

#[derive(Clone, Debug, Default)]

pub struct NumberOfPointsHandle {
	initial_points: u32,
	handle_state: NumberOfPointsHandleState,
}

impl NumberOfPointsHandle {
	fn overlays(&self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, mouse_position: DVec2, overlay_context: &mut OverlayContext) {}
	fn update_state(&mut self, state: NumberOfPointsHandleState) {
		self.handle_state = state;
	}
}

impl PointRadiusHandle {
	pub fn cleanup(&mut self) {
		self.handle_state = PointRadiusHandleState::Inactive;
		self.snap_radii.clear();
	}

	pub fn is_hovered(&self) -> bool {
		self.handle_state == PointRadiusHandleState::Hover
	}

	fn update_state(&mut self, state: PointRadiusHandleState) {
		self.handle_state = state;
	}

	pub fn overlays(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, mouse_position: DVec2, overlay_context: &mut OverlayContext) {
		match &self.handle_state {
			PointRadiusHandleState::Inactive => {
				for layer in document
					.network_interface
					.selected_nodes()
					.selected_visible_and_unlocked_layers(&document.network_interface)
					.filter(|layer| graph_modification_utils::get_star_id(*layer, &document.network_interface).is_some())
				{
					let Some(node_inputs) = NodeGraphLayer::new(layer, &document.network_interface).find_node_inputs("Star") else {
						return;
					};

					let (Some(&TaggedValue::U32(n)), Some(&TaggedValue::F64(outer)), Some(&TaggedValue::F64(inner))) =
						(node_inputs[1].as_value(), node_inputs[2].as_value(), node_inputs[3].as_value())
					else {
						return;
					};

					let viewport = document.metadata().transform_to_viewport(layer);

					for i in 0..(2 * n) {
						let angle = i as f64 * PI / n as f64;
						let (radius, radius_index) = if i % 2 == 0 { (outer, 2) } else { (inner, 3) };

						let point = viewport.transform_point2(DVec2 {
							x: radius * angle.sin(),
							y: -radius * angle.cos(),
						});

						let center = viewport.transform_point2(DVec2::ZERO);
						let viewport_diagonal = input.viewport_bounds.size().length();

						if point.distance(mouse_position) < 5.0 {
							let Some(direction) = (point - center).try_normalize() else {
								continue;
							};

							self.layer = layer;
							self.point = i;
							self.snap_radii = Self::calculate_snap_radii(document, layer, radius_index);
							self.update_state(PointRadiusHandleState::Hover);
							overlay_context.manipulator_handle(point, true, None);
							overlay_context.line(center, center + direction * viewport_diagonal, None, None);

							break;
						}

						overlay_context.manipulator_handle(point, false, None);
					}
				}
			}

			PointRadiusHandleState::Dragging | PointRadiusHandleState::Hover => {
				let layer = self.layer;
				let viewport = document.metadata().transform_to_viewport(layer);
				let center = viewport.transform_point2(DVec2::ZERO);
				let viewport_diagonal = input.viewport_bounds.size().length();

				let Some(node_inputs) = NodeGraphLayer::new(layer, &document.network_interface).find_node_inputs("Star") else {
					return;
				};

				let (Some(&TaggedValue::U32(n)), Some(&TaggedValue::F64(outer)), Some(&TaggedValue::F64(inner))) = (node_inputs[1].as_value(), node_inputs[2].as_value(), node_inputs[3].as_value())
				else {
					return;
				};
				let angle = self.point as f64 * PI / n as f64;
				let radius = if self.point % 2 == 0 { outer } else { inner };

				let point = viewport.transform_point2(DVec2 {
					x: radius * angle.sin(),
					y: -radius * angle.cos(),
				});

				if matches!(&self.handle_state, PointRadiusHandleState::Hover) {
					if (mouse_position - point).length() > 5. {
						self.update_state(PointRadiusHandleState::Inactive);
						return;
					}
				}

				let Some(direction) = (point - center).try_normalize() else { return };

				// Draws the ray from the center to the dragging point extending till the viewport
				overlay_context.manipulator_handle(point, true, None);
				overlay_context.line(center, center + direction * viewport_diagonal, None, None);

				// makes the ticks for snapping

				// if dragging to make radius negative don't show the
				if (mouse_position - center).dot(direction) < 0. {
					return;
				}

				for snapped_radius in &self.snap_radii {
					let Some(tick_direction) = direction.perp().try_normalize() else { return };
					// let Some(&TaggedValue::F64(radius)) = node_inputs[self.index].as_value() else { return };
					let difference = snapped_radius - self.initial_radii;
					log::info!("difference {:?}", difference);

					let tick_position = viewport.transform_point2(DVec2 {
						x: snapped_radius * angle.sin(),
						y: -snapped_radius * angle.cos(),
					});

					// let tick_position = viewport.transform_point2(initial_point_position + difference * direction);

					// overlay_context.manipulator_handle(tick_position, false, None);

					overlay_context.line(tick_position, tick_position + tick_direction * 5., None, Some(2.));
					overlay_context.line(tick_position, tick_position - tick_direction * 5., None, Some(2.));
				}
			}
			PointRadiusHandleState::Snapped(snapping_index) => {
				let layer = self.layer;
				let viewport = document.metadata().transform_to_viewport(layer);

				let Some(node_inputs) = NodeGraphLayer::new(layer, &document.network_interface).find_node_inputs("Star") else {
					return;
				};

				let (Some(&TaggedValue::U32(n)), Some(&TaggedValue::F64(outer)), Some(&TaggedValue::F64(inner))) = (node_inputs[1].as_value(), node_inputs[2].as_value(), node_inputs[3].as_value())
				else {
					return;
				};

				let radius = |i: i32| -> f64 { if i.abs() % 2 == 0 { outer } else { inner } };
				let viewport_position = |i: i32, radius: f64| -> DVec2 {
					let angle = i as f64 * PI / n as f64;

					viewport.transform_point2(DVec2 {
						x: radius * angle.sin(),
						y: -radius * angle.cos(),
					})
				};

				match snapping_index {
					//Make a triangle with previous two points
					0 => {
						let outer_radius: f64 = radius(self.point as i32 - 1);
						let outer_position = viewport_position(self.point as i32 - 1, outer_radius);

						let before_outer_radius = radius(self.point as i32 - 2);
						let before_outer_position = viewport_position(self.point as i32 - 2, before_outer_radius);

						let point_radius = radius(self.point as i32);
						let point_position = viewport_position(self.point as i32, point_radius);

						overlay_context.line(before_outer_position, outer_position, Some(COLOR_OVERLAY_RED), Some(3.));
						overlay_context.line(outer_position, point_position, Some(COLOR_OVERLAY_RED), Some(3.));

						let l1 = (before_outer_position - outer_position).length() * 0.2;
						let Some(l1_direction) = (before_outer_position - outer_position).try_normalize() else { return };
						let l1_angle = -l1_direction.angle_to(DVec2::X);
						// overlay_context.draw_angle(outer_position, l1, f64::MAX, l1_angle, FRAC_PI_2);

						let l2 = (point_position - outer_position).length() * 0.2;
						let Some(l2_direction) = (point_position - outer_position).try_normalize() else { return };
						let l2_angle = -l2_direction.angle_to(DVec2::X);

						let net_angle = (l2_angle + l1_angle) / 2.;

						let new_point = SQRT_2 * l1 * DVec2::from_angle(net_angle) + outer_position;

						let before_outer_position = l1 * l1_direction + outer_position;
						let point_position = l1 * l2_direction + outer_position;

						overlay_context.line(before_outer_position, new_point, Some(COLOR_OVERLAY_RED), Some(3.));
						overlay_context.line(new_point, point_position, Some(COLOR_OVERLAY_RED), Some(3.));
					}
					1 => {
						let before_outer_radius = radius(self.point as i32 - 1);
						let before_outer_position = viewport_position(self.point as i32 - 1, before_outer_radius);

						let after_point_radius = radius(self.point as i32 + 1);
						let after_point_position = viewport_position(self.point as i32 + 1, after_point_radius);

						let point_radius = radius(self.point as i32);
						let point_position = viewport_position(self.point as i32, point_radius);

						overlay_context.line(before_outer_position, point_position, Some(COLOR_OVERLAY_RED), Some(3.));
						overlay_context.line(point_position, after_point_position, Some(COLOR_OVERLAY_RED), Some(3.));

						let l1 = (before_outer_position - point_position).length() * 0.2;
						let Some(l1_direction) = (before_outer_position - point_position).try_normalize() else { return };
						let l1_angle = -l1_direction.angle_to(DVec2::X);

						let l2 = (after_point_position - point_position).length() * 0.2;
						let Some(l2_direction) = (after_point_position - point_position).try_normalize() else { return };
						let l2_angle = -l2_direction.angle_to(DVec2::X);

						let net_angle = (l2_angle + l1_angle) / 2.;

						let new_point = SQRT_2 * l1 * DVec2::from_angle(net_angle) + point_position;

						let before_outer_position = l1 * l1_direction + point_position;
						let after_point_position = l1 * l2_direction + point_position;

						overlay_context.line(before_outer_position, new_point, Some(COLOR_OVERLAY_RED), Some(3.));
						overlay_context.line(new_point, after_point_position, Some(COLOR_OVERLAY_RED), Some(3.));
					}
					i => {
						if i % 2 != 0 {
							// flipped case
							let point_radius = radius(self.point as i32);
							let point_position = viewport_position(self.point as i32, point_radius);

							let target_index = *i as i32;
							let target_point_radius = radius(target_index);
							let target_point_position = viewport_position(target_index, target_point_radius);

							let mirrored = viewport_position(-target_index + 2, target_point_radius);

							overlay_context.line(point_position, target_point_position, Some(COLOR_OVERLAY_RED), Some(3.));
							overlay_context.line(point_position, mirrored, Some(COLOR_OVERLAY_RED), Some(3.));
						} else {
							let outer_radius = radius(self.point as i32 - 1);
							let outer_position = viewport_position(self.point as i32 - 1, outer_radius);

							let target_index = self.point as i32 + *i as i32 - 1;
							let target_point_radius = radius(target_index);
							let target_point_position = viewport_position(target_index, target_point_radius);

							let mirrored = viewport_position(-target_index, target_point_radius);

							overlay_context.line(outer_position, target_point_position, Some(COLOR_OVERLAY_RED), Some(3.));
							overlay_context.line(outer_position, mirrored, Some(COLOR_OVERLAY_RED), Some(3.));
						}
					}
				}
				// 0,1 90
			}
		}
	}
	fn calculate_snap_radii(document: &DocumentMessageHandler, layer: LayerNodeIdentifier, index: usize) -> Vec<f64> {
		let mut snap_radii = Vec::new();

		let Some(node_inputs) = NodeGraphLayer::new(layer, &document.network_interface).find_node_inputs("Star") else {
			return snap_radii;
		};

		let other_index = if index == 3 { 2 } else { 3 };

		let Some(&TaggedValue::F64(other_radius)) = node_inputs[other_index].as_value() else {
			return snap_radii;
		};

		let Some(&TaggedValue::U32(n)) = node_inputs[1].as_value() else {
			return snap_radii;
		};

		// inner radius for 90
		let b = FRAC_PI_4 * (3.) - (PI / n as f64);
		let angle = b.sin();
		let required_radius = (other_radius / angle) * (FRAC_1_SQRT_2);

		snap_radii.push(required_radius);

		// also push the case when the when it length increases more than the other

		let flipped = other_radius * angle * SQRT_2;

		snap_radii.push(flipped);

		for i in 1..n {
			let n = n as f64;
			let i = i as f64;
			let denominator = 2. * (PI * (i - 1.) / n).cos() * (PI * i / n).sin();
			let numerator = (2. * PI * i / n).sin();
			let factor = numerator / denominator;

			if factor < 0. {
				break;
			}

			if (other_radius * factor) > 1e-6 {
				snap_radii.push(other_radius * factor);
			}

			snap_radii.push(other_radius * 1. / factor);
		}

		snap_radii
	}

	fn check_snapping(&self, new_radius: f64, original_radius: f64) -> Option<(usize, f64)> {
		self.snap_radii
			.iter()
			.enumerate()
			.filter(|(_, rad)| (**rad - new_radius).abs() < POINT_RADIUS_HANDLE_SNAP_THRESHOLD)
			.min_by(|(_, a), (_, b)| (**a - new_radius).abs().partial_cmp(&(**b - new_radius).abs()).unwrap_or(std::cmp::Ordering::Equal))
			.map(|(i, rad)| (i, *rad - original_radius))
	}
}

#[derive(Clone, Debug, Default)]
pub struct StarShapeData {
	pub point_radius_handle: PointRadiusHandle,
	pub number_of_points_handle: NumberOfPointsHandle,
}

impl StarShapeData {
	pub fn star_gizmos(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, mouse_position: DVec2, overlay_context: &mut OverlayContext) {
		self.point_radius_handle.overlays(document, input, mouse_position, overlay_context);
		self.number_of_points_handle.overlays(document, input, mouse_position, overlay_context);
	}

	pub fn set_point_radius_handle(&mut self, document: &DocumentMessageHandler, mouse_pos: DVec2) -> Option<LayerNodeIdentifier> {
		if let Some((layer, point, index, initial_radii)) = points_on_inner_circle(document, mouse_pos) {
			let snap_radii = PointRadiusHandle::calculate_snap_radii(document, layer, index);
			self.point_radius_handle = PointRadiusHandle {
				layer,
				point,
				index,
				snap_radii,
				initial_radii,
				handle_state: PointRadiusHandleState::Dragging,
			};
			return Some(layer);
		}
		None
	}

	pub fn update_inner_radius(
		&mut self,
		document: &DocumentMessageHandler,
		input: &InputPreprocessorMessageHandler,
		layer: LayerNodeIdentifier,
		responses: &mut VecDeque<Message>,
		drag_start: DVec2,
	) {
		let Some(node_id) = graph_modification_utils::get_star_id(layer, &document.network_interface) else {
			return;
		};

		let Some(vector_data) = document.network_interface.compute_modified_vector(layer) else {
			return;
		};

		let path = vector_data.stroke_bezier_paths().next().unwrap();
		let center = path.length_centroid(None, true).unwrap();
		let transform = document.network_interface.document_metadata().transform_to_viewport(layer);
		let index = self.point_radius_handle.index;

		let original_radius = self.point_radius_handle.initial_radii;

		let delta = input.mouse.position - document.metadata().document_to_viewport.transform_point2(drag_start);
		let radius = document.metadata().document_to_viewport.transform_point2(drag_start) - transform.transform_point2(center);
		let projection = delta.project_onto(radius);
		let sign = radius.dot(delta).signum();

		let mut net_delta = projection.length() * sign;
		let new_radius = original_radius + net_delta;

		self.point_radius_handle.update_state(PointRadiusHandleState::Dragging);
		if let Some((index, snapped_delta)) = self.point_radius_handle.check_snapping(new_radius, original_radius) {
			net_delta = snapped_delta;
			self.point_radius_handle.update_state(PointRadiusHandleState::Snapped(index));
		}

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, index),
			input: NodeInput::value(TaggedValue::F64(original_radius + net_delta), false),
		});
		responses.add(NodeGraphMessage::RunDocumentGraph);
	}
}

impl Star {
	pub fn create_node(vertices: u32) -> NodeTemplate {
		let node_type = resolve_document_node_type("Star").expect(" Star node does not exist");
		node_type.node_template_input_override([
			None,
			Some(NodeInput::value(TaggedValue::U32(vertices), false)),
			Some(NodeInput::value(TaggedValue::F64(0.5), false)),
			Some(NodeInput::value(TaggedValue::F64(0.25), false)),
		])
	}

	pub fn update_shape(
		document: &DocumentMessageHandler,
		ipp: &InputPreprocessorMessageHandler,
		layer: LayerNodeIdentifier,
		shape_tool_data: &mut ShapeToolData,
		modifier: ShapeToolModifierKey,
		responses: &mut VecDeque<Message>,
	) -> bool {
		let (center, lock_ratio) = (modifier[0], modifier[1]);
		if let Some([start, end]) = shape_tool_data.data.calculate_points(document, ipp, center, lock_ratio) {
			// TODO: We need to determine how to allow the polygon node to make irregular shapes
			update_radius_sign(end, start, layer, document, responses);

			let dimensions = (start - end).abs();
			let mut scale = DVec2::ONE;
			let radius: f64;

			// We keep the smaller dimension's scale at 1 and scale the other dimension accordingly
			if dimensions.x > dimensions.y {
				scale.x = dimensions.x / dimensions.y;
				radius = dimensions.y / 2.;
			} else {
				scale.y = dimensions.y / dimensions.x;
				radius = dimensions.x / 2.;
			}

			let Some(node_id) = graph_modification_utils::get_star_id(layer, &document.network_interface) else {
				return false;
			};

			responses.add(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(node_id, 2),
				input: NodeInput::value(TaggedValue::F64(radius), false),
			});

			responses.add(NodeGraphMessage::SetInput {
				input_connector: InputConnector::node(node_id, 3),
				input: NodeInput::value(TaggedValue::F64(radius / 2.), false),
			});

			responses.add(GraphOperationMessage::TransformSet {
				layer,
				transform: DAffine2::from_scale_angle_translation(scale, 0., (start + end) / 2.),
				transform_in: TransformIn::Viewport,
				skip_rerender: false,
			});
		}
		false
	}
}

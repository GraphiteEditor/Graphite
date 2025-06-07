use super::line_shape::NodeGraphLayer;
use super::shape_utility::{ShapeToolModifierKey, update_radius_sign};
use super::*;
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::tool_messages::tool_prelude::*;
use glam::DAffine2;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graphene_std::vector::PointId;
use std::collections::VecDeque;

#[derive(Default)]
pub struct Star;

#[derive(Clone, Debug, Default)]
pub struct PointRadiusHandle {
	pub center: DVec2,
	pub vertex: Option<PointId>,
	pub index: usize,
}

#[derive(Clone, Debug, Default)]
pub struct StarShapeData {
	pub point_radius_handle: PointRadiusHandle,
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

	pub fn set_point_radius_handle(document: &DocumentMessageHandler, mouse_pos: DVec2, shape_tool_data: &mut ShapeToolData) -> bool {
		if let Some((layer, (center, _, vertex, index))) = Self::points_on_inner_circle(document, mouse_pos).iter().next() {
			shape_tool_data.data.layer = Some(*layer);
			shape_tool_data.star_data.point_radius_handle = PointRadiusHandle {
				center: *center,
				vertex: Some(*vertex),
				index: *index,
			};
			return true;
		}
		false
	}

	pub fn inner_gizmo_overlays(document: &DocumentMessageHandler, shape_tool_data: &mut ShapeToolData, overlay_context: &mut OverlayContext) {
		let PointRadiusHandle { center, vertex, .. } = shape_tool_data.star_data.point_radius_handle;
		let layer = shape_tool_data.data.layer.unwrap();
		let vector_data = document.network_interface.compute_modified_vector(layer).unwrap();
		let viewport = document.metadata().transform_to_viewport(layer);
		let vertex_pos = vector_data.point_domain.position_from_id(vertex.unwrap()).unwrap();
		Self::draw_point_radius_overlay(center, vertex_pos, viewport, overlay_context);
	}

	fn draw_point_radius_overlay(center: DVec2, vertex_pos: DVec2, transform: DAffine2, overlay_context: &mut OverlayContext) {
		let viewport_center = transform.transform_point2(center);
		let viewport_vertex = transform.transform_point2(vertex_pos);
		let extension_length = (viewport_vertex - viewport_center).length() * 0.5;
		let extension = (viewport_vertex - viewport_center).normalize() * extension_length;

		overlay_context.line(viewport_center, viewport_vertex + extension, None, None);
		overlay_context.manipulator_handle(viewport_vertex, true, None);
	}
	// when hovered
	pub fn hover_point_radius_handle(document: &DocumentMessageHandler, mouse_pos: DVec2, overlay_context: &mut OverlayContext) -> bool {
		for (layer, (center, vertex_pos, _, _)) in Self::points_on_inner_circle(document, mouse_pos) {
			let transform = document.metadata().transform_to_viewport(layer);
			Self::draw_point_radius_overlay(center, vertex_pos, transform, overlay_context);
			return true;
		}

		for layer in document
			.network_interface
			.selected_nodes()
			.selected_visible_and_unlocked_layers(&document.network_interface)
			.filter(|layer| graph_modification_utils::get_star_id(*layer, &document.network_interface).is_some())
		{
			let Some(vector_data) = document.network_interface.compute_modified_vector(layer) else {
				return false;
			};

			for (_, anchor_positions) in vector_data.point_domain.position_ids() {
				let transform = document.metadata().transform_to_viewport(layer);
				overlay_context.manipulator_handle(transform.transform_point2(*anchor_positions), false, None);
			}

			return false;
		}
		false
	}

	pub fn points_on_inner_circle(document: &DocumentMessageHandler, mouse_pos: DVec2) -> HashMap<LayerNodeIdentifier, (DVec2, DVec2, PointId, usize)> {
		let mut result = HashMap::new();

		for layer in document
			.network_interface
			.selected_nodes()
			.selected_visible_and_unlocked_layers(&document.network_interface)
			.filter(|layer| graph_modification_utils::get_star_id(*layer, &document.network_interface).is_some())
		{
			let Some(node_inputs) = NodeGraphLayer::new(layer, &document.network_interface).find_node_inputs("Star") else {
				return result;
			};

			let Some(vector_data) = document.network_interface.compute_modified_vector(layer) else {
				return result;
			};

			let transform = document.network_interface.document_metadata().transform_to_viewport(layer);
			let center = DVec2::ZERO;

			let (Some(&TaggedValue::F64(outer)), Some(&TaggedValue::F64(inner))) = (node_inputs[2].as_value(), node_inputs[3].as_value()) else {
				return result;
			};

			let mut index = 0;

			let inner_point = vector_data.point_domain.position_ids().find(|(_, pos)| {
				let transformed = transform.transform_point2(**pos);
				if transformed.distance(mouse_pos) >= 5.0 {
					return false;
				}

				let dist = pos.distance(center);
				if (dist - inner).abs() < 1e-6 {
					index = 3;

					true
				} else if (dist - outer).abs() < 1e-6 {
					index = 2;
					log::info!("dist to outer {:?}", (dist - outer).abs());

					true
				} else {
					false
				}
			});

			// Only insert if we found an inner point
			if let Some((point_id, vertex_pos)) = inner_point {
				result.insert(layer, (center, *vertex_pos, point_id, index));
				break;
			}
		}

		result
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

	pub fn update_inner_radius(
		document: &DocumentMessageHandler,
		input: &InputPreprocessorMessageHandler,
		layer: LayerNodeIdentifier,
		responses: &mut VecDeque<Message>,
		shape_tool_data: &mut ShapeToolData,
	) {
		let Some(node_id) = graph_modification_utils::get_star_id(layer, &document.network_interface) else {
			return;
		};

		let Some(vector_data) = document.network_interface.compute_modified_vector(layer) else {
			return;
		};

		let Some(node_inputs) = NodeGraphLayer::new(layer, &document.network_interface).find_node_inputs("Star") else {
			return;
		};

		let path = vector_data.stroke_bezier_paths().next().unwrap();
		let center = path.length_centroid(None, true).unwrap();
		let transform = document.network_interface.document_metadata().transform_to_viewport(layer);
		let index = shape_tool_data.star_data.point_radius_handle.index;

		// inner radiust
		let Some(&TaggedValue::F64(required_radius)) = node_inputs[index].as_value() else {
			return;
		};

		// update_radius_sign(start, end, layer, document, responses);

		let delta = input.mouse.position - shape_tool_data.last_mouse_position;
		let radius = document.metadata().document_to_viewport.transform_point2(shape_tool_data.data.drag_start) - transform.transform_point2(center);
		let projection = delta.project_onto(radius);
		let sign = radius.dot(delta).signum();

		let net_delta = projection.length() * sign;
		shape_tool_data.last_mouse_position = input.mouse.position;

		// overlay_context.line(transform.transform_point2(center), transform.transform_point2(inner + net_delta), None, None);

		// We keep the smaller dimension's scale at 1 and scale the other dimension accordingly

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, index),
			input: NodeInput::value(TaggedValue::F64(required_radius + net_delta), false),
		});
		responses.add(NodeGraphMessage::RunDocumentGraph);
	}
}

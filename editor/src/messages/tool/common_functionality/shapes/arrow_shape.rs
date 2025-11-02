use super::shape_utility::ShapeToolModifierKey;
use super::*;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::NodeTemplate;
use crate::messages::prelude::*;
use glam::DVec2;
use graphene_std::vector::{PointId, SegmentId, VectorModificationType};
use std::collections::VecDeque;

#[derive(Default)]
pub struct Arrow;

impl Arrow {
	pub fn create_node() -> NodeTemplate {
		let node_type = resolve_document_node_type("Path").expect("Path node does not exist");
		node_type.default_node_template()
	}

	pub fn update_shape(
		document: &DocumentMessageHandler,
		input: &InputPreprocessorMessageHandler,
		layer: LayerNodeIdentifier,
		tool_data: &mut ShapeToolData,
		modifier: ShapeToolModifierKey,
		responses: &mut VecDeque<Message>,
	) {
		let [center, lock_ratio, _] = modifier;

		// Work in viewport space like Line does
		let start_viewport = tool_data.data.viewport_drag_start(document);
		let end_viewport = input.mouse.position;

		let delta = end_viewport - start_viewport;
		let length = delta.length();
		if length < 1e-6 {
			return;
		}

		let direction = delta.normalize();
		let perpendicular = DVec2::new(-direction.y, direction.x);

		let shaft_thickness = length * 0.05;
		let head_width = length * 0.15;
		let head_length = length * 0.2;

		// Build arrow in viewport space
		let viewport_anchors = vec![
			start_viewport,
			start_viewport + direction * head_length - perpendicular * (head_width * 0.5),
			start_viewport + direction * head_length - perpendicular * (shaft_thickness * 0.5),
			end_viewport - perpendicular * (shaft_thickness * 0.5),
			end_viewport + perpendicular * (shaft_thickness * 0.5),
			start_viewport + direction * head_length + perpendicular * (shaft_thickness * 0.5),
			start_viewport + direction * head_length + perpendicular * (head_width * 0.5),
		];

		let vector = document.network_interface.compute_modified_vector(layer);
		let existing_point_ids: Vec<PointId> = vector.as_ref().map(|v| v.point_domain.ids().to_vec()).unwrap_or_default();
		let existing_segment_ids: Vec<SegmentId> = vector.as_ref().map(|v| v.segment_domain.ids().to_vec()).unwrap_or_default();

		for point_id in existing_point_ids {
			responses.add(GraphOperationMessage::Vector {
				layer,
				modification_type: VectorModificationType::RemovePoint { id: point_id },
			});
		}

		for segment_id in existing_segment_ids {
			responses.add(GraphOperationMessage::Vector {
				layer,
				modification_type: VectorModificationType::RemoveSegment { id: segment_id },
			});
		}

		let point_ids: Vec<PointId> = viewport_anchors
			.iter()
			.map(|&pos| {
				let id = PointId::generate();
				responses.add(GraphOperationMessage::Vector {
					layer,
					modification_type: VectorModificationType::InsertPoint { id, position: pos },
				});
				id
			})
			.collect();

		for i in 0..point_ids.len() {
			let id = SegmentId::generate();
			let points = [point_ids[i], point_ids[(i + 1) % point_ids.len()]];
			responses.add(GraphOperationMessage::Vector {
				layer,
				modification_type: VectorModificationType::InsertSegment { id, points, handles: [None, None] },
			});
		}

		responses.add(NodeGraphMessage::RunDocumentGraph);
	}

	pub fn overlays(_document: &DocumentMessageHandler, _tool_data: &ShapeToolData, _overlay_context: &mut OverlayContext) {}
}

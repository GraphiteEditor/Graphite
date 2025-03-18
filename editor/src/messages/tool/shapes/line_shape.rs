use super::*;
use crate::consts::LINE_ROTATE_SNAP_ANGLE;
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, NodeTemplate};
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::snapping::{SnapCandidatePoint, SnapConstraint, SnapData, SnapTypeConfiguration};
use crate::messages::tool::tool_messages::shape_tool::ShapeToolData;
use crate::messages::tool::tool_messages::tool_prelude::*;
use glam::DVec2;
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use std::collections::VecDeque;

#[derive(Clone, Debug, Default)]
pub enum LineEnd {
	#[default]
	Start,
	End,
}

#[derive(Default)]
pub struct Line;

impl Line {
	pub fn name() -> &'static str {
		"Line"
	}

	pub fn icon_name() -> &'static str {
		"VectorLineTool"
	}

	pub fn create_node(document: &DocumentMessageHandler, init_data: LineInitData) -> NodeTemplate {
		let drag_start = init_data.drag_start;
		let node_type = resolve_document_node_type("Line").expect("Line node does not exist");
		node_type.node_template_input_override([
			None,
			Some(NodeInput::value(TaggedValue::DVec2(document.metadata().document_to_viewport.transform_point2(drag_start)), false)),
			Some(NodeInput::value(TaggedValue::DVec2(document.metadata().document_to_viewport.transform_point2(drag_start)), false)),
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
		let (center, snap_angle, lock_angle) = (modifier[0], modifier[3], modifier[2]);
		shape_tool_data.drag_current = ipp.mouse.position;
		let keyboard = &ipp.keyboard;
		let ignore = vec![layer];
		let snap_data = SnapData::ignore(document, ipp, &ignore);
		let document_points = generate_line(shape_tool_data, snap_data, keyboard.key(lock_angle), keyboard.key(snap_angle), keyboard.key(center));

		let Some(node_id) = graph_modification_utils::get_line_id(layer, &document.network_interface) else {
			return true;
		};

		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, 1),
			input: NodeInput::value(TaggedValue::DVec2(document_points[0]), false),
		});
		responses.add(NodeGraphMessage::SetInput {
			input_connector: InputConnector::node(node_id, 2),
			input: NodeInput::value(TaggedValue::DVec2(document_points[1]), false),
		});
		responses.add(NodeGraphMessage::RunDocumentGraph);
		false
	}
}

fn generate_line(tool_data: &mut ShapeToolData, snap_data: SnapData, lock_angle: bool, snap_angle: bool, center: bool) -> [DVec2; 2] {
	let document_to_viewport = snap_data.document.metadata().document_to_viewport;
	let mut document_points = [tool_data.data.drag_start, document_to_viewport.inverse().transform_point2(tool_data.drag_current)];

	let mut angle = -(document_points[1] - document_points[0]).angle_to(DVec2::X);
	let mut line_length = (document_points[1] - document_points[0]).length();

	if lock_angle {
		angle = tool_data.angle;
	} else if snap_angle {
		let snap_resolution = LINE_ROTATE_SNAP_ANGLE.to_radians();
		angle = (angle / snap_resolution).round() * snap_resolution;
	}

	tool_data.angle = angle;

	if lock_angle {
		let angle_vec = DVec2::new(angle.cos(), angle.sin());
		line_length = (document_points[1] - document_points[0]).dot(angle_vec);
	}

	document_points[1] = document_points[0] + line_length * DVec2::new(angle.cos(), angle.sin());

	let constrained = snap_angle || lock_angle;
	let snap = &mut tool_data.data.snap_manager;

	let near_point = SnapCandidatePoint::handle_neighbors(document_points[1], [tool_data.data.drag_start]);
	let far_point = SnapCandidatePoint::handle_neighbors(2. * document_points[0] - document_points[1], [tool_data.data.drag_start]);
	let config = SnapTypeConfiguration::default();

	if constrained {
		let constraint = SnapConstraint::Line {
			origin: document_points[0],
			direction: document_points[1] - document_points[0],
		};
		if center {
			let snapped = snap.constrained_snap(&snap_data, &near_point, constraint, config);
			let snapped_far = snap.constrained_snap(&snap_data, &far_point, constraint, config);
			let best = if snapped_far.other_snap_better(&snapped) { snapped } else { snapped_far };
			document_points[1] = document_points[0] * 2. - best.snapped_point_document;
			document_points[0] = best.snapped_point_document;
			snap.update_indicator(best);
		} else {
			let snapped = snap.constrained_snap(&snap_data, &near_point, constraint, config);
			document_points[1] = snapped.snapped_point_document;
			snap.update_indicator(snapped);
		}
	} else if center {
		let snapped = snap.free_snap(&snap_data, &near_point, config);
		let snapped_far = snap.free_snap(&snap_data, &far_point, config);
		let best = if snapped_far.other_snap_better(&snapped) { snapped } else { snapped_far };
		document_points[1] = document_points[0] * 2. - best.snapped_point_document;
		document_points[0] = best.snapped_point_document;
		snap.update_indicator(best);
	} else {
		let snapped = snap.free_snap(&snap_data, &near_point, config);
		document_points[1] = snapped.snapped_point_document;
		snap.update_indicator(snapped);
	}

	document_points
}

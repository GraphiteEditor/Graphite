use super::super::tool_prelude::*;
use crate::consts::PATH_JOIN_THRESHOLD;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::snapping::{SnapCandidatePoint, SnapData, SnapManager, SnapTypeConfiguration, SnappedPoint};
use crate::messages::tool::common_functionality::utility_functions::closest_point;

use graphene_std::vector::{PointId, SegmentId, VectorModificationType};

#[derive(Clone, Debug)]
pub(super) enum EndpointPosition {
	Start,
	End,
}

#[derive(Clone, Debug, Default)]
pub(super) struct SplineModeToolData {
	/// List of points inserted.
	pub points: Vec<(PointId, DVec2)>,
	/// Point to be inserted.
	pub next_point: DVec2,
	/// Point that was inserted temporarily to show preview.
	pub preview_point: Option<PointId>,
	/// Segment that was inserted temporarily to show preview.
	pub preview_segment: Option<SegmentId>,
	pub extend: bool,
	pub weight: f64,
	/// The layer we are editing.
	pub current_layer: Option<LayerNodeIdentifier>,
	/// The layers to merge to the current layer before we merge endpoints in merge_endpoint field.
	pub merge_layers: HashSet<LayerNodeIdentifier>,
	/// The endpoint IDs to merge with the spline's start/end endpoint after spline drawing is finished.
	pub merge_endpoints: Vec<(EndpointPosition, PointId)>,
	pub snap_manager: SnapManager,
	pub auto_panning: AutoPanning,
}

impl SplineModeToolData {
	pub fn cleanup(&mut self) {
		self.current_layer = None;
		self.merge_layers = HashSet::new();
		self.merge_endpoints = Vec::new();
		self.preview_point = None;
		self.preview_segment = None;
		self.extend = false;
		self.points = Vec::new();
	}

	/// Get the snapped point while ignoring current layer
	pub fn snapped_point(&mut self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler) -> SnappedPoint {
		let point = SnapCandidatePoint::handle(document.metadata().document_to_viewport.inverse().transform_point2(input.mouse.position));
		let ignore = if let Some(layer) = self.current_layer { vec![layer] } else { vec![] };
		let snap_data = SnapData::ignore(document, input, &ignore);
		self.snap_manager.free_snap(&snap_data, &point, SnapTypeConfiguration::default())
	}
}

pub(super) fn try_merging_latest_endpoint(document: &DocumentMessageHandler, tool_data: &mut SplineModeToolData, preferences: &PreferencesMessageHandler) -> Option<()> {
	if tool_data.points.len() < 2 {
		return None;
	};
	let (last_endpoint, last_endpoint_position) = tool_data.points.last()?;
	let preview_point = tool_data.preview_point;
	let current_layer = tool_data.current_layer?;

	let layers = LayerNodeIdentifier::ROOT_PARENT
		.descendants(document.metadata())
		.filter(|layer| !document.network_interface.is_artboard(&layer.to_node(), &[]));

	let exclude = |p: PointId| preview_point.is_some_and(|pp| pp == p) || *last_endpoint == p;
	let position = document.metadata().transform_to_viewport(current_layer).transform_point2(*last_endpoint_position);

	let (layer, endpoint, _) = closest_point(document, position, PATH_JOIN_THRESHOLD, layers, exclude, preferences)?;
	tool_data.merge_layers.insert(layer);
	tool_data.merge_endpoints.push((EndpointPosition::End, endpoint));

	Some(())
}

pub(super) fn extend_spline(tool_data: &mut SplineModeToolData, show_preview: bool, responses: &mut VecDeque<Message>) {
	delete_preview(tool_data, responses);

	let Some(layer) = tool_data.current_layer else { return };

	let next_point_pos = tool_data.next_point;
	let next_point_id = PointId::generate();
	let modification_type = VectorModificationType::InsertPoint {
		id: next_point_id,
		position: next_point_pos,
	};
	responses.add(GraphOperationMessage::Vector { layer, modification_type });

	if let Some((last_point_id, _)) = tool_data.points.last() {
		let points = [*last_point_id, next_point_id];
		let id = SegmentId::generate();
		let modification_type = VectorModificationType::InsertSegment { id, points, handles: [None, None] };
		responses.add(GraphOperationMessage::Vector { layer, modification_type });

		if show_preview {
			tool_data.preview_segment = Some(id);
		}
	}

	if show_preview {
		tool_data.preview_point = Some(next_point_id);
	} else {
		tool_data.points.push((next_point_id, next_point_pos));
	}
}

pub(super) fn delete_preview(tool_data: &mut SplineModeToolData, responses: &mut VecDeque<Message>) {
	let Some(layer) = tool_data.current_layer else { return };

	if let Some(id) = tool_data.preview_point {
		let modification_type = VectorModificationType::RemovePoint { id };
		responses.add(GraphOperationMessage::Vector { layer, modification_type });
	}
	if let Some(id) = tool_data.preview_segment {
		let modification_type = VectorModificationType::RemoveSegment { id };
		responses.add(GraphOperationMessage::Vector { layer, modification_type });
	}

	tool_data.preview_point = None;
	tool_data.preview_segment = None;
}

use crate::consts::ROTATE_INCREMENT;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::transformation::Selected;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils::get_text;
use crate::messages::tool::common_functionality::transformation_cage::SelectedEdges;
use crate::messages::tool::tool_messages::path_tool::PathOverlayMode;
use crate::messages::tool::utility_types::ToolType;
use glam::{DAffine2, DVec2};
use graphene_core::renderer::Quad;
use graphene_core::text::{FontCache, load_face};
use graphene_std::vector::{ManipulatorPointId, PointId, SegmentId, VectorData};

use super::snapping::{SnapCandidatePoint, SnapData, SnapManager};
use super::transformation_cage::{BoundingBoxManager, SizeSnapData};

/// Determines if a path should be extended. Goal in viewport space. Returns the path and if it is extending from the start, if applicable.
pub fn should_extend(
	document: &DocumentMessageHandler,
	goal: DVec2,
	tolerance: f64,
	layers: impl Iterator<Item = LayerNodeIdentifier>,
	preferences: &PreferencesMessageHandler,
) -> Option<(LayerNodeIdentifier, PointId, DVec2)> {
	closest_point(document, goal, tolerance, layers, |_| false, preferences)
}

/// Determine the closest point to the goal point under max_distance.
/// Additionally exclude checking closeness to the point which given to exclude() returns true.
pub fn closest_point<T>(
	document: &DocumentMessageHandler,
	goal: DVec2,
	max_distance: f64,
	layers: impl Iterator<Item = LayerNodeIdentifier>,
	exclude: T,
	preferences: &PreferencesMessageHandler,
) -> Option<(LayerNodeIdentifier, PointId, DVec2)>
where
	T: Fn(PointId) -> bool,
{
	let mut best = None;
	let mut best_distance_squared = max_distance * max_distance;
	for layer in layers {
		let viewspace = document.metadata().transform_to_viewport(layer);
		let Some(vector_data) = document.network_interface.compute_modified_vector(layer) else {
			continue;
		};
		for id in vector_data.extendable_points(preferences.vector_meshes) {
			if exclude(id) {
				continue;
			}
			let Some(point) = vector_data.point_domain.position_from_id(id) else { continue };

			let distance_squared = viewspace.transform_point2(point).distance_squared(goal);

			if distance_squared < best_distance_squared {
				best = Some((layer, id, point));
				best_distance_squared = distance_squared;
			}
		}
	}

	best
}

/// Calculates the bounding box of the layer's text, based on the settings for max width and height specified in the typesetting config.
pub fn text_bounding_box(layer: LayerNodeIdentifier, document: &DocumentMessageHandler, font_cache: &FontCache) -> Quad {
	let Some((text, font, typesetting)) = get_text(layer, &document.network_interface) else {
		return Quad::from_box([DVec2::ZERO, DVec2::ZERO]);
	};

	let buzz_face = font_cache.get(font).map(|data| load_face(data));
	let far = graphene_core::text::bounding_box(text, buzz_face.as_ref(), typesetting, false);

	Quad::from_box([DVec2::ZERO, far])
}

pub fn calculate_segment_angle(anchor: PointId, segment: SegmentId, vector_data: &VectorData, prefer_handle_direction: bool) -> Option<f64> {
	let is_start = |point: PointId, segment: SegmentId| vector_data.segment_start_from_id(segment) == Some(point);
	let anchor_position = vector_data.point_domain.position_from_id(anchor)?;
	let end_handle = ManipulatorPointId::EndHandle(segment).get_position(vector_data);
	let start_handle = ManipulatorPointId::PrimaryHandle(segment).get_position(vector_data);

	let start_point = if is_start(anchor, segment) {
		vector_data.segment_end_from_id(segment).and_then(|id| vector_data.point_domain.position_from_id(id))
	} else {
		vector_data.segment_start_from_id(segment).and_then(|id| vector_data.point_domain.position_from_id(id))
	};

	let required_handle = if is_start(anchor, segment) {
		start_handle
			.filter(|&handle| prefer_handle_direction && handle != anchor_position)
			.or(end_handle.filter(|&handle| Some(handle) != start_point))
			.or(start_point)
	} else {
		end_handle
			.filter(|&handle| prefer_handle_direction && handle != anchor_position)
			.or(start_handle.filter(|&handle| Some(handle) != start_point))
			.or(start_point)
	};

	required_handle.map(|handle| -(handle - anchor_position).angle_to(DVec2::X))
}

/// Check whether a point is visible in the current overlay mode.
pub fn is_visible_point(
	manipulator_point_id: ManipulatorPointId,
	vector_data: &VectorData,
	path_overlay_mode: PathOverlayMode,
	frontier_handles_info: Option<HashMap<SegmentId, Vec<PointId>>>,
	selected_segments: Vec<SegmentId>,
	selected_points: &HashSet<ManipulatorPointId>,
) -> bool {
	match manipulator_point_id {
		ManipulatorPointId::Anchor(_) => true,
		ManipulatorPointId::EndHandle(segment_id) | ManipulatorPointId::PrimaryHandle(segment_id) => {
			match (path_overlay_mode, selected_points.len() == 1) {
				(PathOverlayMode::AllHandles, _) => true,
				(PathOverlayMode::SelectedPointHandles, _) | (PathOverlayMode::FrontierHandles, true) => {
					if selected_segments.contains(&segment_id) {
						return true;
					}

					// Either the segment is a part of selected segments or the opposite handle is a part of existing selection
					let Some(handle_pair) = manipulator_point_id.get_handle_pair(vector_data) else { return false };
					let other_handle = handle_pair[1].to_manipulator_point();

					// Return whether the list of selected points contain the other handle
					selected_points.contains(&other_handle)
				}
				(PathOverlayMode::FrontierHandles, false) => {
					let Some(anchor) = manipulator_point_id.get_anchor(vector_data) else {
						warn!("No anchor for selected handle");
						return false;
					};
					let Some(frontier_handles) = &frontier_handles_info else {
						warn!("No frontier handles info provided");
						return false;
					};

					frontier_handles.get(&segment_id).map(|anchors| anchors.contains(&anchor)).unwrap_or_default()
				}
			}
		}
	}
}

pub fn resize_bounds(
	document: &DocumentMessageHandler,
	responses: &mut VecDeque<Message>,
	bounds: &mut BoundingBoxManager,
	dragging_layers: &mut Vec<LayerNodeIdentifier>,
	snap_manager: &mut SnapManager,
	snap_candidates: &mut Vec<SnapCandidatePoint>,
	input: &InputPreprocessorMessageHandler,
	center: bool,
	constrain: bool,
	tool: ToolType,
) {
	if let Some(movement) = &mut bounds.selected_edges {
		let center = center.then_some(bounds.center_of_transformation);
		let snap = Some(SizeSnapData {
			manager: snap_manager,
			points: snap_candidates,
			snap_data: SnapData::ignore(document, input, &dragging_layers),
		});
		let (position, size) = movement.new_size(input.mouse.position, bounds.original_bound_transform, center, constrain, snap);
		let (delta, mut pivot) = movement.bounds_to_scale_transform(position, size);

		let pivot_transform = DAffine2::from_translation(pivot);
		let transformation = pivot_transform * delta * pivot_transform.inverse();

		dragging_layers.retain(|layer| {
			if *layer != LayerNodeIdentifier::ROOT_PARENT {
				document.network_interface.document_network().nodes.contains_key(&layer.to_node())
			} else {
				log::error!("ROOT_PARENT should not be part of layers_dragging");
				false
			}
		});
		let selected = &dragging_layers;
		let mut selected = Selected::new(&mut bounds.original_transforms, &mut pivot, selected, responses, &document.network_interface, None, &tool, None);

		selected.apply_transformation(bounds.original_bound_transform * transformation * bounds.original_bound_transform.inverse(), None);
	}
}

pub fn rotate_bounds(
	document: &DocumentMessageHandler,
	responses: &mut VecDeque<Message>,
	bounds: &mut BoundingBoxManager,
	dragging_layers: &mut Vec<LayerNodeIdentifier>,
	drag_start: DVec2,
	mouse_position: DVec2,
	snap_angle: bool,
	tool: ToolType,
) {
	let angle = {
		let start_offset = drag_start - bounds.center_of_transformation;
		let end_offset = mouse_position - bounds.center_of_transformation;
		start_offset.angle_to(end_offset)
	};

	let snapped_angle = if snap_angle {
		let snap_resolution = ROTATE_INCREMENT.to_radians();
		(angle / snap_resolution).round() * snap_resolution
	} else {
		angle
	};

	let delta = DAffine2::from_angle(snapped_angle);

	dragging_layers.retain(|layer| {
		if *layer != LayerNodeIdentifier::ROOT_PARENT {
			document.network_interface.document_network().nodes.contains_key(&layer.to_node())
		} else {
			log::error!("ROOT_PARENT should not be part of replacement_selected_layers");
			false
		}
	});
	let mut selected = Selected::new(
		&mut bounds.original_transforms,
		&mut bounds.center_of_transformation,
		&dragging_layers,
		responses,
		&document.network_interface,
		None,
		&tool,
		None,
	);

	selected.update_transforms(delta, None, None);
}

pub fn skew_bounds(
	document: &DocumentMessageHandler,
	responses: &mut VecDeque<Message>,
	bounds: &mut BoundingBoxManager,
	free_movement: bool,
	layers: &mut Vec<LayerNodeIdentifier>,
	mouse_position: DVec2,
	tool: ToolType,
) {
	if let Some(movement) = &mut bounds.selected_edges {
		let transformation = movement.skew_transform(mouse_position, bounds.original_bound_transform, free_movement);

		layers.retain(|layer| {
			if *layer != LayerNodeIdentifier::ROOT_PARENT {
				document.network_interface.document_network().nodes.contains_key(&layer.to_node())
			} else {
				log::error!("ROOT_PARENT should not be part of layers_dragging");
				false
			}
		});
		let selected = &layers;
		let mut pivot = DVec2::ZERO;
		let mut selected = Selected::new(&mut bounds.original_transforms, &mut pivot, selected, responses, &document.network_interface, None, &tool, None);

		selected.apply_transformation(bounds.original_bound_transform * transformation * bounds.original_bound_transform.inverse(), None);
	}
}

pub fn transforming_transform_cage(
	document: &DocumentMessageHandler,
	mut bounding_box_manager: &mut Option<BoundingBoxManager>,
	input: &InputPreprocessorMessageHandler,
	responses: &mut VecDeque<Message>,
	layers_dragging: &mut Vec<LayerNodeIdentifier>,
) -> (bool, bool, bool) {
	let dragging_bounds = bounding_box_manager.as_mut().and_then(|bounding_box| {
		let edges = bounding_box.check_selected_edges(input.mouse.position);

		bounding_box.selected_edges = edges.map(|(top, bottom, left, right)| {
			let selected_edges = SelectedEdges::new(top, bottom, left, right, bounding_box.bounds);
			bounding_box.opposite_pivot = selected_edges.calculate_pivot();
			selected_edges
		});

		edges
	});

	let rotating_bounds = bounding_box_manager.as_ref().map(|bounding_box| bounding_box.check_rotate(input.mouse.position)).unwrap_or_default();

	let selected: Vec<_> = document.network_interface.selected_nodes().selected_visible_and_unlocked_layers(&document.network_interface).collect();

	let is_flat_layer = bounding_box_manager.as_ref().map(|bounding_box_manager| bounding_box_manager.transform_tampered).unwrap_or(true);

	if dragging_bounds.is_some() && !is_flat_layer {
		responses.add(DocumentMessage::StartTransaction);

		*layers_dragging = selected;

		if let Some(bounds) = &mut bounding_box_manager {
			bounds.original_bound_transform = bounds.transform;

			layers_dragging.retain(|layer| {
				if *layer != LayerNodeIdentifier::ROOT_PARENT {
					document.network_interface.document_network().nodes.contains_key(&layer.to_node())
				} else {
					log::error!("ROOT_PARENT should not be part of layers_dragging");
					false
				}
			});

			let mut selected = Selected::new(
				&mut bounds.original_transforms,
				&mut bounds.center_of_transformation,
				&layers_dragging,
				responses,
				&document.network_interface,
				None,
				&ToolType::Select,
				None,
			);
			bounds.center_of_transformation = selected.mean_average_of_pivots();

			// Check if we're hovering over a skew triangle
			let edges = bounds.check_selected_edges(input.mouse.position);
			if let Some(edges) = edges {
				let closest_edge = bounds.get_closest_edge(edges, input.mouse.position);
				if bounds.check_skew_handle(input.mouse.position, closest_edge) {
					return (false, false, true);
				}
			}
		}
		return (true, false, false);
	}

	if rotating_bounds {
		responses.add(DocumentMessage::StartTransaction);

		if let Some(bounds) = &mut bounding_box_manager {
			layers_dragging.retain(|layer| {
				if *layer != LayerNodeIdentifier::ROOT_PARENT {
					document.network_interface.document_network().nodes.contains_key(&layer.to_node())
				} else {
					log::error!("ROOT_PARENT should not be part of layers_dragging");
					false
				}
			});

			let mut selected = Selected::new(
				&mut bounds.original_transforms,
				&mut bounds.center_of_transformation,
				&selected,
				responses,
				&document.network_interface,
				None,
				&ToolType::Select,
				None,
			);

			bounds.center_of_transformation = selected.mean_average_of_pivots();
		}

		*layers_dragging = selected;

		return (false, true, false);
	}

	return (false, false, false);
}

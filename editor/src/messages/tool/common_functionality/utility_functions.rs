use super::snap_session::{SnapSession, SnapView, anchor_positions};
use super::transformation_cage::{BoundingBoxManager, SizeSnapData};
use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{NodeNetworkInterface, OutputConnector};
use crate::messages::portfolio::document::utility_types::transformation::Selected;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils::{NodeGraphLayer, get_text};
use crate::messages::tool::common_functionality::transformation_cage::SelectedEdges;
use crate::messages::tool::tool_messages::path_tool::PathOverlayMode;
use crate::messages::tool::utility_types::ToolType;
use glam::{DAffine2, DMat2, DVec2};
use graph_craft::document::value::TaggedValue;
use graphene_std::renderer::Quad;
use graphene_std::vector::algorithms::util::pathseg_tangent;
use graphene_std::vector::misc::{HandleId, ManipulatorPointId, dvec2_to_point, point_to_dvec2};
use graphene_std::vector::{HandleExt, PointId, SegmentId, VectorGeometry, VectorModificationType};
use kurbo::{CubicBez, DEFAULT_ACCURACY, ParamCurve, ParamCurveDeriv, PathSeg, Shape};

/// Determines if a path should be extended. Goal in viewport space. Returns the path and if it is extending from the start, if applicable.
pub fn should_extend(document: &DocumentMessageHandler, goal: DVec2, tolerance: f64, layers: impl Iterator<Item = LayerNodeIdentifier>) -> Option<(LayerNodeIdentifier, PointId, DVec2)> {
	closest_point(document, goal, tolerance, layers, |_| false)
}

/// Finds the endpoint of an open path closest to the goal (in viewport space) across the given layers, if one lies within the tolerance.
/// Only anchors with a single connected segment qualify, so closed paths are never matched. Returns the endpoint's position in the layer's local space.
pub fn closest_open_path_endpoint(document: &DocumentMessageHandler, goal: DVec2, tolerance: f64, layers: impl Iterator<Item = LayerNodeIdentifier>) -> Option<(LayerNodeIdentifier, PointId, DVec2)> {
	closest_candidate_point(document, goal, tolerance, layers, |vector| vector.anchor_endpoints().collect())
}

/// Determine the closest point to the goal point under max_distance.
/// Additionally exclude checking closeness to the point which given to exclude() returns true.
pub fn closest_point<T>(
	document: &DocumentMessageHandler,
	goal: DVec2,
	max_distance: f64,
	layers: impl Iterator<Item = LayerNodeIdentifier>,
	exclude: T,
) -> Option<(LayerNodeIdentifier, PointId, DVec2)>
where
	T: Fn(PointId) -> bool,
{
	closest_candidate_point(document, goal, max_distance, layers, |vector| vector.anchor_points().filter(|&id| !exclude(id)).collect())
}

/// Determines the closest of each visible layer's candidate points to the goal (in viewport space) under max_distance. Returns the point's position in the layer's local space.
fn closest_candidate_point(
	document: &DocumentMessageHandler,
	goal: DVec2,
	max_distance: f64,
	layers: impl Iterator<Item = LayerNodeIdentifier>,
	candidates: impl Fn(&VectorGeometry) -> Vec<PointId>,
) -> Option<(LayerNodeIdentifier, PointId, DVec2)> {
	let mut best = None;
	let mut best_distance_squared = max_distance * max_distance;

	for layer in layers {
		if !document.network_interface.is_layer_visible(layer) {
			continue;
		}

		let viewspace = document.metadata().transform_to_viewport_if_feeds(layer, &document.network_interface);
		let Some(vector) = document.network_interface.compute_modified_vector(layer) else { continue };

		for id in candidates(&vector) {
			let Some(point) = vector.position_from_id(id) else { continue };

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
pub fn text_bounding_box(layer: LayerNodeIdentifier, document: &DocumentMessageHandler, fonts: &FontsMessageHandler, responses: &mut VecDeque<Message>) -> Quad {
	// Use the `editor:text_frame` attribute if available (handles multi-item glyphs and the 'Index Elements' node)
	if let Some(&frame) = document.metadata().text_frames.get(&layer) {
		return frame * Quad::from_box([DVec2::ZERO, DVec2::ONE]);
	}

	// Fallback: recompute from text content (e.g. layer hasn't rendered yet)
	let Some((text, font, typesetting)) = get_text(layer, &document.network_interface, fonts, &document.resources) else {
		return Quad::from_box([DVec2::ZERO, DVec2::ZERO]);
	};
	let font = fonts.get_resource_or_queue_load(&font, responses);
	let far = graphene_std::text::bounding_box(text, &font, typesetting, false);
	Quad::from_box([DVec2::ZERO, far])
}

pub fn calculate_segment_angle(anchor: PointId, segment: SegmentId, vector: &VectorGeometry, prefer_handle_direction: bool) -> Option<f64> {
	let is_start = |point: PointId, segment: SegmentId| vector.segment_start_from_id(segment) == Some(point);
	let anchor_position = vector.position_from_id(anchor)?;
	let end_handle = vector.position_of(ManipulatorPointId::EndHandle(segment));
	let start_handle = vector.position_of(ManipulatorPointId::PrimaryHandle(segment));

	let start_point = if is_start(anchor, segment) {
		vector.segment_end_from_id(segment).and_then(|id| vector.position_from_id(id))
	} else {
		vector.segment_start_from_id(segment).and_then(|id| vector.position_from_id(id))
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

	required_handle.map(|handle| handle - anchor_position).and_then(|vector| DVec2::X.try_angle_to(vector))
}

pub fn adjust_handle_colinearity(handle: HandleId, anchor_position: DVec2, target_control_point: DVec2, vector: &VectorGeometry, layer: LayerNodeIdentifier, responses: &mut VecDeque<Message>) {
	let Some(other_handle) = vector.other_colinear_handle(handle) else { return };
	let Some(handle_position) = vector.position_of(other_handle.to_manipulator_point()) else {
		return;
	};
	let Some(direction) = (anchor_position - target_control_point).try_normalize() else { return };

	let new_relative_position = (handle_position - anchor_position).length() * direction;
	let modification_type = other_handle.set_relative_position(new_relative_position);

	responses.add(GraphOperationMessage::Vector { layer, modification_type });
}

pub fn restore_previous_handle_position(
	handle: HandleId,
	original_c: DVec2,
	anchor_position: DVec2,
	vector: &VectorGeometry,
	layer: LayerNodeIdentifier,
	responses: &mut VecDeque<Message>,
) -> Option<HandleId> {
	let other_handle = vector.other_colinear_handle(handle)?;
	let handle_position = vector.position_of(other_handle.to_manipulator_point())?;
	let direction = (anchor_position - original_c).try_normalize()?;

	let old_relative_position = (handle_position - anchor_position).length() * direction;
	let modification_type = other_handle.set_relative_position(old_relative_position);
	responses.add(GraphOperationMessage::Vector { layer, modification_type });

	let handles = [handle, other_handle];
	let modification_type = VectorModificationType::SetG1Continuous { handles, enabled: false };
	responses.add(GraphOperationMessage::Vector { layer, modification_type });

	Some(other_handle)
}

pub fn restore_g1_continuity(
	handle: HandleId,
	other_handle: HandleId,
	control_point: DVec2,
	anchor_position: DVec2,
	vector: &VectorGeometry,
	layer: LayerNodeIdentifier,
	responses: &mut VecDeque<Message>,
) {
	let Some(handle_position) = vector.position_of(other_handle.to_manipulator_point()) else {
		return;
	};
	let Some(direction) = (anchor_position - control_point).try_normalize() else { return };

	let new_relative_position = (handle_position - anchor_position).length() * direction;
	let modification_type = other_handle.set_relative_position(new_relative_position);
	responses.add(GraphOperationMessage::Vector { layer, modification_type });

	let handles = [handle, other_handle];
	let modification_type = VectorModificationType::SetG1Continuous { handles, enabled: true };
	responses.add(GraphOperationMessage::Vector { layer, modification_type });
}

/// Check whether a point is visible in the current overlay mode.
pub fn is_visible_point(
	manipulator_point_id: ManipulatorPointId,
	vector: &VectorGeometry,
	path_overlay_mode: PathOverlayMode,
	frontier_handles_for_layer: Option<&HashMap<SegmentId, Vec<PointId>>>,
	selected_segments: &[SegmentId],
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
					let Some(handle_pair) = vector.handle_pair_of(manipulator_point_id) else { return false };
					let other_handle = handle_pair[1].to_manipulator_point();

					// Return whether the list of selected points contain the other handle
					selected_points.contains(&other_handle)
				}
				(PathOverlayMode::FrontierHandles, false) => {
					let Some(anchor) = vector.anchor_of(manipulator_point_id) else {
						warn!("No anchor for selected handle");
						return false;
					};
					let Some(frontier_handles) = frontier_handles_for_layer else {
						warn!("No frontier handles info provided");
						return false;
					};

					frontier_handles.get(&segment_id).map(|anchors| anchors.contains(&anchor)).unwrap_or_default()
				}
			}
		}
	}
}

/// The most anchors of the dragged layers that snap along with the box around them, which are those nearest where the pointer took hold.
const MAX_SNAPPING_ANCHORS: usize = 8;

/// The most visible layers, counting those inside dragged groups, for which a drag snaps their anchors as well as the box around them.
const MAX_LAYERS_WITH_SNAPPING_ANCHORS: usize = 8;

/// Adds the anchors of the dragged layers nearest the grip to where a gesture's features start, in document space, skipping any already there.
pub fn push_nearest_anchors(starts: &mut Vec<DVec2>, document: &DocumentMessageHandler, layers: &[LayerNodeIdentifier], grip: DVec2) {
	// Anchors far from the grip seldom decide where a drag lands, so only the nearest snap. O(n log n) in the anchors.
	let mut anchors = dragged_anchors(document, layers);
	anchors.sort_by(|a, b| a.distance_squared(grip).total_cmp(&b.distance_squared(grip)));

	let mut pushed = 0;
	for anchor in anchors {
		if pushed >= MAX_SNAPPING_ANCHORS {
			break;
		}
		if !starts.iter().any(|start| start.abs_diff_eq(anchor, 1e-6)) {
			starts.push(anchor);
			pushed += 1;
		}
	}
}

/// The anchors the dragged layers draw, in document space, when few enough layers are dragged for them to be worth snapping.
fn dragged_anchors(document: &DocumentMessageHandler, layers: &[LayerNodeIdentifier]) -> Vec<DVec2> {
	let metadata = document.metadata();
	let with_everything_inside = layers.iter().flat_map(|&layer| std::iter::once(layer).chain(layer.descendants(metadata)));
	let layers: Vec<_> = with_everything_inside
		.filter(|&layer| !layer.has_children(metadata) && document.network_interface.is_layer_visible(layer))
		.take(MAX_LAYERS_WITH_SNAPPING_ANCHORS + 1)
		.collect();
	if layers.len() > MAX_LAYERS_WITH_SNAPPING_ANCHORS {
		return Vec::new();
	}

	let mut anchors = Vec::new();
	for layer in layers {
		let to_document = metadata.transform_to_document_as_rendered(layer);
		for path in metadata.layer_outline(layer) {
			let drawn = anchor_positions(&path).map(|position| to_document.transform_point2(position));
			anchors.extend(drawn.filter(|position| position.is_finite()));
		}
	}
	anchors
}

#[allow(clippy::too_many_arguments)]
pub fn resize_bounds(
	view: SnapView,
	snap_session: &mut SnapSession,
	responses: &mut VecDeque<Message>,
	bounds: &mut BoundingBoxManager,
	dragging_layers: &mut Vec<LayerNodeIdentifier>,
	center: bool,
	constrain: bool,
	tool: ToolType,
) {
	let document = view.document;

	if let Some(movement) = &mut bounds.selected_edges {
		let center = center.then_some(bounds.center_of_transformation);
		let snap = Some(SizeSnapData { session: snap_session, view });
		let (position, size) = movement.new_size(view.input.mouse.position, bounds.original_bound_transform, center, constrain, snap);
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

		let mut selected = Selected::new(&mut bounds.original_transforms, &mut pivot, dragging_layers, responses, &document.network_interface, None, &tool, None);
		selected.apply_transformation(bounds.original_bound_transform * transformation * bounds.original_bound_transform.inverse());
	}
}

#[allow(clippy::too_many_arguments)]
pub fn rotate_bounds(
	view: SnapView,
	snap_session: &mut SnapSession,
	responses: &mut VecDeque<Message>,
	bounds: &mut BoundingBoxManager,
	dragging_layers: &mut Vec<LayerNodeIdentifier>,
	drag_start: DVec2,
	snap_angle: bool,
	tool: ToolType,
) {
	let document = view.document;
	let pointer_angle = || {
		let start_offset = drag_start - bounds.center_of_transformation;
		let end_offset = view.input.mouse.position - bounds.center_of_transformation;
		start_offset.try_angle_to(end_offset)
	};

	// A stepped angle comes from the pointer alone, with nothing snapped
	let angle = if snap_angle {
		snap_session.forget();
		let snap_resolution = document.snapping_state.angle_step_radians();
		pointer_angle().map(|angle| (angle / snap_resolution).round() * snap_resolution)
	} else {
		bounds.snap_turn(view, snap_session).or_else(pointer_angle)
	};
	let Some(angle) = angle else {
		warn!("Unable to rotate bounds due to zero offset");
		return;
	};

	let delta = DAffine2::from_angle(angle);

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
		dragging_layers,
		responses,
		&document.network_interface,
		None,
		&tool,
		None,
	);
	selected.update_transforms(delta, None);
}

pub fn skew_bounds(
	view: SnapView,
	snap_session: &mut SnapSession,
	responses: &mut VecDeque<Message>,
	bounds: &mut BoundingBoxManager,
	free_movement: bool,
	layers: &mut Vec<LayerNodeIdentifier>,
	tool: ToolType,
) {
	let document = view.document;
	let mouse_position = bounds.snap_skew(view, snap_session, free_movement);

	if let Some(movement) = &mut bounds.selected_edges {
		let mut pivot = DVec2::ZERO;

		let transformation = movement.skew_transform(mouse_position, bounds.original_bound_transform, free_movement);

		layers.retain(|layer| {
			if *layer != LayerNodeIdentifier::ROOT_PARENT {
				document.network_interface.document_network().nodes.contains_key(&layer.to_node())
			} else {
				log::error!("ROOT_PARENT should not be part of layers_dragging");
				false
			}
		});

		let mut selected = Selected::new(&mut bounds.original_transforms, &mut pivot, layers, responses, &document.network_interface, None, &tool, None);
		selected.apply_transformation(bounds.original_bound_transform * transformation * bounds.original_bound_transform.inverse());
	}
}

// TODO: Replace returned tuple (where at most 1 element is true at a time) with an enum.
/// Returns the tuple (resize, rotate, skew).
pub fn transforming_transform_cage(
	document: &DocumentMessageHandler,
	mut bounding_box_manager: &mut Option<BoundingBoxManager>,
	input: &InputPreprocessorMessageHandler,
	responses: &mut VecDeque<Message>,
	layers_dragging: &mut Vec<LayerNodeIdentifier>,
	center_of_transformation: Option<DVec2>,
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

			// The resized layers' anchors nearest the grab snap along with the box, taken from where they are before the resize
			if let Some(edges) = &mut bounds.selected_edges {
				let document_to_viewport = document.metadata().document_to_viewport;
				let grab = document_to_viewport.inverse().transform_point2(input.mouse.position);
				let mut anchors = Vec::new();
				push_nearest_anchors(&mut anchors, document, layers_dragging, grab);

				let document_to_cage = bounds.transform.inverse() * document_to_viewport;
				edges.anchors = anchors.into_iter().map(|anchor| document_to_cage.transform_point2(anchor)).collect();
			}

			layers_dragging.retain(|layer| {
				if *layer != LayerNodeIdentifier::ROOT_PARENT {
					document.network_interface.document_network().nodes.contains_key(&layer.to_node())
				} else {
					log::error!("ROOT_PARENT should not be part of layers_dragging");
					false
				}
			});

			bounds.center_of_transformation = center_of_transformation.unwrap_or_else(|| {
				document
					.network_interface
					.selected_nodes()
					.selected_visible_and_unlocked_layers_mean_average_origin(&document.network_interface)
			});

			// Check if we're hovering over a skew triangle
			let edges = bounds.check_selected_edges(input.mouse.position);
			if let Some(edges) = edges {
				let closest_edge = bounds.get_closest_edge(edges, input.mouse.position);
				if bounds.check_skew_handle(input.mouse.position, closest_edge) {
					// No resize or rotate, just skew
					return (false, false, true);
				}
			}
		}

		// Just resize, no rotate or skew
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

			bounds.center_of_transformation = center_of_transformation.unwrap_or_else(|| {
				document
					.network_interface
					.selected_nodes()
					.selected_visible_and_unlocked_layers_mean_average_origin(&document.network_interface)
			});
		}

		*layers_dragging = selected;

		// No resize or skew, just rotate
		return (false, true, false);
	}

	// No resize, rotate, or skew
	(false, false, false)
}

/// Points sampled along each of the two segments that [`fit_handles_to_segment_pair`] fits.
const SEGMENT_PAIR_FIT_SAMPLES: usize = 16;
/// Passes in [`fit_handles_to_segment_pair`] that each move the samples to their closest points on the fitted cubic and refit it.
const SEGMENT_PAIR_FIT_PASSES: usize = 12;
/// How much a sample's error along the fitted cubic counts relative to its distance from it, which damps each pass.
const SEGMENT_PAIR_FIT_TANGENTIAL_WEIGHT: f64 = 0.01;

/// Finds the handles (relative to their anchors) that make the cubic from `start` to `end` best match `first_segment` followed by `second_segment`.
/// The handles keep the given directions. The fit minimizes the squared distance from the segments to the cubic, so a pair split from one cubic is rebuilt exactly.
pub fn fit_handles_to_segment_pair(start: DVec2, end: DVec2, start_direction: DVec2, end_direction: DVec2, first_segment: PathSeg, second_segment: PathSeg) -> (DVec2, DVec2) {
	let start_direction = start_direction.try_normalize().unwrap_or_default();
	let end_direction = end_direction.try_normalize().unwrap_or_default();

	// Orient the segments to run from `start` through the anchor they share to `end`
	let first_segment = if first_segment.start().distance(dvec2_to_point(start)) > first_segment.end().distance(dvec2_to_point(start)) {
		first_segment.reverse()
	} else {
		first_segment
	};
	let second_segment = if second_segment.end().distance(dvec2_to_point(end)) > second_segment.start().distance(dvec2_to_point(end)) {
		second_segment.reverse()
	} else {
		second_segment
	};

	// Each segment's samples count in proportion to its share of the total length
	let first_length = first_segment.perimeter(DEFAULT_ACCURACY);
	let second_length = second_segment.perimeter(DEFAULT_ACCURACY);
	let first_weight = if first_length + second_length > 0. { first_length / (first_length + second_length) } else { 1. };

	// Start each sample at its parameter on the cubic the pair would have been split from, whose split is where the segments' speeds match
	let first_speed = pathseg_tangent(first_segment, 1.).length();
	let second_speed = pathseg_tangent(second_segment, 0.).length();
	let joint = if first_speed > 0. { first_speed / (first_speed + second_speed) } else { first_weight };
	let mut samples = (1..=SEGMENT_PAIR_FIT_SAMPLES)
		.flat_map(|i| {
			let t = i as f64 / SEGMENT_PAIR_FIT_SAMPLES as f64;
			[
				(joint * t, point_to_dvec2(first_segment.eval(t)), first_weight),
				(joint + (1. - joint) * t, point_to_dvec2(second_segment.eval(t)), 1. - first_weight),
			]
		})
		.collect::<Vec<_>>();

	// Weighted least squares for the handle lengths, which the cubic is linear in, so this solves the 2x2 normal equations directly.
	// Error is measured from each sample to its parameter's point on the cubic, mostly perpendicular to `fitted` there once a fit exists.
	let solve = |samples: &[(f64, DVec2, f64)], fitted: Option<CubicBez>| -> DVec2 {
		let fitted_derivative = fitted.map(|fitted| fitted.deriv());
		let mut system = DMat2::ZERO;
		let mut right_side = DVec2::ZERO;

		for &(parameter, target, weight) in samples {
			// Split the Bernstein form into the part fixed by the anchors and the parts that scale with each handle length
			let complement = 1. - parameter;
			let start_basis = 3. * complement * complement * parameter;
			let end_basis = 3. * complement * parameter * parameter;
			let fixed = (complement * complement * complement + start_basis) * start + (end_basis + parameter * parameter * parameter) * end;
			let jacobian = DMat2::from_cols(start_basis * start_direction, end_basis * end_direction);

			let metric = match fitted_derivative {
				Some(fitted_derivative) => {
					let tangent = point_to_dvec2(fitted_derivative.eval(parameter)).try_normalize().unwrap_or(DVec2::X);
					let normal = tangent.perp();
					DMat2::from_cols(normal * normal.x, normal * normal.y) + DMat2::from_cols(tangent * tangent.x, tangent * tangent.y) * SEGMENT_PAIR_FIT_TANGENTIAL_WEIGHT
				}
				None => DMat2::IDENTITY,
			} * weight;

			system += jacobian.transpose() * metric * jacobian;
			right_side += jacobian.transpose() * metric * (target - fixed);
		}

		// Use the unconstrained optimum, unless it gives a handle negative length
		if system.determinant() > f64::EPSILON * system.x_axis.x * system.y_axis.y {
			let lengths = system.inverse() * right_side;
			if lengths.x >= 0. && lengths.y >= 0. {
				return lengths;
			}
		}

		// Otherwise the optimum collapses one handle and gives the other its best length from there
		let start_only = DVec2::new(if system.x_axis.x > 0. { (right_side.x / system.x_axis.x).max(0.) } else { 0. }, 0.);
		let end_only = DVec2::new(0., if system.y_axis.y > 0. { (right_side.y / system.y_axis.y).max(0.) } else { 0. });
		let error = |lengths: DVec2| lengths.dot(system * lengths) - 2. * right_side.dot(lengths);
		if error(start_only) <= error(end_only) { start_only } else { end_only }
	};

	let mut lengths = solve(&samples, None);

	// Tangent distance minimization: Gauss-Newton on each sample's distance to the cubic, rather than to its parameter's point
	for _ in 0..SEGMENT_PAIR_FIT_PASSES {
		let fitted = CubicBez::new(
			dvec2_to_point(start),
			dvec2_to_point(start + start_direction * lengths.x),
			dvec2_to_point(end + end_direction * lengths.y),
			dvec2_to_point(end),
		);
		let derivative = fitted.deriv();

		// Move each sample's parameter toward its closest point on the cubic
		for (parameter, target, _) in &mut samples {
			for _ in 0..2 {
				let offset = point_to_dvec2(fitted.eval(*parameter)) - *target;
				let tangent = point_to_dvec2(derivative.eval(*parameter));
				if tangent.length_squared() > 0. {
					*parameter = (*parameter - offset.dot(tangent) / tangent.length_squared()).clamp(0., 1.);
				}
			}
		}

		lengths = solve(&samples, Some(fitted));
	}

	(start_direction * lengths.x, end_direction * lengths.y)
}

pub fn make_path_editable_is_allowed(network_interface: &mut NodeNetworkInterface) -> Option<LayerNodeIdentifier> {
	// Must have exactly one layer selected
	let selected_nodes = network_interface.selected_nodes();
	let mut selected_layers = selected_nodes.selected_layers(network_interface.document_metadata());
	let first_layer = selected_layers.next()?;
	if selected_layers.next().is_some() {
		return None;
	}
	for _ in selected_layers {}

	// Must be a vector layer, at either rank
	let node_id = NodeGraphLayer::new(first_layer, network_interface).horizontal_layer_flow().nth(1)?;

	let output_type = network_interface.output_type(&OutputConnector::primary_output(node_id), &[]);
	if output_type.compiled_element_name().as_deref() != Some("Vector") {
		return None;
	}

	// Must not already have an existing Path node, in the right-most part of the layer chain, which has an empty set of modifications
	// (otherwise users could repeatedly keep running this command and stacking up empty Path nodes)
	if let Some(TaggedValue::VectorModification(modifications)) = NodeGraphLayer::new(first_layer, network_interface).find_input(&DefinitionIdentifier::Network("Path".into()), 1)
		&& modifications.is_empty()
	{
		return None;
	}

	Some(first_layer)
}

/// Smallest extent, in document units, a nudge-resized box may have per axis (avoids a zero divisor and prevents collapse/inversion).
const NUDGE_RESIZE_MIN_EXTENT: f64 = 1.;

/// The resized box corners and the document-space scale transform produced by [`nudge_resize_bounds`].
pub struct NudgeResize {
	pub min: DVec2,
	pub max: DVec2,
	pub transform: DAffine2,
}

/// Resizes the axis-aligned document-space box `[min, max]` by an arrow-key nudge `delta` (screen space).
///
/// `tilt` (document rotation) is snapped to a quarter turn so the arrow maps onto a box axis. The top-left corner is anchored by default,
/// or the bottom-right corner when `resize_opposite` (Control) is held; the other corner moves. The box stays at least one unit per axis.
pub fn nudge_resize_bounds(min: DVec2, max: DVec2, delta: DVec2, tilt: f64, resize_opposite: bool) -> NudgeResize {
	// Snap rotation to a quarter turn so the screen arrow lands on a document-space box axis
	let doc_delta = match ((tilt / std::f64::consts::FRAC_PI_2).round() as i32).rem_euclid(4) {
		1 => DVec2::new(delta.y, -delta.x),
		2 => -delta,
		3 => DVec2::new(-delta.y, delta.x),
		_ => delta,
	};

	// Move one corner by the arrow, keeping the other (the anchor) at least the minimum extent away
	let mut new_min = min;
	let mut new_max = max;
	if resize_opposite {
		new_min += doc_delta;
		new_min = new_min.min(new_max - DVec2::splat(NUDGE_RESIZE_MIN_EXTENT));
	} else {
		new_max += doc_delta;
		new_max = new_max.max(new_min + DVec2::splat(NUDGE_RESIZE_MIN_EXTENT));
	}

	// Ratio of new to old extent, treating a degenerate (sub-unit) original extent as unscaled
	let old_extent = max - min;
	let new_extent = new_max - new_min;
	let scale = DVec2::new(
		if old_extent.x.abs() < NUDGE_RESIZE_MIN_EXTENT { 1. } else { new_extent.x / old_extent.x },
		if old_extent.y.abs() < NUDGE_RESIZE_MIN_EXTENT { 1. } else { new_extent.y / old_extent.y },
	);

	// Scale about the anchored corner, guarding non-finite components to zero
	let anchor = if resize_opposite { max } else { min };
	let scale_center = DVec2::new(if anchor.x.is_finite() { anchor.x } else { 0. }, if anchor.y.is_finite() { anchor.y } else { 0. });
	let transform = DAffine2::from_scale_angle_translation(scale, 0., scale_center - scale * scale_center);

	NudgeResize {
		min: new_min,
		max: new_max,
		transform,
	}
}

pub fn is_almost_colinear(point: DVec2, handle1: DVec2, handle2: DVec2) -> bool {
	let to_handle1 = handle1 - point;
	let to_handle2 = handle2 - point;
	// Take the angle to the negated other handle in the range `[-π, +π]`.
	let Some(angle) = to_handle1.try_angle_to(-to_handle2) else {
		// If it is impossible to take an angle (one of the handles is zero length) return true since that counts also as colinear.
		return true;
	};
	// An angle of zero means they are perfectly opposite.
	angle.abs().abs() < 1e-6
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::test_utils::test_prelude::*;

	#[tokio::test]
	async fn hidden_layers_neither_offer_anchors_nor_count_toward_the_cutoff_for_snapping_them() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		// One more rectangle than the cutoff allows, the last of which is then hidden
		let mut layers = Vec::new();
		let mut left = 100.;
		for _ in 0..=MAX_LAYERS_WITH_SNAPPING_ANCHORS {
			editor.draw_rect(left, 100., left + 60., 160.).await;
			layers.push(editor.get_selected_layer().await.expect("the rectangle should be selected"));
			left += 80.;
		}
		let hide = NodeGraphMessage::SetVisibility {
			node_id: layers[MAX_LAYERS_WITH_SNAPPING_ANCHORS].to_node(),
			network_path: Vec::new(),
			visible: false,
		};
		editor.handle_message(hide).await;

		let mut starts = Vec::new();
		push_nearest_anchors(&mut starts, editor.active_document(), &layers, DVec2::new(100., 100.));
		assert!(!starts.is_empty(), "the visible rectangles' anchors should snap");
	}

	#[test]
	fn fitting_a_segment_pair_rebuilds_the_cubic_it_was_split_from() {
		let original = CubicBez::new((0., 0.), (0., -400.), (600., -400.), (600., 0.));

		// An anchor inserted along the cubic and slid further along it leaves two segments that one cubic covers exactly
		for (inserted, slid) in [(0.1, 0.15), (0.3, 0.6), (0.5, 0.95)] {
			let further_segment = PathSeg::Cubic(original.subsegment(0. ..inserted));
			let absorbed_segment = PathSeg::Cubic(original.subsegment(inserted..slid));
			let expected = original.subsegment(0. ..slid);

			let start = point_to_dvec2(expected.p0);
			let end = point_to_dvec2(expected.p3);
			let (start_handle, end_handle) = fit_handles_to_segment_pair(start, end, point_to_dvec2(expected.p1) - start, point_to_dvec2(expected.p2) - end, further_segment, absorbed_segment);

			assert!((start + start_handle).abs_diff_eq(point_to_dvec2(expected.p1), 1e-6), "start handle for {inserted} slid to {slid}");
			assert!((end + end_handle).abs_diff_eq(point_to_dvec2(expected.p2), 1e-6), "end handle for {inserted} slid to {slid}");
		}
	}

	#[test]
	fn is_almost_colinear_test() {
		assert!(!is_almost_colinear(DVec2::ZERO, DVec2::X, DVec2::X));
		assert!(!is_almost_colinear(DVec2::ZERO, DVec2::Y, DVec2::X));
		assert!(is_almost_colinear(DVec2::ZERO, DVec2::NEG_X, DVec2::X));
		assert!(is_almost_colinear(DVec2::ZERO, DVec2::ZERO, DVec2::X));
		assert!(is_almost_colinear(DVec2::ZERO, DVec2::ZERO, DVec2::ZERO));
	}
}

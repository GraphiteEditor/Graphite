use super::snapping::{SnapCandidatePoint, SnapData, SnapManager};
use super::transformation_cage::{BoundingBoxManager, SizeSnapData};
use crate::consts::ROTATE_INCREMENT;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{NodeNetworkInterface, OutputConnector};
use crate::messages::portfolio::document::utility_types::transformation::Selected;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils::{NodeGraphLayer, get_text};
use crate::messages::tool::common_functionality::transformation_cage::SelectedEdges;
use crate::messages::tool::tool_messages::path_tool::PathOverlayMode;
use crate::messages::tool::utility_types::ToolType;
use glam::{DAffine2, DVec2};
use graph_craft::concrete;
use graph_craft::document::value::TaggedValue;
use graphene_std::renderer::Quad;
use graphene_std::subpath::{Bezier, BezierHandles};
use graphene_std::table::Table;
use graphene_std::text::FontCache;
use graphene_std::vector::algorithms::bezpath_algorithms::pathseg_compute_lookup_table;
use graphene_std::vector::misc::{HandleId, ManipulatorPointId, dvec2_to_point};
use graphene_std::vector::{HandleExt, PointId, SegmentId, Vector, VectorModification, VectorModificationType};
use kurbo::{CubicBez, DEFAULT_ACCURACY, Line, ParamCurve, PathSeg, Point, QuadBez, Shape};

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
		let Some(vector) = document.network_interface.compute_modified_vector(layer) else { continue };
		for id in vector.extendable_points(preferences.vector_meshes) {
			if exclude(id) {
				continue;
			}
			let Some(point) = vector.point_domain.position_from_id(id) else { continue };

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
	let Some((text, font, typesetting, per_glyph_instances)) = get_text(layer, &document.network_interface) else {
		return Quad::from_box([DVec2::ZERO, DVec2::ZERO]);
	};

	let far = graphene_std::text::bounding_box(text, font, font_cache, typesetting, false);

	// TODO: Once the instance tables refactor is complete and per_glyph_instances can be removed (since it'll be the default),
	// TODO: remove this because the top of the dashed bounding overlay should no longer be based on the first line's baseline.
	let vertical_offset = if per_glyph_instances {
		DVec2::NEG_Y * typesetting.font_size * (1. + (typesetting.line_height_ratio - 1.) / 2.)
	} else {
		DVec2::ZERO
	};

	Quad::from_box([DVec2::ZERO + vertical_offset, far + vertical_offset])
}

pub fn calculate_segment_angle(anchor: PointId, segment: SegmentId, vector: &Vector, prefer_handle_direction: bool) -> Option<f64> {
	let is_start = |point: PointId, segment: SegmentId| vector.segment_start_from_id(segment) == Some(point);
	let anchor_position = vector.point_domain.position_from_id(anchor)?;
	let end_handle = ManipulatorPointId::EndHandle(segment).get_position(vector);
	let start_handle = ManipulatorPointId::PrimaryHandle(segment).get_position(vector);

	let start_point = if is_start(anchor, segment) {
		vector.segment_end_from_id(segment).and_then(|id| vector.point_domain.position_from_id(id))
	} else {
		vector.segment_start_from_id(segment).and_then(|id| vector.point_domain.position_from_id(id))
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

pub fn adjust_handle_colinearity(handle: HandleId, anchor_position: DVec2, target_control_point: DVec2, vector: &Vector, layer: LayerNodeIdentifier, responses: &mut VecDeque<Message>) {
	let Some(other_handle) = vector.other_colinear_handle(handle) else { return };
	let Some(handle_position) = other_handle.to_manipulator_point().get_position(vector) else {
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
	vector: &Vector,
	layer: LayerNodeIdentifier,
	responses: &mut VecDeque<Message>,
) -> Option<HandleId> {
	let other_handle = vector.other_colinear_handle(handle)?;
	let handle_position = other_handle.to_manipulator_point().get_position(vector)?;
	let direction = (anchor_position - original_c).try_normalize()?;

	let old_relative_position = (handle_position - anchor_position).length() * direction;
	let modification_type = other_handle.set_relative_position(old_relative_position);
	responses.add(GraphOperationMessage::Vector { layer, modification_type });

	let handles = [handle, other_handle];
	let modification_type = VectorModificationType::SetG1Continuous { handles, enabled: false };
	responses.add(GraphOperationMessage::Vector { layer, modification_type });

	Some(other_handle)
}

pub fn restore_g1_continuity(handle: HandleId, other_handle: HandleId, control_point: DVec2, anchor_position: DVec2, vector: &Vector, layer: LayerNodeIdentifier, responses: &mut VecDeque<Message>) {
	let Some(handle_position) = other_handle.to_manipulator_point().get_position(vector) else {
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
	vector: &Vector,
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
					let Some(handle_pair) = manipulator_point_id.get_handle_pair(vector) else { return false };
					let other_handle = handle_pair[1].to_manipulator_point();

					// Return whether the list of selected points contain the other handle
					selected_points.contains(&other_handle)
				}
				(PathOverlayMode::FrontierHandles, false) => {
					let Some(anchor) = manipulator_point_id.get_anchor(vector) else {
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

pub fn is_intersecting(bezier: Bezier, quad: [DVec2; 2], transform: DAffine2) -> bool {
	let to_layerspace = transform.inverse();
	let quad = [to_layerspace.transform_point2(quad[0]), to_layerspace.transform_point2(quad[1])];
	let start = Point::new(bezier.start.x, bezier.start.y);
	let end = Point::new(bezier.end.x, bezier.end.y);
	let segment = match bezier.handles {
		BezierHandles::Cubic { handle_start, handle_end } => {
			let p1 = Point::new(handle_start.x, handle_start.y);
			let p2 = Point::new(handle_end.x, handle_end.y);
			PathSeg::Cubic(CubicBez::new(start, p1, p2, end))
		}
		BezierHandles::Quadratic { handle } => {
			let p1 = Point::new(handle.x, handle.y);
			PathSeg::Quad(QuadBez::new(start, p1, end))
		}
		BezierHandles::Linear => PathSeg::Line(Line::new(start, end)),
	};

	// Create a list of all the sides
	let sides = [
		Line::new((quad[0].x, quad[0].y), (quad[1].x, quad[0].y)),
		Line::new((quad[0].x, quad[0].y), (quad[0].x, quad[1].y)),
		Line::new((quad[1].x, quad[1].y), (quad[1].x, quad[0].y)),
		Line::new((quad[1].x, quad[1].y), (quad[0].x, quad[1].y)),
	];

	let mut is_intersecting = false;
	for line in sides {
		let intersections = segment.intersect_line(line);
		let mut intersects = false;
		for intersection in intersections {
			if intersection.line_t <= 1. && intersection.line_t >= 0. && intersection.segment_t <= 1. && intersection.segment_t >= 0. {
				// There is a valid intersection point
				intersects = true;
				break;
			}
		}
		if intersects {
			is_intersecting = true;
			break;
		}
	}
	is_intersecting
}

#[allow(clippy::too_many_arguments)]
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
			snap_data: SnapData::ignore(document, input, dragging_layers),
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

		let mut selected = Selected::new(&mut bounds.original_transforms, &mut pivot, dragging_layers, responses, &document.network_interface, None, &tool, None);
		selected.apply_transformation(bounds.original_bound_transform * transformation * bounds.original_bound_transform.inverse(), None);
	}
}

#[allow(clippy::too_many_arguments)]
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
		dragging_layers,
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
		selected.apply_transformation(bounds.original_bound_transform * transformation * bounds.original_bound_transform.inverse(), None);
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

/// Calculates similarity metric between new bezier curve and two old beziers by using sampled points.
#[allow(clippy::too_many_arguments)]
pub fn log_optimization(a: f64, b: f64, p1: DVec2, p3: DVec2, d1: DVec2, d2: DVec2, points1: &[DVec2], n: usize) -> f64 {
	let start_handle_length = a.exp();
	let end_handle_length = b.exp();

	// Compute the handle positions of new bezier curve
	let c1 = p1 + d1 * start_handle_length;
	let c2 = p3 + d2 * end_handle_length;

	let new_curve = PathSeg::Cubic(CubicBez::new(Point::new(p1.x, p1.y), Point::new(c1.x, c1.y), Point::new(c2.x, c2.y), Point::new(p3.x, p3.y)));

	// Sample 2*n points from new curve and get the L2 metric between all of points
	let points = pathseg_compute_lookup_table(new_curve, Some(2 * n), false);

	let dist = points1.iter().zip(points).map(|(p1, p2)| (p1.x - p2.x).powi(2) + (p1.y - p2.y).powi(2)).sum::<f64>();

	dist / (2 * n) as f64
}

/// Calculates optimal handle lengths with adam optimization.
#[allow(clippy::too_many_arguments)]
pub fn find_two_param_best_approximate(p1: DVec2, p3: DVec2, d1: DVec2, d2: DVec2, min_len1: f64, min_len2: f64, further_segment: PathSeg, other_segment: PathSeg) -> (DVec2, DVec2) {
	let h = 1e-6;
	let tol = 1e-6;
	let max_iter = 200;

	let mut a = (5_f64).ln();
	let mut b = (5_f64).ln();

	let mut m_a = 0.;
	let mut v_a = 0.;
	let mut m_b = 0.;
	let mut v_b = 0.;

	let initial_alpha = 0.05;
	let decay_rate: f64 = 0.99;

	let beta1 = 0.9;
	let beta2 = 0.999;
	let epsilon = 1e-8;

	let n = 20;

	let further_segment = if further_segment.start().distance(dvec2_to_point(p1)) >= f64::EPSILON {
		further_segment.reverse()
	} else {
		further_segment
	};

	let other_segment = if other_segment.end().distance(dvec2_to_point(p3)) >= f64::EPSILON {
		other_segment.reverse()
	} else {
		other_segment
	};

	// Now we sample points proportional to the lengths of the beziers
	let l1 = further_segment.perimeter(DEFAULT_ACCURACY);
	let l2 = other_segment.perimeter(DEFAULT_ACCURACY);
	let ratio = l1 / (l1 + l2);
	let n_points1 = ((2 * n) as f64 * ratio).floor() as usize;
	let mut points1 = pathseg_compute_lookup_table(further_segment, Some(n_points1), false).collect::<Vec<_>>();
	let mut points2 = pathseg_compute_lookup_table(other_segment, Some(n), false).collect::<Vec<_>>();
	points1.append(&mut points2);

	let f = |a: f64, b: f64| -> f64 { log_optimization(a, b, p1, p3, d1, d2, &points1, n) };

	for t in 1..=max_iter {
		let dfa = (f(a + h, b) - f(a - h, b)) / (2. * h);
		let dfb = (f(a, b + h) - f(a, b - h)) / (2. * h);

		m_a = beta1 * m_a + (1. - beta1) * dfa;
		m_b = beta1 * m_b + (1. - beta1) * dfb;

		v_a = beta2 * v_a + (1. - beta2) * dfa * dfa;
		v_b = beta2 * v_b + (1. - beta2) * dfb * dfb;

		let m_a_hat = m_a / (1. - beta1.powi(t));
		let v_a_hat = v_a / (1. - beta2.powi(t));
		let m_b_hat = m_b / (1. - beta1.powi(t));
		let v_b_hat = v_b / (1. - beta2.powi(t));

		let alpha_t = initial_alpha * decay_rate.powi(t);

		// Update log-lengths
		a -= alpha_t * m_a_hat / (v_a_hat.sqrt() + epsilon);
		b -= alpha_t * m_b_hat / (v_b_hat.sqrt() + epsilon);

		// Convergence check
		if dfa.abs() < tol && dfb.abs() < tol {
			break;
		}
	}

	let len1 = a.exp().max(min_len1);
	let len2 = b.exp().max(min_len2);

	(d1 * len1, d2 * len2)
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

	// Must be a layer of type Table<Vector>
	let node_id = NodeGraphLayer::new(first_layer, network_interface).horizontal_layer_flow().nth(1)?;

	let (output_type, _) = network_interface.output_type(&OutputConnector::node(node_id, 0), &[]);
	if output_type.nested_type() != concrete!(Table<Vector>).nested_type() {
		return None;
	}

	// Must not already have an existing Path node, in the right-most part of the layer chain, which has an empty set of modifications
	// (otherwise users could repeatedly keep running this command and stacking up empty Path nodes)
	if let Some(TaggedValue::VectorModification(modifications)) = NodeGraphLayer::new(first_layer, network_interface).find_input("Path", 1) {
		if modifications.as_ref() == &VectorModification::default() {
			return None;
		}
	}

	Some(first_layer)
}

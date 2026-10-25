use super::utility_types::{DrawHandles, OverlayContext, Pivot};
use crate::consts::{
	COLOR_OVERLAY_BLUE, COLOR_OVERLAY_BLUE_50, COLOR_OVERLAY_MAGENTA, CURVATURE_COMB_MAX_TEETH_PER_SEGMENT, CURVATURE_COMB_MAX_TOOTH_LENGTH, CURVATURE_COMB_TOOTH_SPACING, HIDE_HANDLE_DISTANCE,
	INDEX_LABEL_GAP, PATH_DIRECTION_ARC_LENGTH_ACCURACY, PATH_DIRECTION_ARROW_SPACING, PATH_DIRECTION_CHEVRON_SIZE, PATH_DIRECTION_HARPOON_OFFSET, PATH_END_MARKER_DISTANCE, PATH_END_MARKER_SIZE,
	PATH_FLOW_JOIN_TICK_LENGTH, PATH_FLOW_JOIN_TICK_THICKNESS, PATH_INDEX_LABEL_OFFSET, PATH_MARKERS_MIN_LENGTH, POINT_INDEX_LABEL_OFFSET, SNAP_POINT_TOLERANCE,
};
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::NodeNetworkInterface;
pub use crate::messages::portfolio::document::utility_types::text_metrics::text_width;
use crate::messages::tool::common_functionality::shape_editor::{SelectedLayerState, ShapeState};
use crate::messages::tool::common_functionality::utility_functions::closest_open_path_endpoint;
use crate::messages::tool::tool_messages::tool_prelude::DocumentMessageHandler;
use glam::{DAffine2, DVec2};
use graphene_std::vector::algorithms::util::pathseg_tangent;
use graphene_std::vector::misc::{BezierHandles, ManipulatorPointId, PathSegPoints, dvec2_to_point, pathseg_points, point_to_dvec2, segment_to_handles};
use graphene_std::vector::{PointId, SegmentId, VectorGeometry, curve_type_at, handle_point, segment_id_of, span_endpoints, spans};
use kurbo::{Affine, BezPath, ParamCurve, ParamCurveArclen, ParamCurveArea, ParamCurveDeriv, PathSeg, Rect, Shape};
use std::collections::{HashMap, HashSet};
#[cfg(target_family = "wasm")]
use wasm_bindgen::JsCast;

#[cfg(target_family = "wasm")]
pub fn overlay_canvas_element() -> Option<web_sys::HtmlCanvasElement> {
	let window = web_sys::window()?;
	let document = window.document()?;
	let canvas = document.query_selector("[data-overlays-canvas]").ok().flatten()?;
	canvas.dyn_into::<web_sys::HtmlCanvasElement>().ok()
}

#[cfg(target_family = "wasm")]
pub fn overlay_canvas_context() -> web_sys::CanvasRenderingContext2d {
	let create_context = || {
		let context = overlay_canvas_element()?.get_context("2d").ok().flatten()?;
		context.dyn_into().ok()
	};
	create_context().expect("Failed to get canvas context")
}

pub fn selected_segments(network_interface: &NodeNetworkInterface, shape_editor: &ShapeState) -> HashMap<LayerNodeIdentifier, Vec<SegmentId>> {
	let mut map = HashMap::new();

	for (layer, state) in &shape_editor.selected_shape_state {
		let Some(vector) = network_interface.compute_modified_vector(*layer) else { continue };
		let selected_segments = selected_segments_for_layer(&vector, state);

		map.insert(*layer, selected_segments);
	}

	map
}

pub fn selected_segments_for_layer(vector: &VectorGeometry, state: &SelectedLayerState) -> Vec<SegmentId> {
	let selected_anchors = state
		.selected_points()
		.filter_map(|point| if let ManipulatorPointId::Anchor(p) = point { Some(p) } else { None })
		.collect::<Vec<_>>();

	// Collect the segments whose handles are selected
	let mut selected_segments = state
		.selected_points()
		.filter_map(|point_id| match point_id {
			ManipulatorPointId::PrimaryHandle(segment_id) | ManipulatorPointId::EndHandle(segment_id) => Some(segment_id),
			ManipulatorPointId::Anchor(_) => None,
		})
		.collect::<Vec<_>>();

	// Adding segments which are are connected to selected anchors
	for (segment_id, _, start, end) in vector.segment_iter() {
		if selected_anchors.contains(&start) || selected_anchors.contains(&end) {
			selected_segments.push(segment_id);
		}
	}
	selected_segments
}

/// A handle drawn by an overlay, in viewport space.
#[derive(Clone, Copy)]
struct DrawnHandle {
	point: ManipulatorPointId,
	position: DVec2,
	/// The point its index label sits away from: its anchor, or the midpoint of both anchors for a quadratic's lone handle.
	away_from: DVec2,
}

fn overlay_bezier_handles(
	segment: PathSeg,
	segment_id: SegmentId,
	transform: DAffine2,
	is_selected: impl Fn(ManipulatorPointId) -> bool,
	overlay_context: &mut OverlayContext,
) -> [Option<DrawnHandle>; 2] {
	let segment = Affine::new(transform.to_cols_array()) * segment;
	let segment_start = point_to_dvec2(segment.start());
	let segment_end = point_to_dvec2(segment.end());
	let not_under_anchor = |position: DVec2, anchor: DVec2| position.distance_squared(anchor) >= HIDE_HANDLE_DISTANCE * HIDE_HANDLE_DISTANCE;

	match segment_to_handles(&segment) {
		BezierHandles::Quadratic { handle } if not_under_anchor(handle, segment_start) && not_under_anchor(handle, segment_end) => {
			let point = ManipulatorPointId::PrimaryHandle(segment_id);
			overlay_context.line(handle, segment_start, None, None);
			overlay_context.line(handle, segment_end, None, None);
			overlay_context.manipulator_handle(handle, is_selected(point), None);

			let away_from = segment_start.midpoint(segment_end);
			[Some(DrawnHandle { point, position: handle, away_from }), None]
		}
		BezierHandles::Cubic { handle_start, handle_end } => {
			let mut drawn = [None, None];
			if not_under_anchor(handle_start, segment_start) {
				let point = ManipulatorPointId::PrimaryHandle(segment_id);
				overlay_context.line(handle_start, segment_start, None, None);
				overlay_context.manipulator_handle(handle_start, is_selected(point), None);
				drawn[0] = Some(DrawnHandle {
					point,
					position: handle_start,
					away_from: segment_start,
				});
			}
			if not_under_anchor(handle_end, segment_end) {
				let point = ManipulatorPointId::EndHandle(segment_id);
				overlay_context.line(handle_end, segment_end, None, None);
				overlay_context.manipulator_handle(handle_end, is_selected(point), None);
				drawn[1] = Some(DrawnHandle {
					point,
					position: handle_end,
					away_from: segment_end,
				});
			}
			drawn
		}
		_ => [None, None],
	}
}

fn overlay_bezier_handle_specific_point(
	segment: PathSeg,
	segment_id: SegmentId,
	(start, end): (PointId, PointId),
	point_to_render: PointId,
	transform: DAffine2,
	is_selected: impl Fn(ManipulatorPointId) -> bool,
	overlay_context: &mut OverlayContext,
) -> [Option<DrawnHandle>; 2] {
	let segment = Affine::new(transform.to_cols_array()) * segment;
	let segment_start = point_to_dvec2(segment.start());
	let segment_end = point_to_dvec2(segment.end());
	let not_under_anchor = |position: DVec2, anchor: DVec2| position.distance_squared(anchor) >= HIDE_HANDLE_DISTANCE * HIDE_HANDLE_DISTANCE;

	match segment_to_handles(&segment) {
		BezierHandles::Quadratic { handle } if not_under_anchor(handle, segment_start) && not_under_anchor(handle, segment_end) => {
			let point = ManipulatorPointId::PrimaryHandle(segment_id);
			let anchor = if start == point_to_render { segment_start } else { segment_end };
			overlay_context.line(handle, anchor, None, None);
			overlay_context.manipulator_handle(handle, is_selected(point), None);

			[
				Some(DrawnHandle {
					point,
					position: handle,
					away_from: anchor,
				}),
				None,
			]
		}
		BezierHandles::Cubic { handle_start, handle_end } => {
			let mut drawn = [None, None];
			if not_under_anchor(handle_start, segment_start) && (point_to_render == start) {
				let point = ManipulatorPointId::PrimaryHandle(segment_id);
				overlay_context.line(handle_start, segment_start, None, None);
				overlay_context.manipulator_handle(handle_start, is_selected(point), None);
				drawn[0] = Some(DrawnHandle {
					point,
					position: handle_start,
					away_from: segment_start,
				});
			}
			if not_under_anchor(handle_end, segment_end) && (point_to_render == end) {
				let point = ManipulatorPointId::EndHandle(segment_id);
				overlay_context.line(handle_end, segment_end, None, None);
				overlay_context.manipulator_handle(handle_end, is_selected(point), None);
				drawn[1] = Some(DrawnHandle {
					point,
					position: handle_end,
					away_from: segment_end,
				});
			}
			drawn
		}
		_ => [None, None],
	}
}

pub fn path_overlays(document: &DocumentMessageHandler, draw_handles: DrawHandles, shape_editor: &mut ShapeState, overlay_context: &mut OverlayContext) {
	let display_path = overlay_context.visibility_settings.path();
	let display_handles = overlay_context.visibility_settings.handles();
	let display_anchors = overlay_context.visibility_settings.anchors();
	let document_zoom = document_zoom(document);

	for layer in document.network_interface.selected_nodes().selected_visible_layers(&document.network_interface) {
		let Some(vector) = document.network_interface.compute_modified_vector(layer) else { continue };
		let transform = document.metadata().transform_to_viewport_if_feeds(layer, &document.network_interface);
		if display_path {
			overlay_context.outline_vector(&vector, transform);
		}

		let selected_shape_state = shape_editor.selected_shape_state.entry(layer).or_default();
		// Get the selected segments and then add a bold line overlay on them
		for (segment_id, bezier, _, _) in vector.segment_iter() {
			if selected_shape_state.is_segment_selected(segment_id) {
				overlay_context.outline_select_bezier(bezier, transform);
			}
		}

		let path_index_labels = path_marker_overlays(&vector, transform, document_zoom, false, overlay_context);

		let is_selected = |point: ManipulatorPointId| selected_shape_state.is_point_selected(point);

		let mut drawn_handles = Vec::new();
		if display_handles {
			let opposite_handles_data = selected_shape_state.selected_points().filter_map(|point_id| vector.adjacent_segment(&point_id)).collect::<Vec<_>>();

			match draw_handles {
				DrawHandles::All => {
					vector.segment_iter().for_each(|(segment_id, segment, _start, _end)| {
						drawn_handles.extend(overlay_bezier_handles(segment, segment_id, transform, is_selected, overlay_context).into_iter().flatten());
					});
				}
				DrawHandles::SelectedAnchors(ref selected_segments) => {
					let focused_segments = selected_segments.get(&layer).map(Vec::as_slice).unwrap_or_default();

					vector
						.segment_iter()
						.filter(|(segment_id, ..)| focused_segments.contains(segment_id))
						.for_each(|(segment_id, segment, _start, _end)| {
							drawn_handles.extend(overlay_bezier_handles(segment, segment_id, transform, is_selected, overlay_context).into_iter().flatten());
						});

					for (segment_id, segment, start, end) in vector.segment_iter() {
						if let Some((corresponding_anchor, _)) = opposite_handles_data.iter().find(|(_, adj_segment_id)| adj_segment_id == &segment_id) {
							let drawn = overlay_bezier_handle_specific_point(segment, segment_id, (start, end), *corresponding_anchor, transform, is_selected, overlay_context);
							drawn_handles.extend(drawn.into_iter().flatten());
						}
					}
				}
				DrawHandles::FrontierHandles(ref segment_endpoints_by_layer) => {
					if let Some(segment_endpoints) = segment_endpoints_by_layer.get(&layer) {
						vector
							.segment_iter()
							.filter(|(segment_id, ..)| segment_endpoints.contains_key(segment_id))
							.for_each(|(segment_id, segment, start, end)| {
								let drawn = if segment_endpoints.get(&segment_id).unwrap().len() == 1 {
									let point_to_render = segment_endpoints.get(&segment_id).unwrap()[0];
									overlay_bezier_handle_specific_point(segment, segment_id, (start, end), point_to_render, transform, is_selected, overlay_context)
								} else {
									overlay_bezier_handles(segment, segment_id, transform, is_selected, overlay_context)
								};
								drawn_handles.extend(drawn.into_iter().flatten());
							});
					}
				}
				DrawHandles::None => {}
			}
		}

		if display_anchors {
			for (id, position) in vector.anchors() {
				overlay_context.manipulator_anchor(transform.transform_point2(position), is_selected(ManipulatorPointId::Anchor(id)), None);
			}
		}

		index_label_overlays(&vector, transform, false, &drawn_handles, path_index_labels, overlay_context);
	}
}

/// Draws an anchor overlay at each endpoint of every open path on the selected visible layers, in the selected style for endpoints that are part of the path editing selection.
/// Given a pointer position, the endpoint a press there would continue from is drawn in the hover style instead.
pub fn open_path_endpoint_overlays(document: &DocumentMessageHandler, shape_editor: &ShapeState, pointer: Option<DVec2>, overlay_context: &mut OverlayContext) {
	if !overlay_context.visibility_settings.anchors() {
		return;
	}

	let selected_nodes = document.network_interface.selected_nodes();
	let is_selected = |layer: LayerNodeIdentifier, id: PointId| {
		shape_editor
			.selected_shape_state
			.get(&layer)
			.is_some_and(|state| state.is_point_selected(ManipulatorPointId::Anchor(id)))
	};
	let hovered = pointer.and_then(|pointer| closest_open_path_endpoint(document, pointer, SNAP_POINT_TOLERANCE, selected_nodes.selected_visible_layers(&document.network_interface)));
	let document_zoom = document_zoom(document);

	for layer in selected_nodes.selected_visible_layers(&document.network_interface) {
		let Some(vector) = document.network_interface.compute_modified_vector(layer) else { continue };
		let transform = document.metadata().transform_to_viewport_if_feeds(layer, &document.network_interface);

		let path_index_labels = path_marker_overlays(&vector, transform, document_zoom, true, overlay_context);

		for id in vector.anchor_endpoints() {
			if hovered.is_some_and(|(hovered_layer, hovered_id, _)| hovered_layer == layer && hovered_id == id) {
				continue;
			}
			let Some(position) = vector.position_from_id(id) else { continue };

			overlay_context.manipulator_anchor(transform.transform_point2(position), is_selected(layer, id), None);
		}

		index_label_overlays(&vector, transform, true, &[], path_index_labels, overlay_context);
	}

	// Drawn last so its halo sits above any other endpoint at the same spot
	if let Some((layer, id, position)) = hovered {
		let transform = document.metadata().transform_to_viewport_if_feeds(layer, &document.network_interface);
		overlay_context.hover_manipulator_anchor(transform.transform_point2(position), is_selected(layer, id));
	}
}

/// How many viewport pixels span one document pixel at the current zoom.
fn document_zoom(document: &DocumentMessageHandler) -> f64 {
	document.metadata().document_to_viewport.matrix2.determinant().abs().sqrt()
}

/// A path's index label, placed off its start anchor where it covers the least of the path.
struct PathIndexLabel {
	index: usize,
	/// The pool index of the anchor the path starts at, where a point index label may share its spot.
	anchor: u32,
	position: DVec2,
}

/// Marks how each path of a layer runs and bends, as the overlay settings choose: its ends and flow joins, direction arrows, and curvature comb.
/// Returns each path's index label for [`index_label_overlays`] to draw. With `open_paths_only`, closed paths go unmarked, for tools that only show the anchors at open ends.
fn path_marker_overlays(vector: &VectorGeometry, transform: DAffine2, document_zoom: f64, open_paths_only: bool, overlay_context: &mut OverlayContext) -> Vec<PathIndexLabel> {
	let mut index_labels = Vec::new();

	let settings = overlay_context.visibility_settings;
	let (show_ends, show_index, show_direction, show_curvature_comb) = (settings.path_ends(), settings.path_index(), settings.path_direction(), settings.curvature_comb());
	if !(show_ends || show_index || show_direction || show_curvature_comb) {
		return index_labels;
	}

	let to_viewport = Affine::new(transform.to_cols_array());
	let viewport_size: DVec2 = overlay_context.viewport.size().into();
	let arrow_view = Rect::new(0., 0., viewport_size.x, viewport_size.y).inflate(PATH_DIRECTION_CHEVRON_SIZE, PATH_DIRECTION_CHEVRON_SIZE);

	// Curvature measured in the viewport shrinks as the zoom grows, so squaring the zoom keeps the teeth in proportion to the artwork
	let comb_scale = settings.curvature_comb_scale * document_zoom.powi(2);

	// Enumerated before filtering so each index is the one Separate Paths gives that path
	for (index, contour) in vector.oriented_contours().into_iter().enumerate() {
		if open_paths_only && contour.closed {
			continue;
		}

		let segments: Vec<PathSeg> = contour.segments.iter().map(|&segment| to_viewport * segment).collect();
		if segments.iter().map(|&segment| control_polygon_length(segment)).sum::<f64>() < PATH_MARKERS_MIN_LENGTH {
			continue;
		}
		// Zero-length segments point nowhere, so the markers at each end go on the outermost segments that do
		let has_direction = |segment: &&PathSeg| end_directions(**segment).is_some();
		let Some(&first) = segments.iter().find(has_direction) else { continue };
		let Some(&last) = segments.iter().rfind(has_direction) else { continue };
		let Some((departure, _)) = end_directions(first) else { continue };
		let start = point_to_dvec2(first.start());

		// Teeth sticking out from each curve as far as it bends there, with their tips joined, drawn beneath the other markers
		if show_curvature_comb {
			let mut teeth = BezPath::new();
			let mut tips = BezPath::new();

			// O(teeth) curvature evaluations per segment, capped for segments that run far past the viewport
			for &segment in segments.iter().filter(|segment| !matches!(segment, PathSeg::Line(_))) {
				let tooth_count = (control_polygon_length(segment) / CURVATURE_COMB_TOOTH_SPACING).ceil().clamp(1., CURVATURE_COMB_MAX_TEETH_PER_SEGMENT);

				// Each segment's tips join on their own, so an anchor where the curvature jumps shows a break in the comb
				let step_size = 1. / tooth_count;
				let mut previous_tooth: Option<CurvatureCombTooth> = None;
				for step in 0..=tooth_count as usize {
					let Some(tooth) = curvature_comb_tooth(segment, step as f64 * step_size, comb_scale) else {
						previous_tooth = None;
						continue;
					};

					teeth.move_to(dvec2_to_point(tooth.root));
					teeth.line_to(dvec2_to_point(tooth.tip));

					// Hermite curves between tips, leaving each one in the exact direction the tips travel there
					match previous_tooth {
						Some(previous) => tips.curve_to(
							dvec2_to_point(previous.tip + previous.tip_velocity * step_size / 3.),
							dvec2_to_point(tooth.tip - tooth.tip_velocity * step_size / 3.),
							dvec2_to_point(tooth.tip),
						),
						None => tips.move_to(dvec2_to_point(tooth.tip)),
					}
					previous_tooth = Some(tooth);
				}
			}

			overlay_context.stroke_path(&teeth, DAffine2::IDENTITY, COLOR_OVERLAY_BLUE_50);
			overlay_context.stroke_path(&tips, DAffine2::IDENTITY, COLOR_OVERLAY_BLUE);
		}

		// Arrows spread evenly over the stretch clear of the end markers, as harpoons barbed on the right of travel so their side shows the winding direction
		if show_direction {
			// O(segments + arrows in view) arc lengths per frame
			let lengths: Vec<f64> = segments.iter().map(|segment| segment.arclen(PATH_DIRECTION_ARC_LENGTH_ACCURACY)).collect();
			let total_length: f64 = lengths.iter().sum();

			let end_clearance = if show_ends { PATH_END_MARKER_DISTANCE + PATH_END_MARKER_SIZE } else { 0. };
			let (arrows_start, arrows_end) = (end_clearance, total_length - end_clearance);
			let arrows_length = arrows_end - arrows_start;

			if arrows_length.is_finite() && arrows_length >= PATH_DIRECTION_ARROW_SPACING / 2. {
				let arrow_count = (arrows_length / PATH_DIRECTION_ARROW_SPACING).round().max(1.);
				let spacing = arrows_length / arrow_count;

				let mut pieces_in_view = Vec::new();
				let mut walked = 0.;
				for (&segment, &length) in segments.iter().zip(&lengths) {
					collect_pieces_in_view(segment, walked, length, arrow_view, spacing, &mut pieces_in_view);
					walked += length;
				}

				for (piece, piece_start, piece_length) in pieces_in_view {
					// The first of the arrows, centered in each interval of `spacing` along their stretch, that falls on this piece
					let mut next_arrow = arrows_start + (((piece_start - arrows_start) / spacing - 0.5).ceil().max(0.) + 0.5) * spacing;

					while next_arrow < (piece_start + piece_length).min(arrows_end) {
						// With the viewport's Y axis pointing down, the perpendicular is on the right of travel
						if let Some((point, direction)) = point_along(piece, next_arrow - piece_start) {
							draw_chevron(overlay_context, point, direction, PATH_DIRECTION_CHEVRON_SIZE, Some(direction.perp()));
						}
						next_arrow += spacing;
					}
				}
			}
		}

		// A chevron just after the start anchor and a bar just before the end, which on a closed path is where it arrives back at its start
		if show_ends {
			if let Some((point, direction)) = point_along(first, PATH_END_MARKER_DISTANCE) {
				draw_chevron(overlay_context, point, direction, PATH_END_MARKER_SIZE, None);
			}

			if let Some((point, direction)) = point_along(last.reverse(), PATH_END_MARKER_DISTANCE) {
				let across = direction.perp() * PATH_END_MARKER_SIZE / 2.;
				overlay_context.line(point - across, point + across, None, None);
			}

			// A tick across the path at each anchor where it crosses from one path flow into another
			for &join in &contour.flow_joins {
				let arriving = join.checked_sub(1).map_or(segments.last(), |previous| segments.get(previous));
				let (Some(&arriving), Some(&departing)) = (arriving, segments.get(join)) else { continue };
				let (Some((_, into_join)), Some((out_of_join, _))) = (end_directions(arriving), end_directions(departing)) else {
					continue;
				};

				let along = direction_or(into_join + out_of_join, out_of_join);
				let across = along.perp() * PATH_FLOW_JOIN_TICK_LENGTH / 2.;
				let point = point_to_dvec2(departing.start());
				overlay_context.line(point - across, point + across, None, Some(PATH_FLOW_JOIN_TICK_THICKNESS));
			}
		}

		// The index sits off the start, opposite the segments leaving it, where it covers the least of the path
		if show_index {
			let arrival = end_directions(last).filter(|_| contour.closed).map(|(_, arrival)| arrival);
			let spokes = arrival.map_or(departure, |arrival| departure - arrival);

			// Where the path runs straight through its start, the label goes outside the loop
			let clockwise = segments.iter().map(|segment| segment.signed_area()).sum::<f64>() > 0.;
			let outside = if clockwise { -departure.perp() } else { departure.perp() };
			let away = direction_or(-spokes, outside);

			index_labels.push(PathIndexLabel {
				index,
				anchor: contour.entry,
				position: start + away * PATH_INDEX_LABEL_OFFSET,
			});
		}
	}

	index_labels
}

/// Draws the path index labels, and when the overlay settings show point indices, labels the anchors (or just the endpoints with `endpoints_only`) and drawn handles with their indices in the layer's list of points.
/// A point index label sits off its point away from the segments meeting there, and a path's start anchor shares that spot with the path's index, the two side by side.
fn index_label_overlays(vector: &VectorGeometry, transform: DAffine2, endpoints_only: bool, drawn_handles: &[DrawnHandle], path_index_labels: Vec<PathIndexLabel>, overlay_context: &OverlayContext) {
	// The size `OverlayContext::text` draws at
	const FONT_SIZE: f64 = 12.;

	let draw_label = |text: &str, color: &str, position: DVec2, horizontal_pivot: Pivot| {
		overlay_context.text(text, color, None, DAffine2::from_translation(position), 0., [horizontal_pivot, Pivot::Middle]);
	};

	if !overlay_context.visibility_settings.point_index() {
		for label in &path_index_labels {
			draw_label(&label.index.to_string(), COLOR_OVERLAY_MAGENTA, label.position, Pivot::Middle);
		}
		return;
	}

	let mut path_index_labels_at: HashMap<u32, Vec<&PathIndexLabel>> = HashMap::new();
	for label in &path_index_labels {
		path_index_labels_at.entry(label.anchor).or_default().push(label);
	}

	let to_viewport = Affine::new(transform.to_cols_array());

	// In one O(segments) pass: the directions segments leave each anchor in (their sum and the first), and each handle's pool index
	let mut spokes: HashMap<u32, (DVec2, DVec2)> = HashMap::new();
	let mut handle_indices: HashMap<ManipulatorPointId, u32> = HashMap::new();
	for span in spans(&vector.paths) {
		let Some((start, end)) = span_endpoints(&vector.paths, span) else { continue };
		let curve_type = curve_type_at(&vector.paths, span.path as usize);
		let Some(segment) = vector
			.paths
			.element(span.path as usize)
			.and_then(|path| path.segment(&vector.vector.0, curve_type, span.index as usize))
		else {
			continue;
		};

		if let Some((departure, arrival)) = end_directions(to_viewport * segment) {
			for (anchor, spoke) in [(start, departure), (end, -arrival)] {
				let (sum, _) = spokes.entry(anchor).or_insert((DVec2::ZERO, spoke));
				*sum += spoke;
			}
		}

		let segment_id = segment_id_of(&vector.paths, span).unwrap_or_default();
		for (point, primary) in [(ManipulatorPointId::PrimaryHandle(segment_id), true), (ManipulatorPointId::EndHandle(segment_id), false)] {
			if let Some(index) = handle_point(&vector.paths, span, primary) {
				handle_indices.insert(point, index);
			}
		}
	}

	let endpoints: HashSet<PointId> = if endpoints_only { vector.anchor_endpoints().collect() } else { HashSet::new() };

	// Opposite the segments meeting at an anchor, or beside them on the upper side where they run straight through
	for index in vector.vector.anchor_indices(&vector.paths) {
		let index = index as u32;
		let (Some(id), Some(position)) = (vector.point_id_at(index), vector.position(index as usize)) else {
			continue;
		};
		if endpoints_only && !endpoints.contains(&id) {
			continue;
		}

		let away = match spokes.get(&index) {
			Some(&(sum, first)) => direction_or(-sum, if first.perp().y > 0. { -first.perp() } else { first.perp() }),
			None => DVec2::NEG_Y,
		};
		let spot = transform.transform_point2(position) + away * POINT_INDEX_LABEL_OFFSET;
		let point_index = index.to_string();

		let Some(path_labels) = path_index_labels_at.remove(&index) else {
			draw_label(&point_index, COLOR_OVERLAY_BLUE, spot, Pivot::Middle);
			continue;
		};

		// The point index and path index sit either side of the spot, slid outward so the label nearer the anchor clears it as a lone label would
		let path_index = path_labels.iter().map(|label| label.index.to_string()).collect::<Vec<_>>().join(", ");
		let nearer_width = text_width(if away.x < 0. { &path_index } else { &point_index }, FONT_SIZE);
		let center = spot + DVec2::X * away.x * (INDEX_LABEL_GAP + nearer_width) / 2.;
		draw_label(&point_index, COLOR_OVERLAY_BLUE, center - DVec2::X * INDEX_LABEL_GAP / 2., Pivot::End);
		draw_label(&path_index, COLOR_OVERLAY_MAGENTA, center + DVec2::X * INDEX_LABEL_GAP / 2., Pivot::Start);
	}

	// Past each handle's tip, away from its anchor
	for handle in drawn_handles {
		let Some(&index) = handle_indices.get(&handle.point) else { continue };
		let away = direction_or(handle.position - handle.away_from, DVec2::NEG_Y);
		draw_label(&index.to_string(), COLOR_OVERLAY_BLUE, handle.position + away * POINT_INDEX_LABEL_OFFSET, Pivot::Middle);
	}

	// Any path index whose start anchor has no point index label of its own
	for label in path_index_labels_at.into_values().flatten() {
		draw_label(&label.index.to_string(), COLOR_OVERLAY_MAGENTA, label.position, Pivot::Middle);
	}
}

/// Collects the parts of a segment that `view` overlaps, each with where it starts along the path and its length, halving any part that crosses the edge of `view` until it's shorter than `min_length`.
/// Costs O(log(length / min_length)) arc lengths per crossing of that edge.
fn collect_pieces_in_view(piece: PathSeg, start: f64, length: f64, view: Rect, min_length: f64, pieces: &mut Vec<(PathSeg, f64, f64)>) {
	let bounds = piece.bounding_box();
	if !bounds.overlaps(view) {
		return;
	}

	if length < min_length || view.contains_rect(bounds) {
		pieces.push((piece, start, length));
		return;
	}

	let (first_half, second_half) = piece.subdivide();
	let first_length = first_half.arclen(PATH_DIRECTION_ARC_LENGTH_ACCURACY);
	collect_pieces_in_view(first_half, start, first_length, view, min_length, pieces);
	collect_pieces_in_view(second_half, start + first_length, length - first_length, view, min_length, pieces);
}

/// Draws a right-angled `>` chevron `size` across, centered on `center` and pointing in `direction`.
/// Given a `barb_side`, it instead draws a one-sided `|\` harpoon: a shaft set off from the path toward that side, with a barb reaching further in.
fn draw_chevron(overlay_context: &mut OverlayContext, center: DVec2, direction: DVec2, size: f64, barb_side: Option<DVec2>) {
	let tip = center + direction * size / 4.;
	let back = center - direction * size / 4.;

	match barb_side {
		Some(side) => {
			let (tip, back) = (tip + side * PATH_DIRECTION_HARPOON_OFFSET, back + side * PATH_DIRECTION_HARPOON_OFFSET);
			let barb_end = back + side * size / 2.;

			// The shaft matches the barb's length
			let shaft_end = tip - direction * tip.distance(barb_end);

			overlay_context.line(shaft_end, tip, Some(COLOR_OVERLAY_MAGENTA), None);
			overlay_context.line(barb_end, tip, Some(COLOR_OVERLAY_MAGENTA), None);
		}
		None => {
			let spread = direction.perp() * size / 2.;
			overlay_context.line(back + spread, tip, None, None);
			overlay_context.line(back - spread, tip, None, None);
		}
	}
}

/// The point a distance along a segment from its start, or its end if the segment is shorter, with the unit direction the segment runs there.
fn point_along(segment: PathSeg, distance: f64) -> Option<(DVec2, DVec2)> {
	let t = segment.inv_arclen(distance, PATH_DIRECTION_ARC_LENGTH_ACCURACY);
	let direction = pathseg_tangent(segment, t).try_normalize().or_else(|| end_directions(segment).map(|(departure, _)| departure))?;

	Some((point_to_dvec2(segment.eval(t)), direction))
}

/// The unit directions a segment sets out in and arrives from, read from its control points so a handle resting on its anchor still gives one.
fn end_directions(segment: PathSeg) -> Option<(DVec2, DVec2)> {
	let PathSegPoints { p0, p1, p2, p3 } = pathseg_points(segment);
	let distinct = |point: &DVec2, end: DVec2| point.distance_squared(end) > f64::EPSILON;

	let departure = [p1, p2, Some(p3)].into_iter().flatten().find(|point| distinct(point, p0))? - p0;
	let arrival = p3 - [p2, p1, Some(p0)].into_iter().flatten().find(|point| distinct(point, p3))?;

	Some((departure.try_normalize()?, arrival.try_normalize()?))
}

/// One tooth of a curvature comb, in viewport space.
#[derive(Clone, Copy)]
struct CurvatureCombTooth {
	/// Where the tooth sticks out from the segment.
	root: DVec2,
	tip: DVec2,
	/// How fast the tip moves as the segment's `t` advances, for joining the tips with smooth curves.
	tip_velocity: DVec2,
}

/// The curvature comb tooth on a segment at `t`, reaching away from the center of the bend by the curvature times `scale`.
/// `None` where the segment momentarily stops (as at a cusp) and so bends in no particular direction.
fn curvature_comb_tooth(segment: PathSeg, t: f64, scale: f64) -> Option<CurvatureCombTooth> {
	let velocity_curve = segment.to_cubic().deriv();
	let acceleration_curve = velocity_curve.deriv();
	let [velocity, acceleration, jerk] = [velocity_curve.eval(t), acceleration_curve.eval(t), acceleration_curve.deriv().eval(t)].map(point_to_dvec2);

	let speed_squared = velocity.length_squared();
	if speed_squared < 1e-18 {
		return None;
	}
	let speed = speed_squared.sqrt();

	// Signed curvature, positive where the segment bends toward its left normal, and how fast it changes along `t`
	let curvature = velocity.perp_dot(acceleration) / (speed_squared * speed);
	let curvature_velocity = velocity.perp_dot(jerk) / (speed_squared * speed) - 3. * curvature * velocity.dot(acceleration) / speed_squared;
	let normal = velocity.perp() / speed;

	// A tooth held at the maximum length stops growing, so only the turning normal moves its tip
	let unclamped_length = curvature * scale;
	let (length, length_velocity) = if unclamped_length.abs() <= CURVATURE_COMB_MAX_TOOTH_LENGTH {
		(unclamped_length, curvature_velocity * scale)
	} else {
		(CURVATURE_COMB_MAX_TOOTH_LENGTH.copysign(unclamped_length), 0.)
	};

	// The normal turns at minus the curvature times the velocity, carrying the tip with it
	let root = point_to_dvec2(segment.eval(t));
	Some(CurvatureCombTooth {
		root,
		tip: root - normal * length,
		tip_velocity: velocity * (1. + length * curvature) - normal * length_velocity,
	})
}

/// The unit direction of a vector, or the fallback when the vector is too short to point anywhere reliably.
fn direction_or(vector: DVec2, fallback: DVec2) -> DVec2 {
	if vector.length_squared() > 1e-6 { vector.normalize() } else { fallback }
}

/// The length of a segment's control polygon, which is never shorter than the segment itself.
fn control_polygon_length(segment: PathSeg) -> f64 {
	let PathSegPoints { p0, p1, p2, p3 } = pathseg_points(segment);

	let mut previous = p0;
	let mut length = 0.;
	for point in [p1, p2, Some(p3)].into_iter().flatten() {
		length += previous.distance(point);
		previous = point;
	}

	length
}

pub fn hex_to_rgba_u8(hex: &str) -> [u8; 4] {
	let hex = hex.trim().trim_start_matches('#');
	if hex.len() != 6 && hex.len() != 8 {
		return [0, 0, 0, 255];
	}
	let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
	let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
	let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
	let a = if hex.len() >= 8 { u8::from_str_radix(&hex[6..8], 16).unwrap_or(255) } else { 255 };
	[r, g, b, a]
}

#[cfg(test)]
mod tests {
	use super::*;
	use kurbo::CubicBez;

	#[test]
	fn curvature_comb_teeth_point_away_from_the_center_of_the_bend() {
		// A quarter circle of radius 100 around the origin
		let handle = 100. * 0.552_284_75;
		let arc = PathSeg::Cubic(CubicBez::new((100., 0.), (100., handle), (handle, 100.), (0., 100.)));

		for segment in [arc, arc.reverse()] {
			let Some(tooth) = curvature_comb_tooth(segment, 0.5, 1000.) else { panic!("The arc should bend") };
			let reach = tooth.tip.length() - tooth.root.length();

			assert!((reach - 10.).abs() < 0.2, "The tooth should reach about 10 units outward from the arc, not {reach}");
		}
	}

	#[test]
	fn curvature_comb_tip_velocity_matches_how_the_tips_move() {
		let segment = PathSeg::Cubic(CubicBez::new((0., 0.), (30., 80.), (90., -40.), (120., 30.)));
		let tip_at = |t: f64, scale: f64| curvature_comb_tooth(segment, t, scale).unwrap().tip;

		// Teeth growing with the curvature, then held at the maximum length
		for scale in [500., 1e9] {
			for t in [0.15, 0.3, 0.85] {
				let step = 1e-6;
				let measured = (tip_at(t + step, scale) - tip_at(t - step, scale)) / (2. * step);
				let derived = curvature_comb_tooth(segment, t, scale).unwrap().tip_velocity;

				assert!((derived - measured).length() < 1e-4 * measured.length().max(1.), "At t = {t}, {derived} should match {measured}");
			}
		}
	}
}

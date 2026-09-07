use super::utility_types::{DrawHandles, OverlayContext};
use crate::consts::{HIDE_HANDLE_DISTANCE, SNAP_POINT_TOLERANCE};
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::NodeNetworkInterface;
pub use crate::messages::portfolio::document::utility_types::text_metrics::text_width;
use crate::messages::tool::common_functionality::shape_editor::{SelectedLayerState, ShapeState};
use crate::messages::tool::common_functionality::utility_functions::closest_open_path_endpoint;
use crate::messages::tool::tool_messages::tool_prelude::DocumentMessageHandler;
use glam::{DAffine2, DVec2};
use graphene_std::vector::misc::{BezierHandles, ManipulatorPointId, point_to_dvec2, segment_to_handles};
use graphene_std::vector::{PointId, SegmentId, Vector};
use kurbo::{Affine, ParamCurve, PathSeg};
use std::collections::HashMap;
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

pub fn selected_segments_for_layer(vector: &Vector, state: &SelectedLayerState) -> Vec<SegmentId> {
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

fn overlay_bezier_handles(segment: PathSeg, segment_id: SegmentId, transform: DAffine2, is_selected: impl Fn(ManipulatorPointId) -> bool, overlay_context: &mut OverlayContext) {
	let segment = Affine::new(transform.to_cols_array()) * segment;
	let segment_start = point_to_dvec2(segment.start());
	let segment_end = point_to_dvec2(segment.end());
	let not_under_anchor = |position: DVec2, anchor: DVec2| position.distance_squared(anchor) >= HIDE_HANDLE_DISTANCE * HIDE_HANDLE_DISTANCE;

	match segment_to_handles(&segment) {
		BezierHandles::Quadratic { handle } if not_under_anchor(handle, segment_start) && not_under_anchor(handle, segment_end) => {
			overlay_context.line(handle, segment_start, None, None);
			overlay_context.line(handle, segment_end, None, None);
			overlay_context.manipulator_handle(handle, is_selected(ManipulatorPointId::PrimaryHandle(segment_id)), None);
		}
		BezierHandles::Cubic { handle_start, handle_end } => {
			if not_under_anchor(handle_start, segment_start) {
				overlay_context.line(handle_start, segment_start, None, None);
				overlay_context.manipulator_handle(handle_start, is_selected(ManipulatorPointId::PrimaryHandle(segment_id)), None);
			}
			if not_under_anchor(handle_end, segment_end) {
				overlay_context.line(handle_end, segment_end, None, None);
				overlay_context.manipulator_handle(handle_end, is_selected(ManipulatorPointId::EndHandle(segment_id)), None);
			}
		}
		_ => {}
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
) {
	let segment = Affine::new(transform.to_cols_array()) * segment;
	let segment_start = point_to_dvec2(segment.start());
	let segment_end = point_to_dvec2(segment.end());
	let not_under_anchor = |position: DVec2, anchor: DVec2| position.distance_squared(anchor) >= HIDE_HANDLE_DISTANCE * HIDE_HANDLE_DISTANCE;

	match segment_to_handles(&segment) {
		BezierHandles::Quadratic { handle } if not_under_anchor(handle, segment_start) && not_under_anchor(handle, segment_end) => {
			let anchor = if start == point_to_render { segment_start } else { segment_end };
			overlay_context.line(handle, anchor, None, None);
			overlay_context.manipulator_handle(handle, is_selected(ManipulatorPointId::PrimaryHandle(segment_id)), None);
		}
		BezierHandles::Cubic { handle_start, handle_end } => {
			if not_under_anchor(handle_start, segment_start) && (point_to_render == start) {
				overlay_context.line(handle_start, segment_start, None, None);
				overlay_context.manipulator_handle(handle_start, is_selected(ManipulatorPointId::PrimaryHandle(segment_id)), None);
			}
			if not_under_anchor(handle_end, segment_end) && (point_to_render == end) {
				overlay_context.line(handle_end, segment_end, None, None);
				overlay_context.manipulator_handle(handle_end, is_selected(ManipulatorPointId::EndHandle(segment_id)), None);
			}
		}
		_ => {}
	}
}

pub fn path_overlays(document: &DocumentMessageHandler, draw_handles: DrawHandles, shape_editor: &mut ShapeState, overlay_context: &mut OverlayContext) {
	let display_path = overlay_context.visibility_settings.path();
	let display_handles = overlay_context.visibility_settings.handles();
	let display_anchors = overlay_context.visibility_settings.anchors();

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

		let is_selected = |point: ManipulatorPointId| selected_shape_state.is_point_selected(point);

		if display_handles {
			let opposite_handles_data = selected_shape_state.selected_points().filter_map(|point_id| vector.adjacent_segment(&point_id)).collect::<Vec<_>>();

			match draw_handles {
				DrawHandles::All => {
					vector.segment_iter().for_each(|(segment_id, segment, _start, _end)| {
						overlay_bezier_handles(segment, segment_id, transform, is_selected, overlay_context);
					});
				}
				DrawHandles::SelectedAnchors(ref selected_segments) => {
					let Some(focused_segments) = selected_segments.get(&layer) else { continue };

					vector
						.segment_iter()
						.filter(|(segment_id, ..)| focused_segments.contains(segment_id))
						.for_each(|(segment_id, segment, _start, _end)| {
							overlay_bezier_handles(segment, segment_id, transform, is_selected, overlay_context);
						});

					for (segment_id, segment, start, end) in vector.segment_iter() {
						if let Some((corresponding_anchor, _)) = opposite_handles_data.iter().find(|(_, adj_segment_id)| adj_segment_id == &segment_id) {
							overlay_bezier_handle_specific_point(segment, segment_id, (start, end), *corresponding_anchor, transform, is_selected, overlay_context);
						}
					}
				}
				DrawHandles::FrontierHandles(ref segment_endpoints_by_layer) => {
					let Some(segment_endpoints) = segment_endpoints_by_layer.get(&layer) else { continue };

					vector
						.segment_iter()
						.filter(|(segment_id, ..)| segment_endpoints.contains_key(segment_id))
						.for_each(|(segment_id, segment, start, end)| {
							if segment_endpoints.get(&segment_id).unwrap().len() == 1 {
								let point_to_render = segment_endpoints.get(&segment_id).unwrap()[0];
								overlay_bezier_handle_specific_point(segment, segment_id, (start, end), point_to_render, transform, is_selected, overlay_context);
							} else {
								overlay_bezier_handles(segment, segment_id, transform, is_selected, overlay_context);
							}
						});
				}
				DrawHandles::None => {}
			}
		}

		if display_anchors {
			for (&id, &position) in vector.point_domain.ids().iter().zip(vector.point_domain.positions()) {
				overlay_context.manipulator_anchor(transform.transform_point2(position), is_selected(ManipulatorPointId::Anchor(id)), None);
			}
		}
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

	for layer in selected_nodes.selected_visible_layers(&document.network_interface) {
		let Some(vector) = document.network_interface.compute_modified_vector(layer) else { continue };
		let transform = document.metadata().transform_to_viewport_if_feeds(layer, &document.network_interface);

		for id in vector.anchor_endpoints() {
			if hovered.is_some_and(|(hovered_layer, hovered_id, _)| hovered_layer == layer && hovered_id == id) {
				continue;
			}
			let Some(position) = vector.point_domain.position_from_id(id) else { continue };

			overlay_context.manipulator_anchor(transform.transform_point2(position), is_selected(layer, id), None);
		}
	}

	// Drawn last so its halo sits above any other endpoint at the same spot
	if let Some((layer, id, position)) = hovered {
		let transform = document.metadata().transform_to_viewport_if_feeds(layer, &document.network_interface);
		overlay_context.hover_manipulator_anchor(transform.transform_point2(position), is_selected(layer, id));
	}
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

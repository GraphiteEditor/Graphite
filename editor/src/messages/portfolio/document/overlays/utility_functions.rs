use super::utility_types::{DrawHandles, OverlayContext};
use crate::consts::HIDE_HANDLE_DISTANCE;
use crate::messages::portfolio::document::utility_types::network_interface::NodeNetworkInterface;
use crate::messages::tool::common_functionality::shape_editor::{SelectedLayerState, ShapeState};
use crate::messages::tool::tool_messages::tool_prelude::{DocumentMessageHandler, PreferencesMessageHandler};
use bezier_rs::{Bezier, BezierHandles};
use glam::{DAffine2, DVec2};
use graphene_std::vector::misc::ManipulatorPointId;
use graphene_std::vector::{PointId, SegmentId};
use wasm_bindgen::JsCast;

pub fn overlay_canvas_element() -> Option<web_sys::HtmlCanvasElement> {
	let window = web_sys::window()?;
	let document = window.document()?;
	let canvas = document.query_selector("[data-overlays-canvas]").ok().flatten()?;
	canvas.dyn_into::<web_sys::HtmlCanvasElement>().ok()
}

pub fn overlay_canvas_context() -> web_sys::CanvasRenderingContext2d {
	let create_context = || {
		let context = overlay_canvas_element()?.get_context("2d").ok().flatten()?;
		context.dyn_into().ok()
	};
	create_context().expect("Failed to get canvas context")
}

pub fn selected_segments(network_interface: &NodeNetworkInterface, shape_editor: &ShapeState) -> Vec<SegmentId> {
	let selected_points = shape_editor.selected_points();
	let selected_anchors = selected_points
		.filter_map(|point_id| if let ManipulatorPointId::Anchor(p) = point_id { Some(*p) } else { None })
		.collect::<Vec<_>>();

	// Collect the segments whose handles are selected
	let mut selected_segments = shape_editor
		.selected_points()
		.filter_map(|point_id| match point_id {
			ManipulatorPointId::PrimaryHandle(segment_id) | ManipulatorPointId::EndHandle(segment_id) => Some(*segment_id),
			ManipulatorPointId::Anchor(_) => None,
		})
		.collect::<Vec<_>>();

	// TODO: Currently if there are two duplicate layers, both of their segments get overlays
	// Adding segments which are are connected to selected anchors
	for layer in network_interface.selected_nodes().selected_layers(network_interface.document_metadata()) {
		let Some(vector) = network_interface.compute_modified_vector(layer) else { continue };

		for (segment_id, _bezier, start, end) in vector.segment_bezier_iter() {
			if selected_anchors.contains(&start) || selected_anchors.contains(&end) {
				selected_segments.push(segment_id);
			}
		}
	}

	selected_segments
}

fn overlay_bezier_handles(bezier: Bezier, segment_id: SegmentId, transform: DAffine2, is_selected: impl Fn(ManipulatorPointId) -> bool, overlay_context: &mut OverlayContext) {
	let bezier = bezier.apply_transformation(|point| transform.transform_point2(point));
	let not_under_anchor = |position: DVec2, anchor: DVec2| position.distance_squared(anchor) >= HIDE_HANDLE_DISTANCE * HIDE_HANDLE_DISTANCE;

	match bezier.handles {
		BezierHandles::Quadratic { handle } if not_under_anchor(handle, bezier.start) && not_under_anchor(handle, bezier.end) => {
			overlay_context.line(handle, bezier.start, None, None);
			overlay_context.line(handle, bezier.end, None, None);
			overlay_context.manipulator_handle(handle, is_selected(ManipulatorPointId::PrimaryHandle(segment_id)), None);
		}
		BezierHandles::Cubic { handle_start, handle_end } => {
			if not_under_anchor(handle_start, bezier.start) {
				overlay_context.line(handle_start, bezier.start, None, None);
				overlay_context.manipulator_handle(handle_start, is_selected(ManipulatorPointId::PrimaryHandle(segment_id)), None);
			}
			if not_under_anchor(handle_end, bezier.end) {
				overlay_context.line(handle_end, bezier.end, None, None);
				overlay_context.manipulator_handle(handle_end, is_selected(ManipulatorPointId::EndHandle(segment_id)), None);
			}
		}
		_ => {}
	}
}

fn overlay_bezier_handle_specific_point(
	bezier: Bezier,
	segment_id: SegmentId,
	(start, end): (PointId, PointId),
	point_to_render: PointId,
	transform: DAffine2,
	is_selected: impl Fn(ManipulatorPointId) -> bool,
	overlay_context: &mut OverlayContext,
) {
	let bezier = bezier.apply_transformation(|point| transform.transform_point2(point));
	let not_under_anchor = |position: DVec2, anchor: DVec2| position.distance_squared(anchor) >= HIDE_HANDLE_DISTANCE * HIDE_HANDLE_DISTANCE;

	match bezier.handles {
		BezierHandles::Quadratic { handle } => {
			if not_under_anchor(handle, bezier.start) && not_under_anchor(handle, bezier.end) {
				let end = if start == point_to_render { bezier.start } else { bezier.end };
				overlay_context.line(handle, end, None, None);
				overlay_context.manipulator_handle(handle, is_selected(ManipulatorPointId::PrimaryHandle(segment_id)), None);
			}
		}
		BezierHandles::Cubic { handle_start, handle_end } => {
			if not_under_anchor(handle_start, bezier.start) && (point_to_render == start) {
				overlay_context.line(handle_start, bezier.start, None, None);
				overlay_context.manipulator_handle(handle_start, is_selected(ManipulatorPointId::PrimaryHandle(segment_id)), None);
			}
			if not_under_anchor(handle_end, bezier.end) && (point_to_render == end) {
				overlay_context.line(handle_end, bezier.end, None, None);
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

	for layer in document.network_interface.selected_nodes().selected_layers(document.metadata()) {
		let Some(vector) = document.network_interface.compute_modified_vector(layer) else { continue };
		let transform = document.metadata().transform_to_viewport_if_feeds(layer, &document.network_interface);
		if display_path {
			overlay_context.outline_vector(&vector, transform);
		}

		// Get the selected segments and then add a bold line overlay on them
		for (segment_id, bezier, _, _) in vector.segment_bezier_iter() {
			let Some(selected_shape_state) = shape_editor.selected_shape_state.get_mut(&layer) else {
				continue;
			};

			if selected_shape_state.is_segment_selected(segment_id) {
				overlay_context.outline_select_bezier(bezier, transform);
			}
		}

		let selected = shape_editor.selected_shape_state.get(&layer);
		let is_selected = |point: ManipulatorPointId| selected.is_some_and(|selected| selected.is_point_selected(point));

		if display_handles {
			let opposite_handles_data: Vec<(PointId, SegmentId)> = shape_editor.selected_points().filter_map(|point_id| vector.adjacent_segment(point_id)).collect();

			match draw_handles {
				DrawHandles::All => {
					vector.segment_bezier_iter().for_each(|(segment_id, bezier, _start, _end)| {
						overlay_bezier_handles(bezier, segment_id, transform, is_selected, overlay_context);
					});
				}
				DrawHandles::SelectedAnchors(ref selected_segments) => {
					vector
						.segment_bezier_iter()
						.filter(|(segment_id, ..)| selected_segments.contains(segment_id))
						.for_each(|(segment_id, bezier, _start, _end)| {
							overlay_bezier_handles(bezier, segment_id, transform, is_selected, overlay_context);
						});

					for (segment_id, bezier, start, end) in vector.segment_bezier_iter() {
						if let Some((corresponding_anchor, _)) = opposite_handles_data.iter().find(|(_, adj_segment_id)| adj_segment_id == &segment_id) {
							overlay_bezier_handle_specific_point(bezier, segment_id, (start, end), *corresponding_anchor, transform, is_selected, overlay_context);
						}
					}
				}
				DrawHandles::FrontierHandles(ref segment_endpoints) => {
					vector
						.segment_bezier_iter()
						.filter(|(segment_id, ..)| segment_endpoints.contains_key(segment_id))
						.for_each(|(segment_id, bezier, start, end)| {
							if segment_endpoints.get(&segment_id).unwrap().len() == 1 {
								let point_to_render = segment_endpoints.get(&segment_id).unwrap()[0];
								overlay_bezier_handle_specific_point(bezier, segment_id, (start, end), point_to_render, transform, is_selected, overlay_context);
							} else {
								overlay_bezier_handles(bezier, segment_id, transform, is_selected, overlay_context);
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

pub fn path_endpoint_overlays(document: &DocumentMessageHandler, shape_editor: &mut ShapeState, overlay_context: &mut OverlayContext, preferences: &PreferencesMessageHandler) {
	if !overlay_context.visibility_settings.anchors() {
		return;
	}

	for layer in document.network_interface.selected_nodes().selected_layers(document.metadata()) {
		let Some(vector) = document.network_interface.compute_modified_vector(layer) else {
			continue;
		};
		//let document_to_viewport = document.navigation_handler.calculate_offset_transform(overlay_context.size / 2., &document.document_ptz);
		let transform = document.metadata().transform_to_viewport_if_feeds(layer, &document.network_interface);
		let selected = shape_editor.selected_shape_state.get(&layer);
		let is_selected = |selected: Option<&SelectedLayerState>, point: ManipulatorPointId| selected.is_some_and(|selected| selected.is_point_selected(point));

		for point in vector.extendable_points(preferences.vector_meshes) {
			let Some(position) = vector.point_domain.position_from_id(point) else { continue };
			let position = transform.transform_point2(position);
			overlay_context.manipulator_anchor(position, is_selected(selected, ManipulatorPointId::Anchor(point)), None);
		}
	}
}

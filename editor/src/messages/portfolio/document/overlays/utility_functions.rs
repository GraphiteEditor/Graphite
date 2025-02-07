use super::utility_types::{DrawHandles, OverlayContext};
use crate::consts::HIDE_HANDLE_DISTANCE;
use crate::messages::tool::common_functionality::shape_editor::{SelectedLayerState, ShapeState};
use crate::messages::tool::tool_messages::tool_prelude::{DocumentMessageHandler, PreferencesMessageHandler};

use graphene_core::vector::ManipulatorPointId;

use bezier_rs::Bezier;
use glam::{DAffine2, DVec2};
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

pub fn get_selected_segments(document: &DocumentMessageHandler, shape_editor: &mut ShapeState) -> Vec<SegmentId> {
	let selected_points = shape_editor.selected_points();
	let selected_anchors: Vec<PointId> = selected_points
		.filter_map(|point_id| if let ManipulatorPointId::Anchor(p) = point_id { Some(*p) } else { None })
		.collect();
	//Collect the segments whose handles are selected
	let mut selected_segments: Vec<SegmentId> = shape_editor
		.selected_points()
		.filter_map(|point_id| {
			if let ManipulatorPointId::EndHandle(segment_id) = point_id {
				Some(*segment_id)
			} else if let ManipulatorPointId::PrimaryHandle(segment_id) = point_id {
				Some(*segment_id)
			} else {
				None
			}
		})
		.collect();
	//TODO: Currently if there are two duplicate layers, both of their segments get overlays
	// Segments of which the selected anchors are a part of
	for layer in document.network_interface.selected_nodes(&[]).unwrap().selected_layers(document.metadata()) {
		let Some(vector_data) = document.network_interface.compute_modified_vector(layer) else {
			continue;
		};
		for (segment_id, _bezier, start, end) in vector_data.segment_bezier_iter() {
			if selected_anchors.contains(&start) || selected_anchors.contains(&end) {
				selected_segments.push(segment_id);
			}
		}
	}
	selected_segments
}

fn overlay_bezier_handles(
	segment_id: SegmentId,
	bezier: Bezier,
	transform: DAffine2,
	overlay_context: &mut OverlayContext,
	selected: Option<&SelectedLayerState>,
	is_selected: impl Fn(Option<&SelectedLayerState>, ManipulatorPointId) -> bool,
) {
	let bezier = bezier.apply_transformation(|point| transform.transform_point2(point));
	let not_under_anchor = |position: DVec2, anchor: DVec2| position.distance_squared(anchor) >= HIDE_HANDLE_DISTANCE * HIDE_HANDLE_DISTANCE;
	match bezier.handles {
		bezier_rs::BezierHandles::Quadratic { handle } if not_under_anchor(handle, bezier.start) && not_under_anchor(handle, bezier.end) => {
			overlay_context.line(handle, bezier.start, None);
			overlay_context.line(handle, bezier.end, None);
			overlay_context.manipulator_handle(handle, is_selected(selected, ManipulatorPointId::PrimaryHandle(segment_id)), None);
		}
		bezier_rs::BezierHandles::Cubic { handle_start, handle_end } => {
			if not_under_anchor(handle_start, bezier.start) {
				overlay_context.line(handle_start, bezier.start, None);
				overlay_context.manipulator_handle(handle_start, is_selected(selected, ManipulatorPointId::PrimaryHandle(segment_id)), None);
			}
			if not_under_anchor(handle_end, bezier.end) {
				overlay_context.line(handle_end, bezier.end, None);
				overlay_context.manipulator_handle(handle_end, is_selected(selected, ManipulatorPointId::EndHandle(segment_id)), None);
			}
		}
		_ => {}
	}
}

pub fn path_overlays(document: &DocumentMessageHandler, shape_editor: &mut ShapeState, overlay_context: &mut OverlayContext, draw_handles: DrawHandles) {
	for layer in document.network_interface.selected_nodes(&[]).unwrap().selected_layers(document.metadata()) {
		let Some(vector_data) = document.network_interface.compute_modified_vector(layer) else {
			continue;
		};
		//let document_to_viewport = document.navigation_handler.calculate_offset_transform(overlay_context.size / 2., &document.document_ptz);
		let transform = document.metadata().transform_to_viewport(layer);
		let selected = shape_editor.selected_shape_state.get(&layer);
		let is_selected = |selected: Option<&SelectedLayerState>, point: ManipulatorPointId| selected.is_some_and(|selected| selected.is_selected(point));
		overlay_context.outline_vector(&vector_data, transform);

		//TODO: Here define which handles to show and which handles to not, for path tool selection
		match draw_handles {
			DrawHandles::All => {
				vector_data.segment_bezier_iter().for_each(|(segment_id, bezier, _start, _end)| {
					overlay_bezier_handles(segment_id, bezier, transform, overlay_context, selected, is_selected);
				});
			}
			DrawHandles::SelectedAnchors(ref selected_segments) => {
				vector_data
					.segment_bezier_iter()
					.filter(|(segment_id, ..)| selected_segments.contains(segment_id))
					.for_each(|(segment_id, bezier, _start, _end)| {
						overlay_bezier_handles(segment_id, bezier, transform, overlay_context, selected, is_selected);
					});
			}

			DrawHandles::FrontierHandles(ref segment_endpoints) => {
				vector_data
					.segment_bezier_iter()
					.filter(|(segment_id, ..)| segment_endpoints.contains_key(&segment_id))
					.for_each(|(segment_id, bezier, start, end)| {
						if segment_endpoints.get(&segment_id).unwrap().len() == 1 {
							let point_to_render = segment_endpoints.get(&segment_id).unwrap()[0];
							let bezier = bezier.apply_transformation(|point| transform.transform_point2(point));
							let not_under_anchor = |position: DVec2, anchor: DVec2| position.distance_squared(anchor) >= HIDE_HANDLE_DISTANCE * HIDE_HANDLE_DISTANCE;
							match bezier.handles {
								bezier_rs::BezierHandles::Quadratic { handle } if not_under_anchor(handle, bezier.start) && not_under_anchor(handle, bezier.end) => {
									if start == point_to_render {
										overlay_context.line(handle, bezier.start, None);
									} else {
										overlay_context.line(handle, bezier.end, None);
									}
									overlay_context.manipulator_handle(handle, is_selected(selected, ManipulatorPointId::PrimaryHandle(segment_id)), None);
								}
								bezier_rs::BezierHandles::Cubic { handle_start, handle_end } => {
									if not_under_anchor(handle_start, bezier.start) && (point_to_render == start) {
										overlay_context.line(handle_start, bezier.start, None);
										overlay_context.manipulator_handle(handle_start, is_selected(selected, ManipulatorPointId::PrimaryHandle(segment_id)), None);
									}
									if not_under_anchor(handle_end, bezier.end) && (point_to_render == end) {
										overlay_context.line(handle_end, bezier.end, None);
										overlay_context.manipulator_handle(handle_end, is_selected(selected, ManipulatorPointId::EndHandle(segment_id)), None);
									}
								}
								_ => {}
							}
						} else {
							overlay_bezier_handles(segment_id, bezier, transform, overlay_context, selected, is_selected);
						}
					});
			}
			DrawHandles::None => {}
		}

		for (&id, &position) in vector_data.point_domain.ids().iter().zip(vector_data.point_domain.positions()) {
			overlay_context.manipulator_anchor(transform.transform_point2(position), is_selected(selected, ManipulatorPointId::Anchor(id)), None);
		}
	}
}

pub fn path_endpoint_overlays(document: &DocumentMessageHandler, shape_editor: &mut ShapeState, overlay_context: &mut OverlayContext, preferences: &PreferencesMessageHandler) {
	for layer in document.network_interface.selected_nodes(&[]).unwrap().selected_layers(document.metadata()) {
		let Some(vector_data) = document.network_interface.compute_modified_vector(layer) else {
			continue;
		};
		//let document_to_viewport = document.navigation_handler.calculate_offset_transform(overlay_context.size / 2., &document.document_ptz);
		let transform = document.metadata().transform_to_viewport(layer);
		let selected = shape_editor.selected_shape_state.get(&layer);
		let is_selected = |selected: Option<&SelectedLayerState>, point: ManipulatorPointId| selected.is_some_and(|selected| selected.is_selected(point));

		for point in vector_data.extendable_points(preferences.vector_meshes) {
			let Some(position) = vector_data.point_domain.position_from_id(point) else { continue };
			let position = transform.transform_point2(position);
			overlay_context.manipulator_anchor(position, is_selected(selected, ManipulatorPointId::Anchor(point)), None);
		}
	}
}

use super::utility_types::OverlayContext;
use crate::consts::HIDE_HANDLE_DISTANCE;
use crate::messages::tool::common_functionality::shape_editor::{SelectedLayerState, ShapeState};
use crate::messages::tool::tool_messages::tool_prelude::DocumentMessageHandler;

use graphene_core::vector::ManipulatorPointId;

use glam::DVec2;
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

pub fn path_overlays(document: &DocumentMessageHandler, shape_editor: &mut ShapeState, overlay_context: &mut OverlayContext) {
	for layer in document.selected_nodes.selected_layers(document.metadata()) {
		let Some(vector_data) = document.metadata.compute_modified_vector(layer, &document.network) else {
			continue;
		};
		let transform = document.metadata().transform_to_viewport(layer);
		let selected = shape_editor.selected_shape_state.get(&layer);
		let is_selected = |selected: Option<&SelectedLayerState>, point: ManipulatorPointId| selected.is_some_and(|selected| selected.is_selected(point));
		overlay_context.outline_vector(&vector_data, transform);

		for (segment_id, bezier, _start, _end) in vector_data.segment_bezier_iter() {
			let bezier = bezier.apply_transformation(|point| transform.transform_point2(point));
			let not_under_anchor = |position: DVec2, anchor: DVec2| position.distance_squared(anchor) >= HIDE_HANDLE_DISTANCE * HIDE_HANDLE_DISTANCE;
			match bezier.handles {
				bezier_rs::BezierHandles::Quadratic { handle } if not_under_anchor(handle, bezier.start) && not_under_anchor(handle, bezier.end) => {
					overlay_context.line(handle, bezier.start);
					overlay_context.line(handle, bezier.end);
					overlay_context.manipulator_handle(handle, is_selected(selected, ManipulatorPointId::PrimaryHandle(segment_id)));
				}
				bezier_rs::BezierHandles::Cubic { handle_start, handle_end } => {
					if not_under_anchor(handle_start, bezier.start) {
						overlay_context.line(handle_start, bezier.start);
						overlay_context.manipulator_handle(handle_start, is_selected(selected, ManipulatorPointId::PrimaryHandle(segment_id)));
					}
					if not_under_anchor(handle_end, bezier.end) {
						overlay_context.line(handle_end, bezier.end);
						overlay_context.manipulator_handle(handle_end, is_selected(selected, ManipulatorPointId::EndHandle(segment_id)));
					}
				}
				_ => {}
			}
		}
		for (&id, &position) in vector_data.point_domain.ids().iter().zip(vector_data.point_domain.positions()) {
			overlay_context.manipulator_anchor(transform.transform_point2(position), is_selected(selected, ManipulatorPointId::Anchor(id)), None);
		}
	}
}

pub fn path_endpoint_overlays(document: &DocumentMessageHandler, shape_editor: &mut ShapeState, overlay_context: &mut OverlayContext) {
	for layer in document.selected_nodes.selected_layers(document.metadata()) {
		let Some(vector_data) = document.metadata.compute_modified_vector(layer, &document.network) else {
			continue;
		};
		let transform = document.metadata().transform_to_viewport(layer);
		let selected = shape_editor.selected_shape_state.get(&layer);
		let is_selected = |selected: Option<&SelectedLayerState>, point: ManipulatorPointId| selected.is_some_and(|selected| selected.is_selected(point));

		for point in vector_data.single_connected_points() {
			let Some(position) = vector_data.point_domain.position_from_id(point) else { continue };
			let position = transform.transform_point2(position);
			overlay_context.manipulator_anchor(position, is_selected(selected, ManipulatorPointId::Anchor(point)), None);
		}
	}
}

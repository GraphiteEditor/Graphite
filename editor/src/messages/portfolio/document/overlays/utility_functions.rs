use super::utility_types::OverlayContext;
use crate::consts::HIDE_HANDLE_DISTANCE;
use crate::messages::tool::common_functionality::graph_modification_utils::{get_manipulator_groups, get_subpaths};
use crate::messages::tool::common_functionality::shape_editor::{SelectedLayerState, ShapeState};
use crate::messages::tool::tool_messages::tool_prelude::DocumentMessageHandler;

use graphene_core::vector::{ManipulatorPointId, SelectedType};

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
	// for layer in document.selected_nodes.selected_layers(document.metadata()) {
	// 	let Some(vector_data) = document.metadata.compute_modified_vector(layer, &document.network) else {
	// 		continue;
	// 	};
	// 	let transform = document.metadata().transform_to_viewport(layer);
	// 	let selected = shape_editor.selected_shape_state.get(&layer);
	// 	let is_selected = |selected: Option<&SelectedLayerState>, point: ManipulatorPointId| selected.is_some_and(|selected| selected.is_selected(point));
	// 	overlay_context.outline(vector_data.stroke_bezier_paths(), transform);

	// 	for manipulator_group in vector_data.manipulator_groups() {
	// 		let anchor = manipulator_group.anchor;
	// 		let anchor_position = transform.transform_point2(anchor);

	// 		let not_under_anchor = |&position: &DVec2| transform.transform_point2(position).distance_squared(anchor_position) >= HIDE_HANDLE_DISTANCE * HIDE_HANDLE_DISTANCE;
	// 		if let Some(in_handle) = manipulator_group.in_handle.filter(not_under_anchor) {
	// 			let handle_position = transform.transform_point2(in_handle);
	// 			overlay_context.line(handle_position, anchor_position, None);
	// 			overlay_context.manipulator_handle(handle_position, is_selected(selected, ManipulatorPointId::new(manipulator_group.id.into(), SelectedType::InHandle)));
	// 		}
	// 		if let Some(out_handle) = manipulator_group.out_handle.filter(not_under_anchor) {
	// 			let handle_position = transform.transform_point2(out_handle);
	// 			overlay_context.line(handle_position, anchor_position, None);
	// 			overlay_context.manipulator_handle(handle_position, is_selected(selected, ManipulatorPointId::new(manipulator_group.id.into(), SelectedType::OutHandle)));
	// 		}

	// 		overlay_context.manipulator_anchor(anchor_position, is_selected(selected, ManipulatorPointId::new(manipulator_group.id.into(), SelectedType::Anchor)), None);
	// 	}
	// }
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
			let Some(position) = vector_data.point_domain.pos_from_id(point) else { continue };
			let position = transform.transform_point2(position);
			overlay_context.manipulator_anchor(position, is_selected(selected, ManipulatorPointId::Anchor(point)), None);
		}
	}
}

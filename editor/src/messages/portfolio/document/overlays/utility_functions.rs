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
	for layer in document.selected_nodes.selected_layers(document.metadata()) {
		let Some(subpaths) = get_subpaths(layer, &document.network) else { continue };
		let transform = document.metadata().transform_to_viewport(layer);
		let selected = shape_editor.selected_shape_state.get(&layer);
		let is_selected = |selected: Option<&SelectedLayerState>, point: ManipulatorPointId| selected.is_some_and(|selected| selected.is_selected(point));
		overlay_context.outline(subpaths.iter(), transform);

		for manipulator_group in get_manipulator_groups(subpaths) {
			let anchor = manipulator_group.anchor;
			let anchor_position = transform.transform_point2(anchor);

			let not_under_anchor = |&position: &DVec2| transform.transform_point2(position).distance_squared(anchor_position) >= HIDE_HANDLE_DISTANCE * HIDE_HANDLE_DISTANCE;
			if let Some(in_handle) = manipulator_group.in_handle.filter(not_under_anchor) {
				let handle_position = transform.transform_point2(in_handle);
				overlay_context.line(handle_position, anchor_position, None);
				overlay_context.manipulator_handle(handle_position, is_selected(selected, ManipulatorPointId::new(manipulator_group.id, SelectedType::InHandle)));
			}
			if let Some(out_handle) = manipulator_group.out_handle.filter(not_under_anchor) {
				let handle_position = transform.transform_point2(out_handle);
				overlay_context.line(handle_position, anchor_position, None);
				overlay_context.manipulator_handle(handle_position, is_selected(selected, ManipulatorPointId::new(manipulator_group.id, SelectedType::OutHandle)));
			}

			overlay_context.manipulator_anchor(anchor_position, is_selected(selected, ManipulatorPointId::new(manipulator_group.id, SelectedType::Anchor)), None);
		}
	}
}

pub fn path_endpoint_overlays(document: &DocumentMessageHandler, shape_editor: &mut ShapeState, overlay_context: &mut OverlayContext) {
	for layer in document.selected_nodes.selected_layers(document.metadata()) {
		let Some(subpaths) = get_subpaths(layer, &document.network) else { continue };
		let transform = document.metadata().transform_to_viewport(layer);
		let selected = shape_editor.selected_shape_state.get(&layer);
		let is_selected = |selected: Option<&SelectedLayerState>, point: ManipulatorPointId| selected.is_some_and(|selected| selected.is_selected(point));

		let mut manipulator_groups = get_manipulator_groups(subpaths);

		if let Some(first_manipulator) = manipulator_groups.next() {
			let anchor = first_manipulator.anchor;
			let anchor_position = transform.transform_point2(anchor);

			overlay_context.manipulator_anchor(anchor_position, is_selected(selected, ManipulatorPointId::new(first_manipulator.id, SelectedType::Anchor)), None);
		};

		if let Some(last_manipulator) = manipulator_groups.last() {
			let anchor = last_manipulator.anchor;
			let anchor_position = transform.transform_point2(anchor);

			overlay_context.manipulator_anchor(anchor_position, is_selected(selected, ManipulatorPointId::new(last_manipulator.id, SelectedType::Anchor)), None);
		};
	}
}

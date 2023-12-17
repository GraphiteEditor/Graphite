use crate::consts::HIDE_HANDLE_DISTANCE;
use crate::messages::portfolio::document::overlays::OverlayContext;
use crate::messages::tool::common_functionality::graph_modification_utils::{get_manipulator_groups, get_subpaths};
use crate::messages::tool::common_functionality::shape_editor::{SelectedLayerState, ShapeState};
use crate::messages::tool::tool_messages::tool_prelude::DocumentMessageHandler;
use glam::DVec2;
use graphene_core::vector::{ManipulatorPointId, SelectedType};

pub fn path_overlays(document: &DocumentMessageHandler, shape_editor: &mut ShapeState, overlay_context: &mut OverlayContext) {
	for layer in document.metadata().selected_layers() {
		let Some(subpaths) = get_subpaths(layer, &document.document_legacy) else { continue };
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
				overlay_context.line(handle_position, anchor_position);
				overlay_context.handle(handle_position, is_selected(selected, ManipulatorPointId::new(manipulator_group.id, SelectedType::InHandle)));
			}
			if let Some(out_handle) = manipulator_group.out_handle.filter(not_under_anchor) {
				let handle_position = transform.transform_point2(out_handle);
				overlay_context.line(handle_position, anchor_position);
				overlay_context.handle(handle_position, is_selected(selected, ManipulatorPointId::new(manipulator_group.id, SelectedType::OutHandle)));
			}

			overlay_context.square(anchor_position, is_selected(selected, ManipulatorPointId::new(manipulator_group.id, SelectedType::Anchor)));
		}
	}
}

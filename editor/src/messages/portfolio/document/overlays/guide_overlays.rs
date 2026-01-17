use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::prelude::DocumentMessageHandler;
use glam::DVec2;

const GUIDE_COLOR: &str = "#00BFFF";
const GUIDE_HOVER_COLOR: &str = "#FF6600";

pub fn guide_overlay(document: &DocumentMessageHandler, overlay_context: &mut OverlayContext) {
	let document_to_viewport = document
		.navigation_handler
		.calculate_offset_transform(overlay_context.viewport.center_in_viewport_space().into(), &document.document_ptz);

	let viewport_size: DVec2 = overlay_context.viewport.size().into();

	for guide in &document.horizontal_guides {
		let guide_point_viewport = document_to_viewport.transform_point2(DVec2::new(0.0, guide.position));
		let viewport_y = guide_point_viewport.y;

		let start = DVec2::new(0.0, viewport_y);
		let end = DVec2::new(viewport_size.x, viewport_y);
		let color = if document.hovered_guide_id == Some(guide.id) { GUIDE_HOVER_COLOR } else { GUIDE_COLOR };
		overlay_context.line(start, end, Some(color), None);
	}

	for guide in &document.vertical_guides {
		let guide_point_viewport = document_to_viewport.transform_point2(DVec2::new(guide.position, 0.0));
		let viewport_x = guide_point_viewport.x;

		let start = DVec2::new(viewport_x, 0.0);
		let end = DVec2::new(viewport_x, viewport_size.y);
		let color = if document.hovered_guide_id == Some(guide.id) { GUIDE_HOVER_COLOR } else { GUIDE_COLOR };
		overlay_context.line(start, end, Some(color), None);
	}
}

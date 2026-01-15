use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::prelude::DocumentMessageHandler;
use glam::DVec2;
use graphene_std::renderer::Quad;

const GUIDE_COLOR: &str = "#00BFFF";

pub fn guide_overlay(document: &DocumentMessageHandler, overlay_context: &mut OverlayContext) {
	let document_to_viewport = document
		.navigation_handler
		.calculate_offset_transform(overlay_context.viewport.center_in_viewport_space().into(), &document.document_ptz);

	let bounds = document_to_viewport.inverse() * Quad::from_box([DVec2::ZERO, overlay_context.viewport.size().into()]);

	let [min, max] = bounds.bounding_box();
	let (min_x, max_x) = (min.x, max.x);
	let (min_y, max_y) = (min.y, max.y);

	for guide in &document.horizontal_guides {
		let start = DVec2::new(min_x, guide.position);
		let end = DVec2::new(max_x, guide.position);
		overlay_context.line(document_to_viewport.transform_point2(start), document_to_viewport.transform_point2(end), Some(GUIDE_COLOR), None);
	}

	for guide in &document.vertical_guides {
		let start = DVec2::new(guide.position, min_y);
		let end = DVec2::new(guide.position, max_y);
		overlay_context.line(document_to_viewport.transform_point2(start), document_to_viewport.transform_point2(end), Some(GUIDE_COLOR), None);
	}
}

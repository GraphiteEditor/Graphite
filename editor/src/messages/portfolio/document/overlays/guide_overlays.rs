use crate::consts::COLOR_GUIDE_LINE;
use crate::messages::portfolio::document::document_message_handler::DocumentMessageHandler;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::guide::GuideLineDirection;
use glam::DVec2;
use graphene_std::renderer::Quad;

/// Draws each guide line across the viewport.
///
/// The viewport quad is transformed into document space once, so each line just needs its fixed position and the extent
/// of that quad on its perpendicular axis, rather than being extended toward the viewport a corner at a time.
pub fn draw_guide_lines(document: &DocumentMessageHandler, overlay_context: &mut OverlayContext) {
	let handler = &document.guide_lines_message_handler;
	let document_to_viewport = document
		.navigation_handler
		.calculate_offset_transform(overlay_context.viewport.center_in_viewport_space().into(), &document.document_ptz);
	let document_quad = document_to_viewport.inverse() * Quad::from_box([DVec2::ZERO, overlay_context.viewport.size().into()]);

	// The span a line runs along is the full extent of the visible quad on its perpendicular axis, so a horizontal line
	// takes the quad's width and a vertical one its height.
	let span = |axis: usize| {
		let corners = document_quad.0;
		let min = corners.iter().map(|&corner| corner[axis]).fold(f64::INFINITY, f64::min);
		let max = corners.iter().map(|&corner| corner[axis]).fold(f64::NEG_INFINITY, f64::max);
		(min, max)
	};
	let (horizontal_start, horizontal_end) = span(0);
	let (vertical_start, vertical_end) = span(1);

	let color = COLOR_GUIDE_LINE;
	for guide_line in &handler.guide_lines {
		let (start, end) = match guide_line.direction {
			GuideLineDirection::Horizontal => (DVec2::new(horizontal_start, guide_line.position), DVec2::new(horizontal_end, guide_line.position)),
			GuideLineDirection::Vertical => (DVec2::new(guide_line.position, vertical_start), DVec2::new(guide_line.position, vertical_end)),
		};

		overlay_context.line(document_to_viewport.transform_point2(start), document_to_viewport.transform_point2(end), Some(color), None);
	}
}

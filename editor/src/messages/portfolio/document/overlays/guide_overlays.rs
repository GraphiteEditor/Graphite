use crate::consts::{COLOR_OVERLAY_BLUE, COLOR_OVERLAY_BLUE_50};
use crate::messages::portfolio::document::guide_message_handler::GuideMessageHandler;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::guide::GuideDirection;
use glam::{DAffine2, DVec2};

fn extend_line_to_viewport(point: DVec2, direction: DVec2, viewport_size: DVec2) -> Option<(DVec2, DVec2)> {
	let dir = direction.try_normalize()?;

	// Calculates t values for intersections with viewport edges
	let mut t_values = Vec::new();

	let edges = graphene_std::renderer::Quad::from_box([DVec2::ZERO, viewport_size]).all_edges();
	for [start, end] in edges {
		let t_along_viewport = (point - start).perp_dot(dir) / (end - start).perp_dot(dir);
		let t_along_direction = (point - start).perp_dot(end - start) / (end - start).perp_dot(dir);
		if 0. <= t_along_viewport && t_along_viewport <= 1. && t_along_direction.is_finite() {
			t_values.push(t_along_direction);
		}
	}

	if t_values.len() < 2 {
		return None;
	}

	let t_min = t_values.iter().cloned().fold(f64::INFINITY, f64::min);
	let t_max = t_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

	let start = point + dir * t_min;
	let end = point + dir * t_max;

	Some((start, end))
}

pub fn guide_overlay(guide_handler: &GuideMessageHandler, overlay_context: &mut OverlayContext, document_to_viewport: DAffine2) {
	let viewport_size: DVec2 = overlay_context.viewport.size().into();

	for guide in &guide_handler.guides {
		let (doc_point, doc_direction) = match guide.direction {
			GuideDirection::Horizontal => (DVec2::new(0.0, guide.position), DVec2::X),
			GuideDirection::Vertical => (DVec2::new(guide.position, 0.0), DVec2::Y),
		};

		let viewport_point = document_to_viewport.transform_point2(doc_point);
		let viewport_direction = document_to_viewport.transform_vector2(doc_direction);

		let color = if guide_handler.hovered_guide_id == Some(guide.id) {
			COLOR_OVERLAY_BLUE_50
		} else {
			COLOR_OVERLAY_BLUE
		};

		if let Some((start, end)) = extend_line_to_viewport(viewport_point, viewport_direction, viewport_size) {
			overlay_context.line(start, end, Some(color), None);
		}
	}
}

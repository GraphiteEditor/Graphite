use crate::consts::{COLOR_OVERLAY_BLUE, COLOR_OVERLAY_BLUE_50};
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::prelude::DocumentMessageHandler;
use glam::DVec2;

fn extend_line_to_viewport(point: DVec2, direction: DVec2, viewport_size: DVec2) -> Option<(DVec2, DVec2)> {
	let dir = direction.try_normalize()?;

	// Calculates t values for intersections with viewport edges
	let mut t_values = Vec::new();

	if dir.x.abs() > f64::EPSILON {
		let t = -point.x / dir.x;
		let y = point.y + t * dir.y;
		if y >= 0.0 && y <= viewport_size.y {
			t_values.push(t);
		}
	}

	// Right edge (x = viewport_size.x)
	if dir.x.abs() > f64::EPSILON {
		let t = (viewport_size.x - point.x) / dir.x;
		let y = point.y + t * dir.y;
		if y >= 0.0 && y <= viewport_size.y {
			t_values.push(t);
		}
	}

	// Top edge (y = 0)
	if dir.y.abs() > f64::EPSILON {
		let t = -point.y / dir.y;
		let x = point.x + t * dir.x;
		if x >= 0.0 && x <= viewport_size.x {
			t_values.push(t);
		}
	}

	// Bottom edge (y = viewport_size.y)
	if dir.y.abs() > f64::EPSILON {
		let t = (viewport_size.y - point.y) / dir.y;
		let x = point.x + t * dir.x;
		if x >= 0.0 && x <= viewport_size.x {
			t_values.push(t);
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

pub fn guide_overlay(document: &DocumentMessageHandler, overlay_context: &mut OverlayContext) {
	let document_to_viewport = document
		.navigation_handler
		.calculate_offset_transform(overlay_context.viewport.center_in_viewport_space().into(), &document.document_ptz);

	let viewport_size: DVec2 = overlay_context.viewport.size().into();

	for guide in &document.horizontal_guides {
		let doc_point = DVec2::new(0.0, guide.position);
		let doc_direction = DVec2::X; // Horizontal guides run in the X direction in document space

		let viewport_point = document_to_viewport.transform_point2(doc_point);
		let viewport_direction = document_to_viewport.transform_vector2(doc_direction);

		let color = if document.hovered_guide_id == Some(guide.id) { COLOR_OVERLAY_BLUE_50 } else { COLOR_OVERLAY_BLUE };

		if let Some((start, end)) = extend_line_to_viewport(viewport_point, viewport_direction, viewport_size) {
			overlay_context.line(start, end, Some(color), None);
		}
	}

	for guide in &document.vertical_guides {
		let doc_point = DVec2::new(guide.position, 0.0);
		let doc_direction = DVec2::Y;

		let viewport_point = document_to_viewport.transform_point2(doc_point);
		let viewport_direction = document_to_viewport.transform_vector2(doc_direction);

		let color = if document.hovered_guide_id == Some(guide.id) { COLOR_OVERLAY_BLUE_50 } else { COLOR_OVERLAY_BLUE };

		if let Some((start, end)) = extend_line_to_viewport(viewport_point, viewport_direction, viewport_size) {
			overlay_context.line(start, end, Some(color), None);
		}
	}
}

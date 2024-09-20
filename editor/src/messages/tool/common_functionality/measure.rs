use crate::messages::portfolio::document::overlays::utility_types::{self, OverlayContext};
use crate::messages::tool::tool_messages::tool_prelude::*;

use graphene_std::renderer::Rect;

pub fn overlay(selected_bounds: Rect, hovered_bounds: Rect, transform: DAffine2, document_to_viewport: DAffine2, overlay_context: &mut OverlayContext) {
	let transform_to_document = document_to_viewport.inverse() * transform;
	if selected_bounds.intersects(hovered_bounds) {
		// TODO: I'm not sure what to do here?
		return;
	}

	// Always do horizontal then vertical from the selected
	let turn_x = selected_bounds.center().x.clamp(hovered_bounds.min().x, hovered_bounds.max().x);
	let turn_y = hovered_bounds.center().y.clamp(selected_bounds.min().y, selected_bounds.max().y);

	let selected_x = turn_x.clamp(selected_bounds.min().x, selected_bounds.max().x);
	let hovered_y = turn_y.clamp(hovered_bounds.min().y, hovered_bounds.max().y);

	if turn_x != selected_x {
		let min_viewport = transform.transform_point2(DVec2::new(turn_x.min(selected_x), turn_y));
		let max_viewport = transform.transform_point2(DVec2::new(turn_x.max(selected_x), turn_y));
		overlay_context.line(min_viewport, max_viewport, None);
		let length = format!("{:.2}", transform_to_document.transform_vector2(DVec2::X * (turn_x - selected_x)).length());
		let direction = -(min_viewport - max_viewport).normalize_or_zero();
		let transform = DAffine2::from_translation((min_viewport + max_viewport) / 2.) * DAffine2::from_angle(-direction.angle_to(DVec2::X));
		overlay_context.transformed_text(&length, transform, 5., "10px sans-serif", utility_types::Pivot::TopCentreX);
	}
	if turn_y != hovered_y {
		let min_viewport = transform.transform_point2(DVec2::new(turn_x, turn_y.min(hovered_y)));
		let max_viewport = transform.transform_point2(DVec2::new(turn_x, turn_y.max(hovered_y)));
		overlay_context.line(min_viewport, max_viewport, None);
		let length = format!("{:.2}", transform_to_document.transform_vector2(DVec2::Y * (turn_y - hovered_y)).length());
		let direction = (min_viewport - max_viewport).normalize_or_zero().perp();
		let transform = DAffine2::from_translation((min_viewport + max_viewport) / 2.) * DAffine2::from_angle(-direction.angle_to(DVec2::X));
		overlay_context.transformed_text(&length, transform, 5., "10px sans-serif", utility_types::Pivot::LeftCentreY);
	}
}

//! Draws a blue cross overlay at the origin point of the layers in layer space.
//!
//! This cross is orientated based on the +X vector of the layer.

use crate::consts::{BLUE_LAYER_ORIGIN_CROSS_DIAMETER, BLUE_LAYER_ORIGIN_CROSS_THICKNESS, COLOR_OVERLAY_BLUE};
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::tool_messages::tool_prelude::DocumentMessageHandler;
use glam::DVec2;

pub fn draw_for_selected_layers(overlay_context: &mut OverlayContext, document: &DocumentMessageHandler) {
	// Can be disabled
	if !overlay_context.visibility_settings.blue_layer_origin_cross() {
		return;
	}
	// Only show visible and unlocked and selected and layers
	for layer in document.network_interface.selected_nodes().selected_visible_and_unlocked_layers(&document.network_interface) {
		// Don't show artboards
		if document.network_interface.is_artboard(&layer.to_node(), &[]) {
			continue;
		}

		// Don't crash if we accidentally have the root.
		if layer == LayerNodeIdentifier::ROOT_PARENT {
			continue;
		}

		// Some layers such as groups don't have a local transform.
		if !document.metadata().local_transforms.contains_key(&layer.to_node()) {
			continue;
		}

		// A transformation from the layer's local space to the viewport space (where overlays are drawn)
		let transform_to_viewport = document.metadata().transform_to_viewport(layer);

		// The origin of the layer in viewport space which is the centre of the blue cross/
		let origin_viewport = transform_to_viewport.transform_point2(DVec2::ZERO);
		// The forward +X direction vector from layer space (used to orientate the blue orogin cross)
		let forward = transform_to_viewport.transform_vector2(DVec2::X).normalize_or_zero();

		// Draw the cross
		let offsets = [forward + forward.perp(), forward - forward.perp()].map(|offset| offset * core::f64::consts::FRAC_1_SQRT_2 * BLUE_LAYER_ORIGIN_CROSS_DIAMETER / 2.);
		for offset in offsets {
			overlay_context.line(origin_viewport - offset, origin_viewport + offset, Some(COLOR_OVERLAY_BLUE), Some(BLUE_LAYER_ORIGIN_CROSS_THICKNESS));
		}
	}
}

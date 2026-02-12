use crate::consts::{COLOR_OVERLAY_BLUE, LAYER_ORIGIN_CROSS_DIAMETER, LAYER_ORIGIN_CROSS_THICKNESS};
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::tool_messages::tool_prelude::DocumentMessageHandler;
use glam::DVec2;

/// Draws a cross overlay at the origin point of the layers in layer space.
/// This cross is orientated based on the +X vector of the layer.
pub fn draw_for_selected_layers(overlay_context: &mut OverlayContext, document: &DocumentMessageHandler) {
	// Don't draw if it is a disabled overlay
	if !overlay_context.visibility_settings.layer_origin_cross() {
		return;
	}

	// Only show for layers that are visible, unlocked, and selected
	for layer in document.network_interface.selected_nodes().selected_visible_and_unlocked_layers(&document.network_interface) {
		// Don't show for artboards
		if document.network_interface.is_artboard(&layer.to_node(), &[]) {
			continue;
		}

		// Don't crash if we accidentally have the root
		if layer == LayerNodeIdentifier::ROOT_PARENT {
			continue;
		}

		// Some layers such as groups don't have a local transform (although we'll likely design a fix for that fact later)
		if !document.metadata().local_transforms.contains_key(&layer.to_node()) {
			continue;
		}

		// A transformation from the layer's local space to the viewport space (where overlays are drawn)
		let transform_to_viewport = document.metadata().transform_to_viewport(layer);

		// The origin of the layer in viewport space which is the center of the origin cross
		let origin_viewport = transform_to_viewport.transform_point2(DVec2::ZERO);
		// The forward +X direction vector from layer space (used to orient the origin cross)
		let forward = transform_to_viewport.transform_vector2(DVec2::X).normalize_or_zero();

		// Draw the origin cross
		let offsets = [forward + forward.perp(), forward - forward.perp()].map(|offset| offset * core::f64::consts::FRAC_1_SQRT_2 * LAYER_ORIGIN_CROSS_DIAMETER / 2.);
		for offset in offsets {
			overlay_context.line(origin_viewport - offset, origin_viewport + offset, Some(COLOR_OVERLAY_BLUE), Some(LAYER_ORIGIN_CROSS_THICKNESS));
		}
	}
}

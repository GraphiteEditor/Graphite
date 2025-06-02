use crate::messages::input_mapper::utility_types::input_keyboard::Key;
use crate::messages::prelude::*;
use glam::DVec2;

#[impl_message(Message, DocumentMessage, Navigation)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum NavigationMessage {
	// Messages
	BeginCanvasPan,
	BeginCanvasTilt { was_dispatched_from_menu: bool },
	BeginCanvasZoom,
	CanvasPan { delta: DVec2 },
	CanvasPanAbortPrepare { x_not_y_axis: bool },
	CanvasPanAbort { x_not_y_axis: bool },
	CanvasPanByViewportFraction { delta: DVec2 },
	CanvasPanMouseWheel { use_y_as_x: bool },
	CanvasTiltResetAndZoomTo100Percent,
	CanvasTiltSet { angle_radians: f64 },
	CanvasZoomDecrease { center_on_mouse: bool },
	CanvasZoomIncrease { center_on_mouse: bool },
	CanvasZoomMouseWheel,
	CanvasZoomSet { zoom_factor: f64 },
	CanvasFlip,
	EndCanvasPTZ { abort_transform: bool },
	EndCanvasPTZWithClick { commit_key: Key },
	FitViewportToBounds { bounds: [DVec2; 2], prevent_zoom_past_100: bool },
	FitViewportToSelection,
	PointerMove { snap: Key },
}

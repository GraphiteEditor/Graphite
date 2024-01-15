use crate::messages::input_mapper::utility_types::input_keyboard::Key;
use crate::messages::prelude::*;

use glam::DVec2;
use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, DocumentMessage, Navigation)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum NavigationMessage {
	// Messages
	DecreaseCanvasZoom {
		center_on_mouse: bool,
	},
	FitViewportToBounds {
		bounds: [DVec2; 2],
		prevent_zoom_past_100: bool,
	},
	FitViewportToSelection,
	IncreaseCanvasZoom {
		center_on_mouse: bool,
	},
	PointerMove {
		snap_angle: Key,
		wait_for_snap_angle_release: bool,
		snap_zoom: Key,
		zoom_from_viewport: Option<DVec2>,
	},
	ResetCanvasTiltAndZoomTo100Percent,
	RotateCanvasBegin {
		was_dispatched_from_menu: bool,
	},
	SetCanvasTilt {
		angle_radians: f64,
	},
	SetCanvasZoom {
		zoom_factor: f64,
	},
	TransformCanvasEnd {
		abort_transform: bool,
	},
	TransformFromMenuEnd {
		commit_key: Key,
	},
	TranslateCanvas {
		delta: DVec2,
	},
	TranslateCanvasBegin,
	TranslateCanvasByViewportFraction {
		delta: DVec2,
	},
	WheelCanvasTranslate {
		use_y_as_x: bool,
	},
	WheelCanvasZoom,
	ZoomCanvasBegin,
}

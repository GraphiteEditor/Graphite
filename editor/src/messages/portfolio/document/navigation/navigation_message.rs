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
		padding_scale_factor: Option<f32>,
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
	RotateCanvasBegin,
	SetCanvasRotation {
		angle_radians: f64,
	},
	SetCanvasZoom {
		zoom_factor: f64,
	},
	TransformCanvasEnd,
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

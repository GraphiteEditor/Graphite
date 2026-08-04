use glam::DVec2;

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum NavigationOperation {
	#[default]
	None,
	Pan {
		pan_original_for_abort: DVec2,
	},
	Tilt {
		tilt_original_for_abort: f64,
		tilt_raw_not_snapped: f64,
		snap: bool,
	},
	Zoom {
		zoom_raw_not_snapped: f64,
		zoom_original_for_abort: f64,
		snap: bool,
	},
}

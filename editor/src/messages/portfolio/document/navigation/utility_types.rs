use glam::DVec2;

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum NavigationOperation {
	#[default]
	None,
	Pan {
		pre_commit_pan: DVec2,
	},
	Rotate {
		pre_commit_tilt: f64,
		snap_tilt: bool,
		snap_tilt_released: bool,
	},
	Zoom {
		pre_commit_zoom: f64,
		snap_zoom_enabled: bool,
	},
}

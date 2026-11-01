use graphite_desktop_wrapper::messages::Preferences;
use wgpu::PresentMode;

use crate::ui::Acceleration;

#[derive(Clone, PartialEq)]
pub(crate) struct DesktopPreferences {
	pub(crate) acceleration: Acceleration,
	pub(crate) present_mode: Option<PresentMode>,
	pub(crate) ui_scale: f64,
}

impl From<&Preferences> for DesktopPreferences {
	fn from(preferences: &Preferences) -> Self {
		#[cfg(not(target_os = "macos"))]
		let present_mode = None;
		#[cfg(target_os = "macos")]
		let present_mode = if !preferences.vsync { Some(PresentMode::Immediate) } else { None };

		let acceleration = if preferences.disable_ui_acceleration { Acceleration::Disabled } else { Acceleration::Auto };

		Self {
			acceleration,
			present_mode,
			ui_scale: preferences.ui_scale,
		}
	}
}

pub(crate) fn write(preferences: Preferences) {
	let Ok(preferences) = ron::ser::to_string_pretty(&preferences, Default::default()) else {
		tracing::error!("Failed to serialize preferences");
		return;
	};
	std::fs::write(file_path(), &preferences).unwrap_or_else(|e| {
		tracing::error!("Failed to write preferences to disk: {e}");
	});
}

pub(crate) fn read() -> Preferences {
	let Ok(data) = std::fs::read_to_string(file_path()) else {
		return Preferences::default();
	};
	let Ok(preferences) = ron::from_str(&data) else {
		return Preferences::default();
	};
	preferences
}

pub(crate) fn modify(f: impl FnOnce(&mut Preferences)) {
	let mut preferences = read();
	f(&mut preferences);
	write(preferences);
}

fn file_path() -> std::path::PathBuf {
	let mut path = crate::dirs::app_data_dir();
	path.push(crate::consts::APP_PREFERENCES_FILE_NAME);
	path
}

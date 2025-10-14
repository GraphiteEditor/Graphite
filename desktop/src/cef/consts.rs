use std::time::Duration;

pub(crate) const RESOURCE_SCHEME: &str = "resources";
pub(crate) const RESOURCE_DOMAIN: &str = "resources";

pub(crate) const SCROLL_LINE_HEIGHT: usize = 40;
pub(crate) const SCROLL_LINE_WIDTH: usize = 40;
pub(crate) const SCROLL_SPEED_X: f32 = 3.0;
pub(crate) const SCROLL_SPEED_Y: f32 = 3.0;

pub(crate) const PINCH_ZOOM_SPEED: f64 = 300.0;

pub(crate) const MULTICLICK_TIMEOUT: Duration = Duration::from_millis(500);
pub(crate) const MULTICLICK_ALLOWED_TRAVEL: usize = 4;

use std::time::Duration;

pub(crate) const RESOURCE_SCHEME: &str = "resources";
pub(crate) const RESOURCE_DOMAIN: &str = "resources";

pub(crate) const BROWSER_HOST_CONFIG_FLAG: &str = "--graphite-browser-host=";

pub(crate) const WINDOWLESS_FRAME_RATE: i32 = 60;
pub(crate) const FRAMES_IN_FLIGHT_LIMIT: u64 = 3;
pub(crate) const FRAME_SEGMENT_POOL_SIZE: u64 = FRAMES_IN_FLIGHT_LIMIT + 1; // allow one extra staged frame
pub(crate) const FRAME_SEGMENT_GRANULARITY: usize = 2 * 1024 * 1024; // 2 MiB

pub(crate) const HOST_HELLO_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const HOST_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

#[cfg(target_os = "macos")]
pub(crate) const IPC_BOOTSTRAP_PREFIX: &str = "art.graphite.Graphite.ipc.";

pub(crate) const SCROLL_LINE_HEIGHT: usize = 40;
pub(crate) const SCROLL_LINE_WIDTH: usize = 40;

#[cfg(target_os = "linux")]
pub(crate) const SCROLL_SPEED_X: f32 = 3.;
#[cfg(target_os = "linux")]
pub(crate) const SCROLL_SPEED_Y: f32 = 3.;

#[cfg(not(target_os = "linux"))]
pub(crate) const SCROLL_SPEED_X: f32 = 1.;
#[cfg(not(target_os = "linux"))]
pub(crate) const SCROLL_SPEED_Y: f32 = 1.;

pub(crate) const PINCH_ZOOM_SPEED: f64 = 300.;

pub(crate) const MULTICLICK_TIMEOUT: Duration = Duration::from_millis(500);
pub(crate) const MULTICLICK_ALLOWED_TRAVEL: usize = 4;

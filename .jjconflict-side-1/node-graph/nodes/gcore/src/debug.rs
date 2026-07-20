use core_types::Ctx;
use glam::{DAffine2, DVec2};

/// Meant for debugging purposes, not general use. Logs the input value to the console and passes it through unchanged.
#[node_macro::node(category("Debug"), name("Log to Console"))]
fn log_to_console<T: std::fmt::Debug>(_: impl Ctx, #[implementations(bool, f64, u32, u64, DVec2, DAffine2, String)] value: T) -> T {
	// KEEP THIS `debug!()` - It acts as the output for the debug node itself
	log::debug!("{value:#?}");
	value
}

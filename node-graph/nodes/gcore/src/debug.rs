use core_types::Ctx;
use core_types::list::{Item, List};
use glam::{DAffine2, DVec2};
use raster_types::{CPU, Raster};

/// Meant for debugging purposes, not general use. Logs the input value to the console and passes it through unchanged.
#[node_macro::node(category("Debug"), name("Log to Console"))]
fn log_to_console<T: std::fmt::Debug>(_: impl Ctx, #[implementations(bool, f64, u32, u64, DVec2, DAffine2, String)] value: Item<T>) -> Item<T> {
	// KEEP THIS `debug!()` - It acts as the output for the debug node itself
	log::debug!("{value:#?}");
	value
}

/// Meant for debugging purposes, not general use. Clones the input value.
#[node_macro::node(category("Debug"))]
fn clone<'i, T: Clone + 'i>(_: impl Ctx, #[implementations(&List<Raster<CPU>>)] value: &'i T) -> T {
	value.clone()
}

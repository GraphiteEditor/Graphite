use crate::Ctx;
use crate::raster_types::{CPU, Raster};
use crate::table::Table;
use glam::{DAffine2, DVec2};

/// Meant for debugging purposes, not general use. Logs the input value to the console and passes it through unchanged.
#[node_macro::node(category("Debug"), name("Log to Console"))]
fn log_to_console<T: std::fmt::Debug>(_: impl Ctx, #[implementations(bool, f64, u32, u64, DVec2, DAffine2, String)] value: T) -> T {
	// KEEP THIS `debug!()` - It acts as the output for the debug node itself
	log::debug!("{value:#?}");
	value
}

/// Meant for debugging purposes, not general use. Returns the size of the input type in bytes.
#[node_macro::node(category("Debug"))]
fn size_of(_: impl Ctx, ty: crate::Type) -> Option<usize> {
	ty.size()
}

/// Meant for debugging purposes, not general use. Wraps the input value in the Some variant of an Option.
#[node_macro::node(category("Debug"))]
fn some<T>(_: impl Ctx, #[implementations(f64, f32, u32, u64, String)] input: T) -> Option<T> {
	Some(input)
}

/// Meant for debugging purposes, not general use. Unwraps the input value from an Option, returning the default value if the input is None.
#[node_macro::node(category("Debug"))]
fn unwrap_option<T: Default>(_: impl Ctx, #[implementations(Option<f64>, Option<u32>, Option<u64>, Option<String>)] input: Option<T>) -> T {
	input.unwrap_or_default()
}

/// Meant for debugging purposes, not general use. Clones the input value.
#[node_macro::node(category("Debug"))]
fn clone<'i, T: Clone + 'i>(_: impl Ctx, #[implementations(&Table<Raster<CPU>>)] value: &'i T) -> T {
	value.clone()
}

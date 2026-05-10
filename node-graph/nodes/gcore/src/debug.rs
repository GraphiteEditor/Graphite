use core_types::Ctx;
use core_types::list::{Item, List};
use glam::{DAffine2, DVec2};
use raster_types::{CPU, Raster};

/// Meant for debugging purposes, not general use. Logs the input value to the console and passes it through unchanged.
#[node_macro::node(category("Debug"), name("Log to Console"))]
fn log_to_console<T: std::fmt::Debug>(_: impl Ctx, #[implementations(Item<bool>, Item<f64>, Item<u32>, Item<u64>, Item<DVec2>, Item<DAffine2>, Item<String>)] value: Item<T>) -> Item<T> {
	let value = value.into_element();
	// KEEP THIS `debug!()` - It acts as the output for the debug node itself
	log::debug!("{value:#?}");
	Item::new_from_element(value)
}

/// Meant for debugging purposes, not general use. Returns the size of the input type in bytes.
#[node_macro::node(category("Debug"))]
fn size_of(_: impl Ctx, ty: Item<core_types::Type>) -> Item<Option<usize>> {
	let ty = ty.into_element();
	Item::new_from_element(ty.size())
}

/// Meant for debugging purposes, not general use. Wraps the input value in the Some variant of an Option.
#[node_macro::node(category("Debug"))]
fn some<T>(_: impl Ctx, #[implementations(Item<f64>, Item<f32>, Item<u32>, Item<u64>, Item<String>)] input: Item<T>) -> Item<Option<T>> {
	let input = input.into_element();
	Item::new_from_element(Some(input))
}

/// Meant for debugging purposes, not general use. Unwraps the input value from an Option, returning the default value if the input is None.
#[node_macro::node(category("Debug"))]
fn unwrap_option<T: Default>(_: impl Ctx, #[implementations(Item<Option<f64>>, Item<Option<u32>>, Item<Option<u64>>, Item<Option<String>>)] input: Item<Option<T>>) -> Item<T> {
	let input = input.into_element();
	Item::new_from_element(input.unwrap_or_default())
}

/// Meant for debugging purposes, not general use. Clones the input value.
#[node_macro::node(category("Debug"))]
fn clone<'i, T: Clone + 'i>(_: impl Ctx, #[implementations(Item<&List<Raster<CPU>>>)] value: Item<&'i T>) -> Item<T> {
	let value = value.into_element();
	Item::new_from_element(value.clone())
}

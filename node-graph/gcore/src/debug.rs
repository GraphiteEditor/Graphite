use crate::raster_types::{CPU, RasterDataTable};
use crate::{Color, Ctx};

/// Meant for debugging purposes, not general use. Returns the size of the input type in bytes.
#[node_macro::node(category("Debug"))]
fn size_of(_: impl Ctx, ty: crate::Type) -> Option<usize> {
	ty.size()
}

/// Meant for debugging purposes, not general use. Wraps the input value in the Some variant of an Option.
#[node_macro::node(category("Debug"))]
fn some<T>(_: impl Ctx, #[implementations(f64, f32, u32, u64, String, Color)] input: T) -> Option<T> {
	Some(input)
}

/// Meant for debugging purposes, not general use. Unwraps the input value from an Option, returning the default value if the input is None.
#[node_macro::node(category("Debug"))]
fn unwrap<T: Default>(_: impl Ctx, #[implementations(Option<f64>, Option<f32>, Option<u32>, Option<u64>, Option<String>, Option<Color>)] input: Option<T>) -> T {
	input.unwrap_or_default()
}

/// Meant for debugging purposes, not general use. Clones the input value.
#[node_macro::node(category("Debug"))]
fn clone<'i, T: Clone + 'i>(_: impl Ctx, #[implementations(&RasterDataTable<CPU>)] value: &'i T) -> T {
	value.clone()
}

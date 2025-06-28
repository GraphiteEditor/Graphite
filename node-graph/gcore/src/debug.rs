use crate::GraphicGroupTable;
use crate::instances::Instances;
use crate::raster_types::{CPU, GPU, RasterDataTable};
use crate::vector::VectorDataTable;
use crate::{Color, Ctx};
use glam::{DAffine2, DVec2};

#[node_macro::node(category("Debug"), name("Log to Console"))]
fn log_to_console<T: std::fmt::Debug>(_: impl Ctx, #[implementations(String, bool, f64, u32, u64, DVec2, VectorDataTable, DAffine2, Color, Option<Color>)] value: T) -> T {
	// KEEP THIS `debug!()` - It acts as the output for the debug node itself
	log::debug!("{:#?}", value);
	value
}

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

/// Meant for debugging purposes, not general use. Retrieves the value from a Vec, if it exists, or otherwise the default value for the type.
#[node_macro::node(category("Debug"))]
fn index_vector<T: Clone + Default>(_: impl Ctx, #[implementations(Vec<Color>, Vec<Option<Color>>, Vec<f64>, Vec<u64>, Vec<DVec2>)] value: Vec<T>, index: u32) -> T {
	value.get(index as usize).cloned().unwrap_or_default()
}

/// Meant for debugging purposes, not general use. Creates a table with a single row containing the input index, if it exists.
#[node_macro::node(category("Debug"))]
async fn index_table<T: Clone>(_: impl Ctx, #[implementations(VectorDataTable, RasterDataTable<CPU>, RasterDataTable<GPU>, GraphicGroupTable)] value: Instances<T>, index: u32) -> Instances<T> {
	let mut result = Instances::default();
	if let Some(instance) = value.instance_iter().nth(index as usize) {
		result.push(instance.clone());
	}
	result
}

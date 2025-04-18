use crate::vector::VectorDataTable;
use crate::{Color, Context, Ctx};
use glam::{DAffine2, DVec2};

#[node_macro::node(category("Debug"))]
fn log_to_console<T: core::fmt::Debug>(_: impl Ctx, #[implementations(String, bool, f64, u32, u64, DVec2, VectorDataTable, DAffine2, Color, Option<Color>)] value: T) -> T {
	#[cfg(not(target_arch = "spirv"))]
	// KEEP THIS `debug!()` - It acts as the output for the debug node itself
	debug!("{:#?}", value);
	value
}

#[node_macro::node(category("Text"))]
fn to_string<T: core::fmt::Debug>(_: impl Ctx, #[implementations(String, bool, f64, u32, u64, DVec2, VectorDataTable, DAffine2)] value: T) -> String {
	format!("{:?}", value)
}

#[node_macro::node(category("Text"))]
fn string_concatenate(_: impl Ctx, #[implementations(String)] first: String, #[implementations(String)] second: String) -> String {
	first.clone() + &second
}

#[node_macro::node(category("Text"))]
fn string_replace(_: impl Ctx, #[implementations(String)] string: String, from: String, to: String) -> String {
	string.replace(&from, &to)
}

#[node_macro::node(category("Text"))]
fn string_slice(_: impl Ctx, #[implementations(String)] string: String, start: f64, end: f64) -> String {
	let start = if start < 0. { string.len() - start.abs() as usize } else { start as usize };
	let end = if end <= 0. { string.len() - end.abs() as usize } else { end as usize };
	let n = end.saturating_sub(start);
	string.char_indices().skip(start).take(n).map(|(_, c)| c).collect()
}

#[node_macro::node(category("Text"))]
fn split_string_by_index(
	_: impl Ctx,
	#[implementations(String)]
	/// The comma-separated string to split.
	input: String,
	/// The zero-based index of the item to retrieve.
	#[default(0.0)]
	#[min(0.0)]
	index: f64,
) -> String {
	let parts: Vec<&str> = input.split(',').map(|s| s.trim()).collect();
	let floored_index = index.floor() as usize;
	if floored_index < parts.len() { parts[floored_index].to_string() } else { String::new() }
}

#[node_macro::node(category("Text"))]
fn string_length(_: impl Ctx, #[implementations(String)] string: String) -> usize {
	string.len()
}

#[node_macro::node(category("Text"))]
async fn switch<T, C: Send + 'n + Clone>(
	#[implementations(Context)] ctx: C,
	condition: bool,
	#[expose]
	#[implementations(
		Context -> String, Context -> bool, Context -> f64, Context -> u32, Context -> u64, Context -> DVec2, Context -> VectorDataTable, Context -> DAffine2,
	)]
	if_true: impl Node<C, Output = T>,
	#[expose]
	#[implementations(
		Context -> String, Context -> bool, Context -> f64, Context -> u32, Context -> u64, Context -> DVec2, Context -> VectorDataTable, Context -> DAffine2,
	)]
	if_false: impl Node<C, Output = T>,
) -> T {
	if condition {
		// We can't remove these calls because we only want to evaluate the branch that we actually need
		if_true.eval(ctx).await
	} else {
		if_false.eval(ctx).await
	}
}

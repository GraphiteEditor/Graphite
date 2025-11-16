use crate::Artboard;
use crate::Color;
use crate::Graphic;
use crate::gradient::GradientStops;
use crate::graphene_core::registry::types::TextArea;
use crate::raster_types::{CPU, GPU, Raster};
use crate::table::Table;
use crate::vector::Vector;
use crate::{Context, Ctx};
use glam::{DAffine2, DVec2};

/// Type-asserts a value to be a string.
#[node_macro::node(category("Debug"))]
fn to_string(_: impl Ctx, value: String) -> String {
	value
}

/// Converts a value to a JSON string representation.
#[node_macro::node(category("Text"))]
fn serialize<T: serde::Serialize>(
	_: impl Ctx,
	#[implementations(String, bool, f64, u32, u64, DVec2, DAffine2, Table<Artboard>, Table<Graphic>, Table<Vector>, Table<Raster<CPU>>, Table<Color>, Table<GradientStops>)] value: T,
) -> String {
	serde_json::to_string(&value).unwrap_or_else(|_| "Serialization Error".to_string())
}

/// Joins two strings together.
#[node_macro::node(category("Text"))]
fn string_concatenate(_: impl Ctx, #[implementations(String)] first: String, second: TextArea) -> String {
	first.clone() + &second
}

/// Replaces all occurrences of "From" with "To" in the input string.
#[node_macro::node(category("Text"))]
fn string_replace(_: impl Ctx, string: String, from: TextArea, to: TextArea) -> String {
	string.replace(&from, &to)
}

/// Extracts a substring from the input string, starting at "Start" and ending before "End".
/// Negative indices count from the end of the string.
/// If "Start" equals or exceeds "End", the result is an empty string.
#[node_macro::node(category("Text"))]
fn string_slice(_: impl Ctx, string: String, start: f64, end: f64) -> String {
	let total_chars = string.chars().count();

	let start = if start < 0. {
		total_chars.saturating_sub(start.abs() as usize)
	} else {
		(start as usize).min(total_chars)
	};
	let end = if end <= 0. {
		total_chars.saturating_sub(end.abs() as usize)
	} else {
		(end as usize).min(total_chars)
	};

	if start >= end {
		return String::new();
	}

	string.chars().skip(start).take(end - start).collect()
}

// TODO: Return u32, u64, or usize instead of f64 after #1621 is resolved and has allowed us to implement automatic type conversion in the node graph for nodes with generic type inputs.
// TODO: (Currently automatic type conversion only works for concrete types, via the Graphene preprocessor and not the full Graphene type system.)
/// Counts the number of characters in a string.
#[node_macro::node(category("Text"))]
fn string_length(_: impl Ctx, string: String) -> f64 {
	string.chars().count() as f64
}

/// Splits a string into a list of substrings based on the specified delimeter.
/// For example, the delimeter "," will split "a,b,c" into the strings "a", "b", and "c".
#[node_macro::node(category("Text"))]
fn string_split(
	_: impl Ctx,
	/// The string to split into substrings.
	string: String,
	/// The character(s) that separate the substrings. These are not included in the outputs.
	#[default("\\n")]
	delimeter: String,
	/// Whether to convert escape sequences found in the delimeter into their corresponding characters:
	/// "\n" (newline), "\r" (carriage return), "\t" (tab), "\0" (null), and "\\" (backslash)
	#[default(true)]
	delimeter_escaping: bool,
) -> Vec<String> {
	let delimeter = if delimeter_escaping {
		delimeter.replace("\\n", "\n").replace("\\r", "\r").replace("\\t", "\t").replace("\\0", "\0").replace("\\\\", "\\")
	} else {
		delimeter
	};

	string.split(&delimeter).map(str::to_string).collect()
}

/// Evaluates either the "If True" or "If False" input branch based on whether the input condition is true or false.
#[node_macro::node(category("Math: Logic"))]
async fn switch<T, C: Send + 'n + Clone>(
	#[implementations(Context)] ctx: C,
	condition: bool,
	#[expose]
	#[implementations(
		Context -> String,
		Context -> bool,
		Context -> f32,
		Context -> f64,
		Context -> u32,
		Context -> u64,
		Context -> DVec2,
		Context -> DAffine2,
		Context -> Table<Artboard>,
		Context -> Table<Graphic>,
		Context -> Table<Vector>,
		Context -> Table<Raster<CPU>>,
		Context -> Table<Raster<GPU>>,
		Context -> Table<Color>,
		Context -> GradientStops,
	)]
	if_true: impl Node<C, Output = T>,
	#[expose]
	#[implementations(
		Context -> String,
		Context -> bool,
		Context -> f32,
		Context -> f64,
		Context -> u32,
		Context -> u64,
		Context -> DVec2,
		Context -> DAffine2,
		Context -> Table<Artboard>,
		Context -> Table<Graphic>,
		Context -> Table<Vector>,
		Context -> Table<Raster<CPU>>,
		Context -> Table<Raster<GPU>>,
		Context -> Table<Color>,
		Context -> GradientStops,
	)]
	if_false: impl Node<C, Output = T>,
) -> T {
	if condition { if_true.eval(ctx).await } else { if_false.eval(ctx).await }
}

mod font_cache;
mod path_builder;
mod text_context;
mod to_path;

use core_types::Color;
use core_types::Ctx;
use core_types::registry::types::TextArea;
use core_types::table::Table;
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};
use raster_types::{CPU, Raster};

// Re-export for convenience
pub use core_types as gcore;
pub use font_cache::*;
pub use text_context::TextContext;
pub use to_path::*;
pub use vector_types;

/// Alignment of lines of type within a text block.
#[repr(C)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, Hash, DynAny, node_macro::ChoiceType)]
#[widget(Radio)]
pub enum TextAlign {
	#[default]
	Left,
	Center,
	Right,
	#[label("Justify")]
	JustifyLeft,
	// TODO: JustifyCenter, JustifyRight, JustifyAll
}

impl From<TextAlign> for parley::Alignment {
	fn from(val: TextAlign) -> Self {
		match val {
			TextAlign::Left => parley::Alignment::Left,
			TextAlign::Center => parley::Alignment::Center,
			TextAlign::Right => parley::Alignment::Right,
			TextAlign::JustifyLeft => parley::Alignment::Justify,
		}
	}
}

#[derive(PartialEq, Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct TypesettingConfig {
	pub font_size: f64,
	pub line_height_ratio: f64,
	pub character_spacing: f64,
	pub max_width: Option<f64>,
	pub max_height: Option<f64>,
	pub tilt: f64,
	pub align: TextAlign,
}

impl Default for TypesettingConfig {
	fn default() -> Self {
		Self {
			font_size: 24.,
			line_height_ratio: 1.2,
			character_spacing: 0.,
			max_width: None,
			max_height: None,
			tilt: 0.,
			align: TextAlign::default(),
		}
	}
}

/// Constructs a string value which may be set to any plain text.
#[node_macro::node(category("Value"))]
fn string_value(_: impl Ctx, _primary: (), string: TextArea) -> String {
	string
}

/// Type-asserts a value to be a string.
#[node_macro::node(category("Debug"))]
fn to_string(_: impl Ctx, value: String) -> String {
	value
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
	/// "\n" (newline), "\r" (carriage return), "\t" (tab), "\0" (null), and "\\" (backslash).
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

/// Gets a value from either a json object or array given as a string input.
/// For example, for the input {"name": "ferris"} the key "name" will return "ferris".
#[node_macro::node(category("Text"))]
fn json_get(
	_: impl Ctx,
	/// The json data.
	data: String,
	/// The key to index the object with.
	key: String,
) -> String {
	use serde_json::Value;
	let Ok(value): Result<Value, _> = serde_json::from_str(&data) else {
		return "Input is not valid json".into();
	};
	match value {
		Value::Array(ref arr) => {
			let Ok(index): Result<usize, _> = key.parse() else {
				log::error!("Json input is an array, but key is not a number");
				return String::new();
			};
			let Some(value) = arr.get(index) else {
				log::error!("Index {} out of bounds for len {}", index, arr.len());
				return String::new();
			};
			value.to_string()
		}
		Value::Object(map) => {
			let Some(value) = map.get(&key) else {
				log::error!("Key {key} not found in object");
				return String::new();
			};
			match value {
				Value::String(s) => s.clone(),
				Value::Number(n) => n.to_string(),
				complex => complex.to_string(),
			}
		}
		_ => String::new(),
	}
}

/// Converts a value to a JSON string representation.
#[node_macro::node(category("Text"))]
fn serialize<T: serde::Serialize>(
	_: impl Ctx,
	#[implementations(String, bool, f64, u32, u64, DVec2, DAffine2, /* Table<Artboard>, Table<Graphic>, Table<Vector>, */ Table<Raster<CPU>>, Table<Color> /* , Table<GradientStops> */)] value: T,
) -> String {
	serde_json::to_string(&value).unwrap_or_else(|_| "Serialization Error".to_string())
}

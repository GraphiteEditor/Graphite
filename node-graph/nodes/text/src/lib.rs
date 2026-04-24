mod font_cache;
mod path_builder;
mod text_context;
mod to_path;

use convert_case::{Boundary, Converter, pattern};
use core_types::Color;
use core_types::Ctx;
use core_types::registry::types::{SignedInteger, TextArea};
use core_types::table::Table;
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};
use raster_types::{CPU, Raster};
use unicode_segmentation::UnicodeSegmentation;

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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, dyn_any::DynAny, node_macro::ChoiceType, serde::Serialize, serde::Deserialize)]
#[widget(Dropdown)]
pub enum StringCapitalization {
	/// "on the origin of species" — Converts all letters to lower case.
	#[default]
	#[label("lower case")]
	LowerCase,
	/// "ON THE ORIGIN OF SPECIES" — Converts all letters to upper case.
	#[label("UPPER CASE")]
	UpperCase,
	/// "On The Origin Of Species" — Converts the first letter of every word to upper case.
	#[label("Capital Case")]
	CapitalCase,
	/// "On the Origin of Species" — Converts the first letter of significant words to upper case.
	#[label("Headline Case")]
	HeadlineCase,
	/// "On the origin of species" — Converts the first letter of every word to lower case, except the initial word which is made upper case.
	#[label("Sentence case")]
	SentenceCase,
	/// "on The Origin Of Species" — Converts the first letter of every word to upper case, except the initial word which is made lower case.
	#[label("camel Case")]
	CamelCase,
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
	first + &second
}

/// Replaces all occurrences of "From" with "To" in the input string.
#[node_macro::node(category("Text"))]
fn string_replace(_: impl Ctx, string: String, from: TextArea, to: TextArea) -> String {
	string.replace(&from, &to)
}

/// Extracts a substring from the input string, starting at "Start" and ending before "End".
///
/// Negative indices count from the end of the string. If the index of "Start" equals or exceeds "End", the result is an empty string.
#[node_macro::node(category("Text"))]
fn string_slice(_: impl Ctx, string: String, start: SignedInteger, end: SignedInteger) -> String {
	let total_graphemes = string.graphemes(true).count();

	let start = if start < 0. {
		total_graphemes.saturating_sub(start.abs() as usize)
	} else {
		(start as usize).min(total_graphemes)
	};
	let end = if end <= 0. {
		total_graphemes.saturating_sub(end.abs() as usize)
	} else {
		(end as usize).min(total_graphemes)
	};

	if start >= end {
		return String::new();
	}

	string.graphemes(true).skip(start).take(end - start).collect()
}

/// Converts a string's capitalization style to another of the common upper and lower case patterns, optionally joining words with a chosen separator.
#[node_macro::node(category("Text"), properties("string_capitalization_properties"))]
fn string_capitalization(
	_: impl Ctx,
	/// The string to have its letter capitalization converted.
	string: String,
	/// The capitalization style to apply.
	capitalization: StringCapitalization,
	/// Whether to split the string into words and reconnect with the chosen joiner. When disabled, the existing word structure separators are preserved.
	use_joiner: bool,
	/// The string placed between each word.
	joiner: String,
) -> String {
	// When the joiner is enabled, apply word-level casing and optionally reconnect words with the selected joiner
	if use_joiner {
		match capitalization {
			// Simple case mappings that preserve the string's existing structure
			StringCapitalization::LowerCase => string.to_lowercase(),
			StringCapitalization::UpperCase => string.to_uppercase(),

			// Word-aware capitalizations that split on word boundaries and rejoin with the joiner
			StringCapitalization::CapitalCase => Converter::new().set_boundaries(&Boundary::defaults()).set_pattern(pattern::capital).set_delim(&joiner).convert(&string),
			StringCapitalization::HeadlineCase => {
				// First split into words with convert_case so word boundaries like "AlphaNumeric" are detected consistently with other modes,
				// then apply the titlecase crate for smart capitalization (lowercasing short words like "of", "the", etc.),
				// then rejoin with the custom joiner without mangling the capitalization
				let spaced = Converter::new().set_boundaries(&Boundary::defaults()).set_pattern(pattern::capital).set_delim(" ").convert(&string);
				let headline = titlecase::titlecase(&spaced);
				Converter::new().set_boundaries(&[Boundary::SPACE]).set_pattern(pattern::noop).set_delim(&joiner).convert(&headline)
			}
			StringCapitalization::SentenceCase => Converter::new()
				.set_boundaries(&Boundary::defaults())
				.set_pattern(pattern::sentence)
				.set_delim(&joiner)
				.convert(&string),
			StringCapitalization::CamelCase => Converter::new().set_boundaries(&Boundary::defaults()).set_pattern(pattern::camel).set_delim(&joiner).convert(&string),
		}
	}
	// When the joiner is disabled, apply only character-level casing while preserving the string's existing structure
	else {
		match capitalization {
			StringCapitalization::LowerCase => string.to_lowercase(),
			StringCapitalization::UpperCase => string.to_uppercase(),
			StringCapitalization::CapitalCase => {
				let mut capitalize_next = true;
				string.chars().fold(String::with_capacity(string.len()), |mut result, c| {
					if c.is_whitespace() || c == '_' || c == '-' {
						capitalize_next = true;
						result.push(c);
					} else if capitalize_next {
						capitalize_next = false;
						result.extend(c.to_uppercase());
					} else {
						result.push(c);
					}
					result
				})
			}
			StringCapitalization::HeadlineCase => titlecase::titlecase(&string),
			StringCapitalization::SentenceCase => {
				let mut chars = string.chars();
				match chars.next() {
					Some(first) => first.to_uppercase().to_string() + &chars.as_str().to_lowercase(),
					None => String::new(),
				}
			}
			StringCapitalization::CamelCase => {
				let mut capitalize_next = false;
				string.chars().fold(String::with_capacity(string.len()), |mut result, c| {
					if c.is_whitespace() || c == '_' || c == '-' {
						capitalize_next = true;
						result.push(c);
					} else if capitalize_next {
						capitalize_next = false;
						result.extend(c.to_uppercase());
					} else {
						result.extend(c.to_lowercase());
					}
					result
				})
			}
		}
	}
}

// TODO: Return u32, u64, or usize instead of f64 after #1621 is resolved and has allowed us to implement automatic type conversion in the node graph for nodes with generic type inputs.
// TODO: (Currently automatic type conversion only works for concrete types, via the Graphene preprocessor and not the full Graphene type system.)
/// Counts the number of characters in a string.
#[node_macro::node(category("Text"))]
fn string_length(_: impl Ctx, string: String) -> f64 {
	string.graphemes(true).count() as f64
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

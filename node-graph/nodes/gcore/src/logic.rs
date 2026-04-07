use convert_case::{Boundary, Converter, pattern};
use core_types::Color;
use core_types::registry::types::{SignedInteger, TextArea};
use core_types::table::Table;
use core_types::{CloneVarArgs, Context, Ctx, ExtractAll, ExtractVarArgs, OwnedContextImpl};
use glam::{DAffine2, DVec2};
use graphic_types::vector_types::GradientStops;
use graphic_types::{Artboard, Graphic, Vector};
use raster_types::{CPU, GPU, Raster};
use unicode_segmentation::UnicodeSegmentation;

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

/// Type-asserts a value to be a string.
#[node_macro::node(category("Debug"))]
fn to_string(_: impl Ctx, value: String) -> String {
	value
}

/// Converts a value to a JSON string representation.
#[node_macro::node(category("Text"))]
fn serialize<T: serde::Serialize>(
	_: impl Ctx,
	#[implementations(
		String,
		bool,
		f64,
		u32,
		u64,
		DVec2,
		DAffine2,
		// Table<Artboard>,
		// Table<Graphic>,
		// Table<Vector>,
		Table<Raster<CPU>>,
		Table<Color>,
		// Table<GradientStops>,
	)]
	value: T,
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
/// If the index of "Start" equals or exceeds "End", the result is an empty string.
#[node_macro::node(category("Text"))]
fn string_slice(_: impl Ctx, string: String, start: SignedInteger, end: SignedInteger) -> String {
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

/// Formats a number as a string with control over decimal places, decimal separator, and thousands grouping.
#[node_macro::node(category("Text"), properties("format_number_properties"))]
fn format_number(
	_: impl Ctx,
	number: f64,
	/// The number of digits after the decimal point. The value is rounded to fit. Set to 0 to show only whole numbers.
	#[default(2)]
	#[min(0)]
	decimal_places: u32,
	/// The character(s) used as the decimal point.
	#[default(".")]
	decimal_separator: String,
	/// Always show the exact number of decimal places, even if they are trailing zeros.
	#[default(true)]
	fixed_decimals: bool,
	/// Whether to group digits with a thousands separator.
	use_thousands_separator: bool,
	/// The character(s) inserted between digit groups.
	#[default(",")]
	thousands_separator: String,
	/// Don't group 4-digit numbers (only start grouping at 10,000 and above).
	#[name("Start at 10,000")]
	start_at_10000: bool,
) -> String {
	// Find the maximum meaningful decimal precision by detecting where float noise begins.
	// This works correctly whether the value originated as f32 or f64, since we find the
	// shortest decimal representation that round-trips back to the same f64 value.
	let requested_places = decimal_places as usize;
	let max_places = {
		let whole_digits = if number == 0. { 1 } else { (number.abs().log10().floor() as usize).saturating_add(1) };
		let upper_bound = 17_usize.saturating_sub(whole_digits);
		let mut meaningful = upper_bound;
		for p in 0..=upper_bound {
			let s = format!("{number:.p$}");
			if s.parse::<f64>() == Ok(number) {
				meaningful = p;
				break;
			}
		}
		meaningful
	};
	let places = requested_places.min(max_places);
	let formatted = format!("{number:.places$}");

	// If the user requested more decimal places than the float can represent, pad with zeros
	let extra_zeros = requested_places.saturating_sub(places);

	// Split into sign, whole, and decimal parts
	let (sign, unsigned) = if formatted.starts_with('-') { ("-", &formatted[1..]) } else { ("", formatted.as_str()) };

	let (whole_string, decimal_string) = match unsigned.split_once('.') {
		Some((w, d)) => {
			let padded = if extra_zeros > 0 { format!("{d}{:0>width$}", "", width = extra_zeros) } else { d.to_string() };
			(w.to_string(), Some(padded))
		}
		None => (unsigned.to_string(), None),
	};

	// Apply thousands grouping to the whole number part
	let grouped_whole = if use_thousands_separator && !thousands_separator.is_empty() {
		let skip = start_at_10000 && whole_string.len() <= 4;
		if skip {
			whole_string.clone()
		} else {
			let mut result = String::new();
			for (i, ch) in whole_string.chars().rev().enumerate() {
				if i > 0 && i % 3 == 0 {
					result.push_str(&thousands_separator.chars().rev().collect::<String>());
				}
				result.push(ch);
			}
			result.chars().rev().collect()
		}
	} else {
		whole_string
	};

	// Build the final string
	let Some(decimal_string) = decimal_string else {
		return format!("{sign}{grouped_whole}");
	};

	if fixed_decimals {
		format!("{sign}{grouped_whole}{decimal_separator}{decimal_string}")
	} else {
		let trimmed = decimal_string.trim_end_matches('0');
		if trimmed.is_empty() {
			format!("{sign}{grouped_whole}")
		} else {
			format!("{sign}{grouped_whole}{decimal_separator}{trimmed}")
		}
	}
}

/// Parses a string into a number. Returns the fallback value if the string is not a valid number.
#[node_macro::node(category("Text"))]
fn string_to_number(_: impl Ctx, string: String, fallback: f64) -> f64 {
	string.trim().parse::<f64>().unwrap_or(fallback)
}

/// Removes leading and/or trailing whitespace from a string.
#[node_macro::node(category("Text"))]
fn string_trim(_: impl Ctx, string: String, #[default(true)] start: bool, #[default(true)] end: bool) -> String {
	match (start, end) {
		(true, true) => string.trim().to_string(),
		(true, false) => string.trim_start().to_string(),
		(false, true) => string.trim_end().to_string(),
		(false, false) => string,
	}
}

/// Reverses the order of grapheme clusters (visual characters) in the string.
#[node_macro::node(category("Text"))]
fn string_reverse(_: impl Ctx, string: String) -> String {
	string.graphemes(true).rev().collect()
}

/// Repeats the string a given number of times, optionally with a separator between each repetition.
#[node_macro::node(category("Text"))]
fn string_repeat(
	_: impl Ctx,
	string: String,
	/// The number of times the string should appear in the output.
	#[default(2)]
	#[min(1)]
	count: u32,
	/// The string placed between each repetition.
	#[default("\\n")]
	separator: String,
	/// Whether to convert escape sequences found in the separator into their corresponding characters:
	/// "\n" (newline), "\r" (carriage return), "\t" (tab), "\0" (null), and "\\" (backslash).
	#[default(true)]
	separator_escaping: bool,
) -> String {
	let separator = if separator_escaping {
		separator.replace("\\n", "\n").replace("\\r", "\r").replace("\\t", "\t").replace("\\0", "\0").replace("\\\\", "\\")
	} else {
		separator
	};

	let count = count.max(1) as usize;

	let mut result = String::with_capacity((string.len() + separator.len()) * count);
	for i in 0..count {
		if i > 0 {
			result.push_str(&separator);
		}
		result.push_str(&string);
	}
	result
}

/// Pads the string to a target length by filling with the given string. If the string is already at or exceeds the target length, it is returned unchanged.
#[node_macro::node(category("Text"))]
fn string_pad(
	_: impl Ctx,
	string: String,
	/// The target character length after padding. When "Up To" is set, this applies to the portion before (or after) that substring.
	#[default(10)]
	length: u32,
	/// The string used to fill the remaining space. Repeats and trims to fit, if multi-character.
	#[default("#")]
	padding: String,
	/// Pad only the length of the string encountered before (or after) this substring, if given and present (otherwise the full string is considered).
	///
	/// For example, this can pad numbers with leading zeros to align them before the decimal point.
	up_to: String,
	/// Pad at the end of the string instead of the start.
	from_end: bool,
) -> String {
	let target_length = length as usize;

	if padding.is_empty() {
		return string;
	}

	// Split the string at the "up to" substring if provided, and only pad that portion
	if !up_to.is_empty()
		&& let Some(position) = if from_end { string.rfind(&*up_to) } else { string.find(&*up_to) }
	{
		let (before, after) = string.split_at(position);

		if from_end {
			// Pad the portion after the substring
			let after_substring = &after[up_to.len()..];
			let current_length = after_substring.chars().count();
			if current_length >= target_length {
				return string;
			}
			let pad_length = target_length - current_length;
			let padding: String = padding.chars().cycle().take(pad_length).collect();
			return format!("{before}{up_to}{after_substring}{padding}");
		} else {
			// Pad the portion before the substring
			let current_length = before.chars().count();
			if current_length >= target_length {
				return string;
			}
			let pad_length = target_length - current_length;
			let padding: String = padding.chars().cycle().take(pad_length).collect();
			return format!("{padding}{before}{after}");
		}
	}

	let current_length = string.chars().count();
	if current_length >= target_length {
		return string;
	}

	let pad_length = target_length - current_length;
	let padding: String = padding.chars().cycle().take(pad_length).collect();

	if from_end { string + &padding } else { padding + &string }
}

/// Checks whether the string contains the given substring. Optionally restricts the match to only the start and/or end of the string.
#[node_macro::node(category("Text"))]
fn string_contains(
	_: impl Ctx,
	string: String,
	substring: String,
	/// Only match if the substring appears at the start of the string.
	at_start: bool,
	/// Only match if the substring appears at the end of the string.
	at_end: bool,
) -> bool {
	match (at_start, at_end) {
		(true, true) => string.starts_with(&*substring) && string.ends_with(&*substring),
		(true, false) => string.starts_with(&*substring),
		(false, true) => string.ends_with(&*substring),
		(false, false) => string.contains(&*substring),
	}
}

/// Similar to the **String Contains** node, this finds the first (or last) occurrence of a substring within the string and returns its start index, or -1 if not found.
#[node_macro::node(category("Text"))]
fn string_find_index(
	_: impl Ctx,
	/// The string to search within.
	string: String,
	/// The substring to search for.
	substring: String,
	/// Find the start index of the last occurrence instead of the first.
	from_end: bool,
) -> f64 {
	if substring.is_empty() {
		return if from_end { string.chars().count() as f64 } else { 0. };
	}

	if from_end {
		// Search backwards by finding all byte-level matches and taking the last one
		string.rmatch_indices(&*substring).next().map_or(-1., |(byte_index, _)| string[..byte_index].chars().count() as f64)
	} else {
		string.match_indices(&*substring).next().map_or(-1., |(byte_index, _)| string[..byte_index].chars().count() as f64)
	}
}

/// Converts a string's capitalization style, optionally joining words with a specified separator.
#[node_macro::node(category("Text"), properties("string_capitalization_properties"))]
fn string_capitalization(
	_: impl Ctx,
	string: String,
	capitalization: StringCapitalization,
	/// Whether to split the string into words and rejoin with the specified joiner.
	/// When disabled, the existing separators and word structure are preserved.
	use_joiner: bool,
	/// The string placed between each word.
	joiner: String,
) -> String {
	// When the joiner is disabled, apply only character-level casing while preserving the string's existing structure
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
	} else {
		match capitalization {
			StringCapitalization::LowerCase => string.to_lowercase(),
			StringCapitalization::UpperCase => string.to_uppercase(),
			StringCapitalization::CapitalCase => {
				let mut capitalize_next = true;
				string
					.chars()
					.map(|c| {
						if c.is_whitespace() || c == '_' || c == '-' {
							capitalize_next = true;
							c
						} else if capitalize_next {
							capitalize_next = false;
							c.to_uppercase().next().unwrap_or(c)
						} else {
							c
						}
					})
					.collect()
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
				string
					.chars()
					.map(|c| {
						if c.is_whitespace() || c == '_' || c == '-' {
							capitalize_next = true;
							c
						} else if capitalize_next {
							capitalize_next = false;
							c.to_uppercase().next().unwrap_or(c)
						} else {
							c.to_lowercase().next().unwrap_or(c)
						}
					})
					.collect()
			}
		}
	}
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

/// Joins a list of strings together with a separator between each pair.
/// For example, joining ["a", "b", "c"] with separator ", " produces "a, b, c".
#[node_macro::node(category("Text"))]
fn string_join(
	_: impl Ctx,
	/// The list of strings to join together.
	strings: Vec<String>,
	/// The character(s) placed between each pair of strings.
	#[default(", ")]
	separator: String,
	/// Whether to convert escape sequences found in the separator into their corresponding characters:
	/// "\n" (newline), "\r" (carriage return), "\t" (tab), "\0" (null), and "\\" (backslash).
	#[default(true)]
	separator_escaping: bool,
) -> String {
	let separator = if separator_escaping {
		separator.replace("\\n", "\n").replace("\\r", "\r").replace("\\t", "\t").replace("\\0", "\0").replace("\\\\", "\\")
	} else {
		separator
	};

	strings.join(&separator)
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
		Context -> Table<GradientStops>,
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
		Context -> Table<GradientStops>,
	)]
	if_false: impl Node<C, Output = T>,
) -> T {
	if condition { if_true.eval(ctx).await } else { if_false.eval(ctx).await }
}

/// Tests whether a regular expression pattern matches within the string, returning true or false.
#[node_macro::node(category("Text"))]
fn regex_match(
	_: impl Ctx,
	/// The string to test against.
	string: String,
	/// The regular expression pattern to match.
	pattern: String,
	/// Require the pattern to match the entire string, not just any portion.
	entire_string: bool,
	/// Match letters regardless of case.
	case_insensitive: bool,
	/// Make `^` and `$` match the start and end of each line, not just the whole string.
	multiline: bool,
) -> bool {
	let flags = match (case_insensitive, multiline) {
		(false, false) => "",
		(true, false) => "(?i)",
		(false, true) => "(?m)",
		(true, true) => "(?im)",
	};
	let wrapped_pattern = if entire_string { format!("{flags}\\A(?:{pattern})\\z") } else { format!("{flags}{pattern}") };

	let Ok(regex) = fancy_regex::Regex::new(&wrapped_pattern) else {
		log::error!("Invalid regex pattern: {pattern}");
		return false;
	};

	regex.is_match(&string).unwrap_or(false)
}

/// Replaces matches of a regular expression pattern in the string. The replacement string supports backreferences: `$0` for the whole match, `$1`, `$2`, etc. for capture groups.
#[node_macro::node(category("Text"))]
fn regex_replace(
	_: impl Ctx,
	string: String,
	/// The regular expression pattern to search for.
	pattern: String,
	/// The replacement string. Use `$0` for the whole match, `$1`, `$2`, etc. for capture groups.
	replacement: String,
	/// Replace all matches. When disabled, only the first match is replaced.
	#[default(true)]
	replace_all: bool,
	/// Match letters regardless of case.
	case_insensitive: bool,
	/// Make `^` and `$` match the start and end of each line, not just the whole string.
	multiline: bool,
) -> String {
	let flags = match (case_insensitive, multiline) {
		(false, false) => "",
		(true, false) => "(?i)",
		(false, true) => "(?m)",
		(true, true) => "(?im)",
	};
	let full_pattern = format!("{flags}{pattern}");

	let Ok(regex) = fancy_regex::Regex::new(&full_pattern) else {
		log::warn!("Invalid regex pattern: {pattern}");
		return string;
	};

	if replace_all {
		regex.replace_all(&string, replacement.as_str()).into_owned()
	} else {
		regex.replace(&string, replacement.as_str()).into_owned()
	}
}

/// Iterates over a list of strings, evaluating the mapped operation for each one. Use the *Read String* node to access the current string inside the loop.
#[node_macro::node(category("Text"))]
async fn map_string(
	ctx: impl Ctx + CloneVarArgs + ExtractAll,
	strings: Vec<String>,
	#[expose]
	#[implementations(Context -> String)]
	mapped: impl Node<Context<'static>, Output = String>,
) -> Vec<String> {
	let mut result = Vec::new();

	for (i, string) in strings.into_iter().enumerate() {
		let owned_ctx = OwnedContextImpl::from(ctx.clone());
		let owned_ctx = owned_ctx.with_vararg(Box::new(string)).with_index(i);
		let mapped_strings = mapped.eval(owned_ctx.into_context()).await;

		result.push(mapped_strings);
	}

	result
}

/// Reads the current string from within a **Map String** node's loop.
#[node_macro::node(category("Context"))]
fn read_string(ctx: impl Ctx + ExtractVarArgs) -> String {
	let Ok(var_arg) = ctx.vararg(0) else { return String::new() };
	let var_arg = var_arg as &dyn std::any::Any;

	var_arg.downcast_ref::<String>().cloned().unwrap_or_default()
}

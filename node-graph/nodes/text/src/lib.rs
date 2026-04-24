mod font_cache;
pub mod json;
mod path_builder;
pub mod regex;
mod text_context;
mod to_path;

use convert_case::{Boundary, Converter, pattern};
use core_types::Color;
use core_types::registry::types::{SignedInteger, TextArea};
use core_types::table::Table;
use core_types::{CloneVarArgs, Context, Ctx, ExtractAll, ExtractVarArgs, OwnedContextImpl};
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

/// Converts escape sequence representations (`\n`, `\r`, `\t`, `\0`, `\\`) into their corresponding control characters.
/// Unrecognized escape sequences (e.g. `\x`) are preserved as-is.
fn unescape_string(input: String) -> String {
	let mut result = String::with_capacity(input.len());
	let mut chars = input.chars();

	while let Some(c) = chars.next() {
		if c == '\\' {
			match chars.next() {
				Some('n') => result.push('\n'),
				Some('r') => result.push('\r'),
				Some('t') => result.push('\t'),
				Some('0') => result.push('\0'),
				Some('\\') => result.push('\\'),
				Some(unrecognized) => result.extend(['\\', unrecognized]),
				None => result.push('\\'),
			}
		} else {
			result.push(c);
		}
	}

	result
}

/// Converts control characters (newline, carriage return, tab, null, backslash) back into their escape sequence representations.
fn escape_string(input: String) -> String {
	let mut result = String::with_capacity(input.len());

	for c in input.chars() {
		match c {
			'\n' => result.push_str("\\n"),
			'\r' => result.push_str("\\r"),
			'\t' => result.push_str("\\t"),
			'\0' => result.push_str("\\0"),
			'\\' => result.push_str("\\\\"),
			other => result.push(other),
		}
	}

	result
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

/// Clips the string to a maximum character length, optionally appending a suffix (like "…") when truncation occurs. Strings already within the limit are not modified.
#[node_macro::node(category("Text"))]
fn string_truncate(
	_: impl Ctx,
	/// The string to truncate.
	string: String,
	/// The maximum number of characters allowed, including the suffix if one is appended.
	#[default(80)]
	length: u32,
	/// A suffix appended to indicate truncation occurred, unless empty. Its length counts towards the character budget.
	#[default("…")]
	suffix: String,
) -> String {
	let max_length = length as usize;
	let grapheme_count = string.graphemes(true).count();

	if grapheme_count <= max_length {
		return string;
	}

	let suffix: String = suffix.graphemes(true).take(max_length).collect();
	let keep = max_length - suffix.graphemes(true).count();

	let mut truncated: String = string.graphemes(true).take(keep).collect();
	truncated.push_str(&suffix);
	truncated
}

/// Formats a number as a string with control over decimal places, decimal separator, and thousands grouping.
#[node_macro::node(category("Text"), properties("format_number_properties"))]
fn format_number(
	_: impl Ctx,
	/// The number to format as a string.
	number: f64,
	/// The amount of digits after the decimal point. The value is rounded to fit. Set to 0 to show only whole numbers.
	#[default(2)]
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
	/// Don't group 4-digit numbers with a thousands separator (only start grouping at 10,000 and above).
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
	let (sign, unsigned) = if let Some(rest) = formatted.strip_prefix('-') { ("-", rest) } else { ("", formatted.as_str()) };

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
		if fixed_decimals && requested_places > 0 {
			let zeros = "0".repeat(requested_places);
			return format!("{sign}{grouped_whole}{decimal_separator}{zeros}");
		}
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

/// Parses a string into a number. Falls back to the chosen value if the string is not a valid number.
#[node_macro::node(category("Text"))]
fn string_to_number(
	_: impl Ctx,
	/// The string containing a number. Surrounding whitespace is ignored, a decimal point (.) may be included, sign prefixes (+/-) are respected, and scientific notation (e.g. "1e-3") is supported.
	string: String,
	/// The value of the result if the string cannot be parsed as a valid number.
	fallback: f64,
) -> f64 {
	string.trim().parse::<f64>().unwrap_or(fallback)
}

/// Removes leading and/or trailing whitespace from a string. Common whitespace characters include spaces, tabs, and newlines.
#[node_macro::node(category("Text"))]
fn string_trim(
	_: impl Ctx,
	/// The string that may contain leading and trailing whitespace that should be removed.
	string: String,
	/// Whether the start of the string should have its whitespace removed.
	#[default(true)]
	start: bool,
	/// Whether the end of the string should have its whitespace removed.
	#[default(true)]
	end: bool,
) -> String {
	match (start, end) {
		(true, true) => string.trim().to_string(),
		(true, false) => string.trim_start().to_string(),
		(false, true) => string.trim_end().to_string(),
		(false, false) => string,
	}
}

/// Converts between literal escape sequences and their corresponding control characters within a string.
///
/// Unescape: `\n` (newline), `\r` (carriage return), `\t` (tab), `\0` (null), and `\\` (backslash) are converted into the actual special characters.
/// Escape: the actual special characters are converted back into their escape sequence representations.
#[node_macro::node(category("Text"))]
fn string_escape(
	_: impl Ctx,
	/// The string that contains either literal escape sequences or control characters to be converted to the opposite representation.
	string: String,
	/// Convert the control characters back into their escape sequence representations.
	#[default(true)]
	unescape: bool,
) -> String {
	if unescape { unescape_string(string) } else { escape_string(string) }
}

/// Reverses the sequence of characters making up the string so it reads back-to-front. ("Backwards text" becomes "txet sdrawkcaB".)
#[node_macro::node(category("Text"))]
fn string_reverse(
	_: impl Ctx,
	/// The string to be reversed.
	string: String,
) -> String {
	string.graphemes(true).rev().collect()
}

/// Repeats the string a given number of times, optionally with a separator between each repetition.
#[node_macro::node(category("Text"))]
fn string_repeat(
	_: impl Ctx,
	/// The string to be repeated.
	string: String,
	/// The number of times the string should appear in the output.
	#[default(2)]
	#[hard_min(1)]
	count: u32,
	/// The string placed between each repetition.
	#[default("\\n")]
	separator: String,
	/// Whether to convert escape sequences found in the separator into their corresponding characters:
	/// "\n" (newline), "\r" (carriage return), "\t" (tab), "\0" (null), and "\\" (backslash).
	#[default(true)]
	separator_escaping: bool,
) -> String {
	let separator = if separator_escaping { unescape_string(separator) } else { separator };

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

/// Pads the string to a target length by filling with the given repeated substring. If the string already meets or exceeds the target length, it is returned unchanged.
#[node_macro::node(category("Text"))]
fn string_pad(
	_: impl Ctx,
	/// The string to be padded to a target length.
	string: String,
	/// The target character length after padding. When "Up To" is set, this length concerns only the portion before (or after) that substring.
	#[default(10)]
	length: u32,
	/// The repeated substring used to fill the remaining space. A multi-charcter substring may end partway through its final repetition.
	#[default("#")]
	padding: String,
	/// Pad only the length of the string encountered before the start of the first (or after the end of the last) occurrence of this substring, if given and present (otherwise the full string is considered).
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
			let current_length = after_substring.graphemes(true).count();
			if current_length >= target_length {
				return string;
			}
			let pad_length = target_length - current_length;
			let padding: String = padding.graphemes(true).cycle().take(pad_length).collect();
			return format!("{before}{up_to}{after_substring}{padding}");
		} else {
			// Pad the portion before the substring
			let current_length = before.graphemes(true).count();
			if current_length >= target_length {
				return string;
			}
			let pad_length = target_length - current_length;
			let padding: String = padding.graphemes(true).cycle().take(pad_length).collect();
			return format!("{padding}{before}{after}");
		}
	}

	let current_length = string.graphemes(true).count();
	if current_length >= target_length {
		return string;
	}

	let pad_length = target_length - current_length;
	let padding: String = padding.graphemes(true).cycle().take(pad_length).collect();

	if from_end { string + &padding } else { padding + &string }
}

/// Checks whether the string contains the given substring. Optionally restricts the match to only the start and/or end of the string.
#[node_macro::node(category("Text"))]
fn string_contains(
	_: impl Ctx,
	/// The string to search within.
	string: String,
	/// The substring to search for.
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

/// Similar to the **String Contains** node, this searches within the input string for the first (or last) occurrence of a substring and returns the index of where that begins, or -1 if not found.
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
		return if from_end { string.graphemes(true).count() as f64 } else { 0. };
	}

	if from_end {
		// Search backwards by finding all byte-level matches and taking the last one
		string
			.rmatch_indices(&*substring)
			.next()
			.map_or(-1., |(byte_index, _)| string[..byte_index].graphemes(true).count() as f64)
	} else {
		string
			.match_indices(&*substring)
			.next()
			.map_or(-1., |(byte_index, _)| string[..byte_index].graphemes(true).count() as f64)
	}
}

/// Counts the number of occurrences of a substring within the string.
#[node_macro::node(category("Text"))]
fn string_occurrences(
	_: impl Ctx,
	/// The string to search within.
	string: String,
	/// The substring to count occurrences of.
	substring: String,
	/// Whether to count overlapping occurrences, using the substring as a sliding window.
	///
	/// For example, "aa" occurs twice in "aaaa" without overlapping but three times with overlapping.
	overlapping: bool,
) -> f64 {
	if substring.is_empty() {
		return 0.;
	}

	// NON-OVERLAPPING: Simple linear scan.
	// O(n), where n = string length
	if !overlapping {
		return string.matches(&*substring).count() as f64;
	}

	// OVERLAPPING: KMP (Knuth-Morris-Pratt) algorithm.
	// O(n + m), where n = string length, m = substring length

	let pattern: Vec<char> = substring.chars().collect();
	let text: Vec<char> = string.chars().collect();

	// Build the KMP failure function:
	// For each position in the pattern, the length of the longest proper prefix that is also a suffix.
	// This lets us skip ahead on mismatches instead of restarting from scratch.
	let mut failure = vec![0_usize; pattern.len()];
	let mut k = 0;
	for i in 1..pattern.len() {
		while k > 0 && pattern[k] != pattern[i] {
			k = failure[k - 1];
		}

		if pattern[k] == pattern[i] {
			k += 1;
		}

		failure[i] = k;
	}

	// Scan the text, advancing the pattern cursor without ever backtracking in the text
	let mut count: usize = 0;
	let mut pattern_cursor = 0;
	for &text_char in &text {
		while pattern_cursor > 0 && pattern[pattern_cursor] != text_char {
			pattern_cursor = failure[pattern_cursor - 1];
		}

		if pattern[pattern_cursor] == text_char {
			pattern_cursor += 1;
		}

		if pattern_cursor == pattern.len() {
			count += 1;

			// Reset using failure function to allow overlapping matches
			pattern_cursor = failure[pattern_cursor - 1];
		}
	}

	count as f64
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

/// Splits a string into a list of substrings based on the specified delimiter. This is the inverse of the **String Join** node.
///
/// For example, splitting "a, b, c" with delimiter ", " produces `["a", "b", "c"]`.
#[node_macro::node(category("Text"))]
fn string_split(
	_: impl Ctx,
	/// The string to split into substrings.
	string: String,
	/// The character(s) that separate the substrings. These are not included in the outputs.
	#[default("\\n")]
	delimiter: String,
	/// Whether to convert escape sequences found in the delimiter into their corresponding characters:
	/// "\n" (newline), "\r" (carriage return), "\t" (tab), "\0" (null), and "\\" (backslash).
	#[default(true)]
	delimiter_escaping: bool,
) -> Vec<String> {
	let delimiter = if delimiter_escaping { unescape_string(delimiter) } else { delimiter };

	string.split(&delimiter).map(str::to_string).collect()
}

/// Joins a list of strings together with a separator between each pair. This is the inverse of the **String Split** node.
///
/// For example, joining `["a", "b", "c"]` with separator ", " produces "a, b, c".
#[node_macro::node(category("Text"))]
fn string_join(
	_: impl Ctx,
	/// The list of strings to join together.
	strings: Vec<String>,
	/// The text placed between each pair of strings.
	#[default(", ")]
	separator: String,
	/// Whether to convert escape sequences found in the separator into their corresponding characters:
	/// "\n" (newline), "\r" (carriage return), "\t" (tab), "\0" (null), and "\\" (backslash).
	#[default(true)]
	separator_escaping: bool,
) -> String {
	let separator = if separator_escaping { unescape_string(separator) } else { separator };

	strings.join(&separator)
}

/// Iterates over a list of strings, evaluating the mapped operation for each one. Use the **Read String** node to access the current string inside the loop.
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

/// Converts a value to a JSON string representation.
#[node_macro::node(category("Debug"))]
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

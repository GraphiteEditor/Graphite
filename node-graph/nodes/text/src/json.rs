use core_types::Ctx;
use serde_json::Value;

use crate::unescape_string;

// ===========
// Format JSON
// ===========

/// Reformats a JSON string with control over indentation, line breaking, and spacing. Trailing commas are tolerated. Otherwise-invalid JSON input is returned unchanged.
#[node_macro::node(name("Format JSON"), category("Text: JSON"))]
fn format_json(
	_: impl Ctx,
	/// The JSON string to reformat.
	#[name("JSON")]
	json: String,
	/// Removes optional spaces within curly brackets and after colons and commas.
	compact: bool,
	/// Break arrays and objects across multiple lines when they exceed the line break length.
	#[default(true)]
	#[name("Multi-Line")]
	multi_line: bool,
	/// The indentation string used for each nesting level. Escape sequences like `\t` (the tab character) are supported. Two or four spaces are also common choices.
	#[default("\\t")]
	indent: String,
	/// The maximum line length before a container (array or object) is broken across lines. Set this to 0 to always break containers. (Requires *Multi-Line* to take effect.)
	///
	/// This is not a maximum line length guarantee. Deep nesting and long keys or values may exceed this length.
	#[default(120)]
	break_length: u32,
	/// Always break a container (array or object) across lines if it holds another container, even if it would fit within the break length. (Requires *Multi-Line* to take effect.)
	#[default(true)]
	break_nested: bool,
) -> String {
	let cleaned = strip_trailing_commas(&json);
	let Ok(value) = serde_json::from_str::<serde_json::Value>(&cleaned) else { return json };
	let indent = unescape_string(indent);
	let colon = if compact { ":" } else { ": " };
	let comma_space = if compact { "," } else { ", " };
	let line_width = break_length as usize;

	if multi_line {
		format_value(&value, 0, &indent, colon, comma_space, compact, break_nested, line_width)
	} else {
		format_inline(&value, colon, comma_space, compact)
	}
}

/// Strips trailing commas before `]` and `}` to accept JSON-with-trailing-commas input.
/// Respects string literals so commas inside strings are left untouched.
fn strip_trailing_commas(json: &str) -> String {
	let mut output = String::with_capacity(json.len());
	let mut chars = json.chars().peekable();
	let mut in_string = false;

	while let Some(c) = chars.next() {
		if in_string {
			output.push(c);

			// Skip escaped characters inside strings
			if c == '\\'
				&& let Some(escaped) = chars.next()
			{
				output.push(escaped);
			} else if c == '"' {
				in_string = false;
			}

			continue;
		}

		match c {
			'"' => {
				in_string = true;
				output.push(c);
			}
			',' => {
				// Skip any whitespace after the comma
				while chars.peek().is_some_and(|c| c.is_ascii_whitespace()) {
					chars.next();
				}

				// Drop trailing commas (before `]` or `}`), but keep all others
				if !chars.peek().is_some_and(|&c| c == ']' || c == '}') {
					output.push(',');
				}
			}
			_ => output.push(c),
		}
	}

	output
}

/// Formats a JSON value as a single unbroken line.
fn format_inline(value: &serde_json::Value, colon: &str, comma_space: &str, compact: bool) -> String {
	match value {
		serde_json::Value::Array(arr) => {
			let inner: Vec<String> = arr.iter().map(|v| format_inline(v, colon, comma_space, compact)).collect();
			format!("[{}]", inner.join(comma_space))
		}
		serde_json::Value::Object(obj) => {
			let inner: Vec<String> = obj
				.iter()
				.map(|(k, v)| format!("{}{}{}", serde_json::to_string(k).unwrap_or_default(), colon, format_inline(v, colon, comma_space, compact)))
				.collect();
			let joined = inner.join(comma_space);
			if compact || joined.is_empty() { format!("{{{joined}}}") } else { format!("{{ {joined} }}") }
		}
		other => serde_json::to_string(other).unwrap_or_default(),
	}
}

/// Formats a JSON value, optionally breaking containers across lines when they contain other containers or exceed the line break length limit.
#[allow(clippy::too_many_arguments)]
fn format_value(value: &serde_json::Value, depth: usize, indent: &str, colon: &str, comma_space: &str, compact: bool, break_nested: bool, line_width: usize) -> String {
	// Checks whether any direct child of a container is itself a container.
	let contains_containers = |value: &serde_json::Value| {
		// Checks whether a JSON value is a container (array or object).
		let is_container = |value: &serde_json::Value| matches!(value, serde_json::Value::Array(_) | serde_json::Value::Object(_));

		match value {
			serde_json::Value::Array(arr) => arr.iter().any(is_container),
			serde_json::Value::Object(obj) => obj.values().any(is_container),
			_ => false,
		}
	};

	match value {
		serde_json::Value::Array(arr) if !arr.is_empty() => {
			// Try inline if children are all leaves (or break_nested is off) and it fits
			if !break_nested || !contains_containers(value) {
				let inline = format_inline(value, colon, comma_space, compact);
				let current_indent_width = indent.len() * depth;
				if current_indent_width + inline.len() <= line_width {
					return inline;
				}
			}

			// Break across lines
			let child_indent = indent.repeat(depth + 1);
			let closing_indent = indent.repeat(depth);
			let items: Vec<String> = arr
				.iter()
				.map(|v| format!("{child_indent}{}", format_value(v, depth + 1, indent, colon, comma_space, compact, break_nested, line_width)))
				.collect();
			format!("[\n{}\n{closing_indent}]", items.join(",\n"))
		}
		serde_json::Value::Object(obj) if !obj.is_empty() => {
			// Try inline if children are all leaves (or break_nested is off) and it fits
			if !break_nested || !contains_containers(value) {
				let inline = format_inline(value, colon, comma_space, compact);
				let current_indent_width = indent.len() * depth;
				if current_indent_width + inline.len() <= line_width {
					return inline;
				}
			}

			// Break across lines
			let child_indent = indent.repeat(depth + 1);
			let closing_indent = indent.repeat(depth);
			let entries: Vec<String> = obj
				.iter()
				.map(|(k, v)| {
					let key = serde_json::to_string(k).unwrap_or_default();
					let val = format_value(v, depth + 1, indent, colon, comma_space, compact, break_nested, line_width);
					format!("{child_indent}{key}{colon}{val}")
				})
				.collect();
			format!("{{\n{}\n{closing_indent}}}", entries.join(",\n"))
		}
		other => serde_json::to_string(other).unwrap_or_default(),
	}
}

// ================
// Query JSON (All)
// ================

/// Extracts a single matched value from a JSON string using a path expression (see that parameter's description for its syntax). If no matches are found, an empty string is returned. If multiple values are matched, the first is returned. To read all matches, use the **Query JSON All** node.
///
/// This is useful in conjunction with the nodes:
/// • **String to Number**: convert numeric query results to numbers.
/// • **String Value** → **Equals**: convert "true", "false", or "null" query results to bools.
#[node_macro::node(name("Query JSON"), category("Text: JSON"))]
fn query_json(
	_: impl Ctx,
	/// The JSON string to extract a value from.
	#[name("JSON")]
	json: String,
	/// Determines which contained value to extract from within the JSON.
	///
	/// The path syntax is like JavaScript's accessor syntax that follows an array/object value. It also supports negative indexing to count backwards from the end. Additionally, `[]` accesses all array and object values instead of just one.
	///
	/// Examples:
	/// Use `[2]` or `[-1]` to get the last value, and `[1]` or `[-2]` for the middle value, of `["a", "b", "c"]`.
	/// Use `.size` or `["size"]` to get the `size` property of `{ "size": 10 }`. The latter form is required if the key contains spaces or special characters like `["this key with spaces!"]`.
	/// Use chained accessors like `.fonts[0].name` to query deeper.
	/// Use the `[]` accessor to query all elements, like `.fonts[].weights[]` to get every weight of every font.
	path: String,
	/// Strips the surrounding double quotes from string values, returning the raw text. Other types are never wrapped in quotes.
	#[default(true)]
	unquote_strings: bool,
) -> String {
	let cleaned = strip_trailing_commas(&json);
	let Ok(value): Result<Value, _> = serde_json::from_str(&cleaned) else { return String::new() };
	let Some(segments) = parse_json_path(path.trim()) else { return String::new() };

	let mut results = Vec::new();
	resolve_all(&value, &segments, !unquote_strings, &mut results);

	results.into_iter().next().unwrap_or_default()
}

/// Extracts every matched value from a JSON string using a path expression (see that parameter's description for its syntax). A list of zero or more resultant strings is produced. The `[]` path accessor is used to read more than one value.
///
/// This is useful in conjunction with the nodes:
/// • **Index Elements**: access the `N`th query result.
/// • **String to Number**: convert numeric query results to numbers.
/// • **String Value** → **Equals**: convert "true", "false", or "null" query results to bools.
#[node_macro::node(name("Query JSON All"), category("Text: JSON"))]
fn query_json_all(
	_: impl Ctx,
	/// The JSON string to extract values from.
	#[name("JSON")]
	json: String,
	/// Determines which contained values to extract from within the JSON.
	///
	/// The path syntax is like JavaScript's accessor syntax that follows an array/object value. It also supports negative indexing to count backwards from the end. Additionally, `[]` accesses all array and object values instead of just one.
	///
	/// Examples:
	/// Use `[2]` or `[-1]` to get the last value, and `[1]` or `[-2]` for the middle value, of `["a", "b", "c"]`.
	/// Use `.size` or `["size"]` to get the `size` property of `{ "size": 10 }`. The latter form is required if the key contains spaces or special characters like `["this key with spaces!"]`.
	/// Use chained accessors like `.fonts[0].name` to query deeper.
	/// Use the `[]` accessor to query all elements, like `.fonts[].weights[]` to get every weight of every font.
	path: String,
	/// Strips the surrounding double quotes from string values, returning the raw text. Other types are never wrapped in quotes.
	#[default(true)]
	unquote_strings: bool,
) -> Vec<String> {
	let cleaned = strip_trailing_commas(&json);
	let Ok(value): Result<Value, _> = serde_json::from_str(&cleaned) else { return Vec::new() };
	let Some(segments) = parse_json_path(path.trim()) else { return Vec::new() };

	let mut results = Vec::new();
	resolve_all(&value, &segments, !unquote_strings, &mut results);

	results
}

/// A parsed segment of a JSON access path.
enum JsonPathSegment {
	/// Access an object key, e.g. `.name` or `["my key"]`.
	Key(String),
	/// Access an array element by index, e.g. `[0]` or `[-1]`.
	Index(i32),
	/// Iterate all elements of an array or object values, e.g. `[]`.
	IterateAll,
}

/// Parses a JSON access path like `users[0].name` or `.["my key"][].value` into segments.
/// Returns `None` on syntax errors.
fn parse_json_path(path: &str) -> Option<Vec<JsonPathSegment>> {
	let mut segments = Vec::new();
	let mut chars = path.chars().peekable();

	// Skip optional leading dot
	if chars.peek() == Some(&'.') {
		chars.next();

		if chars.peek() == Some(&'.') {
			return None;
		}
	}

	while chars.peek().is_some() {
		if chars.peek() == Some(&'[') {
			chars.next(); // consume '['

			if chars.peek() == Some(&']') {
				// Empty brackets: iterate all
				chars.next();
				segments.push(JsonPathSegment::IterateAll);
			} else if matches!(chars.peek(), Some(&'"') | Some(&'\'')) {
				// Quoted key: ["my key"] or ['my key']
				let closing_quote = chars.next().unwrap(); // consume opening quote
				let mut key = String::new();
				while let Some(&c) = chars.peek() {
					if c == closing_quote {
						chars.next(); // consume closing quote
						break;
					}
					if c == '\\' {
						chars.next();
						match chars.next() {
							Some('"') => key.push('"'),
							Some('\'') => key.push('\''),
							Some('\\') => key.push('\\'),
							Some('/') => key.push('/'),
							Some('b') => key.push('\x08'),
							Some('f') => key.push('\x0C'),
							Some('n') => key.push('\n'),
							Some('r') => key.push('\r'),
							Some('t') => key.push('\t'),
							Some('u') => {
								// Decode a 4-hex-digit Unicode escape sequence, only consuming verified hex digits
								let mut hex_digits = [0_u8; 4];
								let mut count = 0;
								for digit in &mut hex_digits {
									match chars.peek() {
										Some(c) if c.is_ascii_hexdigit() => {
											*digit = chars.next().unwrap() as u8;
											count += 1;
										}
										_ => break,
									}
								}

								let hex = &hex_digits[..count];
								if count == 4
									&& let Ok(hex_str) = core::str::from_utf8(hex)
									&& let Ok(code_point) = u32::from_str_radix(hex_str, 16)
									&& let Some(byte) = char::from_u32(code_point)
								{
									key.push(byte);
								} else {
									key.push('\\');
									key.push('u');
									for &byte in hex {
										key.push(byte as char);
									}
								}
							}
							Some(other) => {
								key.push('\\');
								key.push(other);
							}
							None => key.push('\\'),
						}
					} else {
						key.push(c);
						chars.next();
					}
				}
				// Require the closing ']'
				if chars.peek() == Some(&']') {
					chars.next();
				} else {
					return None;
				}
				segments.push(JsonPathSegment::Key(key));
			} else {
				// Numeric index: [0] or [-1]
				let mut num_str = String::new();
				while let Some(&c) = chars.peek() {
					if c == ']' {
						chars.next();
						break;
					}
					num_str.push(c);
					chars.next();
				}
				if let Ok(index) = num_str.trim().parse::<i32>() {
					segments.push(JsonPathSegment::Index(index));
				} else {
					return None;
				}
			}
		} else if chars.peek() == Some(&'.') {
			// Dot separator before next key
			chars.next();

			if chars.peek() == Some(&'.') || chars.peek().is_none() {
				return None;
			}
		} else {
			// Bare key: read until dot or bracket
			let mut key = String::new();
			while let Some(&c) = chars.peek() {
				if c == '.' || c == '[' {
					break;
				}
				key.push(c);
				chars.next();
			}
			if !key.is_empty() {
				segments.push(JsonPathSegment::Key(key));
			}
		}
	}

	Some(segments)
}

/// Converts a JSON value to its string representation.
/// Strings are quoted by default to produce valid JSON syntax. When `quote_strings` is false, surrounding quotes are stripped.
fn json_value_to_string(value: &serde_json::Value, quote_strings: bool) -> String {
	match value {
		serde_json::Value::String(s) if !quote_strings => s.clone(),
		other => other.to_string(),
	}
}

/// Navigates a JSON value by one path segment, returning the resulting value (or `None` if the path is invalid).
fn json_navigate<'a>(value: &'a serde_json::Value, segment: &JsonPathSegment) -> Option<&'a serde_json::Value> {
	match segment {
		JsonPathSegment::Key(key) => value.as_object().and_then(|obj| obj.get(key)),
		JsonPathSegment::Index(index) => {
			let arr = value.as_array()?;
			let resolved = if *index < 0 { arr.len().checked_sub(index.unsigned_abs() as usize)? } else { *index as usize };
			arr.get(resolved)
		}
		JsonPathSegment::IterateAll => None, // Handled by resolve_all
	}
}

/// Recursively resolves a path against a JSON value, fanning out at each `[]` and collecting leaf results.
fn resolve_all(value: &serde_json::Value, segments: &[JsonPathSegment], quote_strings: bool, results: &mut Vec<String>) {
	// Find the next IterateAll in the remaining segments
	let Some(iterate_position) = segments.iter().position(|s| matches!(s, JsonPathSegment::IterateAll)) else {
		// No more [] segments, navigate the rest linearly
		let mut current = value;
		for segment in segments {
			let Some(next) = json_navigate(current, segment) else { return };
			current = next;
		}
		results.push(json_value_to_string(current, quote_strings));
		return;
	};

	// Navigate to the array/object before the []
	let mut current = value;
	for segment in &segments[..iterate_position] {
		let Some(next) = json_navigate(current, segment) else { return };
		current = next;
	}

	// Fan out over elements and recurse with the remaining path
	let remaining = &segments[iterate_position + 1..];
	match current {
		serde_json::Value::Array(arr) => {
			for element in arr {
				resolve_all(element, remaining, quote_strings, results);
			}
		}
		serde_json::Value::Object(obj) => {
			for element in obj.values() {
				resolve_all(element, remaining, quote_strings, results);
			}
		}
		_ => {}
	}
}

use core_types::list::{Item, List};
use core_types::registry::types::SignedInteger;
use core_types::{ATTR_END, ATTR_NAME, ATTR_START, Ctx};

/// Checks whether the string contains a match for the given regular expression pattern. Optionally restricts the match to only the start and/or end of the string.
#[node_macro::node(category("Text: Regex"))]
fn regex_contains(
	_: impl Ctx,
	/// The string to search within.
	string: String,
	/// The regular expression pattern to search for.
	pattern: String,
	/// Match letters regardless of case.
	case_insensitive: bool,
	/// Make `^` and `$` match the start and end of each line, not just the whole string.
	multiline: bool,
	/// Only match if the pattern appears at the start of the string.
	at_start: bool,
	/// Only match if the pattern appears at the end of the string.
	at_end: bool,
) -> bool {
	let flags = match (case_insensitive, multiline) {
		(false, false) => "",
		(true, false) => "(?i)",
		(false, true) => "(?m)",
		(true, true) => "(?im)",
	};
	let anchored_pattern = match (at_start, at_end) {
		(true, true) => format!("{flags}\\A(?:{pattern})\\z"),
		(true, false) => format!("{flags}\\A(?:{pattern})"),
		(false, true) => format!("{flags}(?:{pattern})\\z"),
		(false, false) => format!("{flags}{pattern}"),
	};

	let Ok(regex) = fancy_regex::Regex::new(&anchored_pattern) else {
		log::error!("Invalid regex pattern: {pattern}");
		return false;
	};

	regex.is_match(&string).unwrap_or(false)
}

/// Replaces matches of a regular expression pattern in the string. The replacement string can reference captures: `$0` for the whole match and `$1`, `$2`, etc. for capture groups.
#[node_macro::node(category("Text: Regex"))]
fn regex_replace(
	_: impl Ctx,
	string: Item<String>,
	/// The regular expression pattern to search for.
	pattern: Item<String>,
	/// The replacement string. Use `$0` for the whole match and `$1`, `$2`, etc. for capture groups.
	replacement: Item<String>,
	/// Replace all matches. When disabled, only the first match is replaced.
	#[default(true)]
	replace_all: Item<bool>,
	/// Match letters regardless of case.
	case_insensitive: Item<bool>,
	/// Make `^` and `$` match the start and end of each line, not just the whole string.
	multiline: Item<bool>,
) -> Item<String> {
	let mut string = string;
	let pattern = pattern.element().clone();
	let replacement = replacement.element().clone();
	let (replace_all, case_insensitive, multiline) = (*replace_all.element(), *case_insensitive.element(), *multiline.element());

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

	let result = if replace_all {
		regex.replace_all(string.element(), replacement.as_str()).into_owned()
	} else {
		regex.replace(string.element(), replacement.as_str()).into_owned()
	};

	*string.element_mut() = result;
	string
}

/// Finds a regex match in the string and returns its components. The result is a list where the first item is the whole match (`$0`) and subsequent items are the capture groups (`$1`, `$2`, etc., if any).
///
/// The match index selects which non-overlapping occurrence to return (0 for the first match). Returns an empty list if no match is found at the given index.
///
/// Each item carries `start` and `end` byte-offset attributes pointing into the original string, plus a `name` attribute holding
/// the capture group's name (empty for unnamed groups, and for index 0 which is the whole match).
#[node_macro::node(category(""))]
fn regex_find(
	_: impl Ctx,
	/// The string to search within.
	string: Item<String>,
	/// The regular expression pattern to search for.
	pattern: Item<String>,
	/// Which non-overlapping occurrence of the pattern to return, starting from 0 for the first match. Negative indices count backwards from the last match.
	match_index: Item<SignedInteger>,
	/// Match letters regardless of case.
	case_insensitive: Item<bool>,
	/// Make `^` and `$` match the start and end of each line, not just the whole string.
	multiline: Item<bool>,
) -> List<String> {
	let string = string.element();
	let pattern = pattern.element();
	let (match_index, case_insensitive, multiline) = (*match_index.element(), *case_insensitive.element(), *multiline.element());

	if pattern.is_empty() {
		return List::new();
	}

	let flags = match (case_insensitive, multiline) {
		(false, false) => "",
		(true, false) => "(?i)",
		(false, true) => "(?m)",
		(true, true) => "(?im)",
	};
	let full_pattern = format!("{flags}{pattern}");

	let Ok(regex) = fancy_regex::Regex::new(&full_pattern) else {
		log::error!("Invalid regex pattern: {pattern}");
		return List::new();
	};

	// Capture group names indexed positionally; index 0 (the whole match) is always None.
	let capture_names: Vec<Option<String>> = regex.capture_names().map(|name| name.map(str::to_string)).collect();

	// Collect all matches since we need to support negative indexing
	let matches: Vec<_> = regex.captures_iter(string).filter_map(|c| c.ok()).collect();

	let match_index = match_index as i32;
	let resolved_index = if match_index < 0 {
		let from_end = (-match_index) as usize;
		if from_end > matches.len() {
			return List::new();
		}
		matches.len() - from_end
	} else {
		match_index as usize
	};

	let Some(captures) = matches.get(resolved_index) else {
		return List::new();
	};

	// Index 0 is the whole match, 1+ are capture groups
	(0..captures.len())
		.map(|i| {
			let captured = captures.get(i);
			let text = captured.map_or(String::new(), |m| m.as_str().to_string());
			let start = captured.map_or(0_u64, |m| m.start() as u64);
			let end = captured.map_or(0_u64, |m| m.end() as u64);
			let name = capture_names.get(i).cloned().flatten().unwrap_or_default();
			Item::new_from_element(text)
				.with_attribute(ATTR_START, start)
				.with_attribute(ATTR_END, end)
				.with_attribute(ATTR_NAME, name)
		})
		.collect()
}

/// Finds all non-overlapping matches of a regular expression pattern in the string, returning a list of the matched substrings.
///
/// Each item carries `start` and `end` byte-offset attributes pointing into the original string.
#[node_macro::node(category("Text: Regex"))]
fn regex_find_all(
	_: impl Ctx,
	/// The string to search within.
	string: Item<String>,
	/// The regular expression pattern to search for.
	pattern: Item<String>,
	/// Match letters regardless of case.
	case_insensitive: Item<bool>,
	/// Make `^` and `$` match the start and end of each line, not just the whole string.
	multiline: Item<bool>,
) -> List<String> {
	let string = string.element();
	let pattern = pattern.element();
	let (case_insensitive, multiline) = (*case_insensitive.element(), *multiline.element());

	if pattern.is_empty() {
		return List::new();
	}

	let flags = match (case_insensitive, multiline) {
		(false, false) => "",
		(true, false) => "(?i)",
		(false, true) => "(?m)",
		(true, true) => "(?im)",
	};
	let full_pattern = format!("{flags}{pattern}");

	let Ok(regex) = fancy_regex::Regex::new(&full_pattern) else {
		log::error!("Invalid regex pattern: {pattern}");
		return List::new();
	};

	regex
		.find_iter(string)
		.filter_map(|m| m.ok())
		.map(|m| {
			Item::new_from_element(m.as_str().to_string())
				.with_attribute(ATTR_START, m.start() as u64)
				.with_attribute(ATTR_END, m.end() as u64)
		})
		.collect()
}

/// Splits a string into a list of substrings pulled from between separator characters as matched by a regular expression.
///
/// For example, splitting "Three, two, one... LIFTOFF" with pattern `\W+` (non-word characters) produces `["Three", "two", "one", "LIFTOFF"]`.
#[node_macro::node(category("Text: Regex"))]
fn regex_split(
	_: impl Ctx,
	/// The string to split into substrings.
	string: Item<String>,
	/// The regular expression pattern to split on. Matches are consumed and not included in the output.
	pattern: Item<String>,
	/// Match letters regardless of case.
	case_insensitive: Item<bool>,
	/// Make `^` and `$` match the start and end of each line, not just the whole string.
	multiline: Item<bool>,
) -> List<String> {
	let pattern = pattern.element().clone();
	let (case_insensitive, multiline) = (*case_insensitive.element(), *multiline.element());

	if pattern.is_empty() {
		return List::new_from_item(string);
	}

	let flags = match (case_insensitive, multiline) {
		(false, false) => "",
		(true, false) => "(?i)",
		(false, true) => "(?m)",
		(true, true) => "(?im)",
	};
	let full_pattern = format!("{flags}{pattern}");

	let Ok(regex) = fancy_regex::Regex::new(&full_pattern) else {
		log::error!("Invalid regex pattern: {pattern}");
		return List::new_from_item(string);
	};

	regex.split(string.element()).filter_map(|s| s.ok()).map(|s| s.to_string()).map(Item::new_from_element).collect()
}

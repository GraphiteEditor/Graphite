use core_types::Ctx;
use core_types::registry::types::SignedInteger;

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
	string: String,
	/// The regular expression pattern to search for.
	pattern: String,
	/// The replacement string. Use `$0` for the whole match and `$1`, `$2`, etc. for capture groups.
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

/// Finds a regex match in the string and returns its components. The result is a list where the first element is the whole match (`$0`) and subsequent elements are the capture groups (`$1`, `$2`, etc., if any).
///
/// The match index selects which non-overlapping occurrence to return (0 for the first match). Returns an empty list if no match is found at the given index.
#[node_macro::node(category(""))]
fn regex_find(
	_: impl Ctx,
	/// The string to search within.
	string: String,
	/// The regular expression pattern to search for.
	pattern: String,
	/// Which non-overlapping occurrence of the pattern to return, starting from 0 for the first match. Negative indices count backwards from the last match.
	match_index: SignedInteger,
	/// Match letters regardless of case.
	case_insensitive: bool,
	/// Make `^` and `$` match the start and end of each line, not just the whole string.
	multiline: bool,
) -> Vec<String> {
	if pattern.is_empty() {
		return Vec::new();
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
		return Vec::new();
	};

	// Collect all matches since we need to support negative indexing
	let matches: Vec<_> = regex.captures_iter(&string).filter_map(|c| c.ok()).collect();

	let match_index = match_index as i32;
	let resolved_index = if match_index < 0 {
		let from_end = (-match_index) as usize;
		if from_end > matches.len() {
			return Vec::new();
		}
		matches.len() - from_end
	} else {
		match_index as usize
	};

	let Some(captures) = matches.get(resolved_index) else {
		return Vec::new();
	};

	// Index 0 is the whole match, 1+ are capture groups
	(0..captures.len()).map(|i| captures.get(i).map_or(String::new(), |m| m.as_str().to_string())).collect()
}

/// Finds all non-overlapping matches of a regular expression pattern in the string, returning a list of the matched substrings.
#[node_macro::node(category("Text: Regex"))]
fn regex_find_all(
	_: impl Ctx,
	/// The string to search within.
	string: String,
	/// The regular expression pattern to search for.
	pattern: String,
	/// Match letters regardless of case.
	case_insensitive: bool,
	/// Make `^` and `$` match the start and end of each line, not just the whole string.
	multiline: bool,
) -> Vec<String> {
	if pattern.is_empty() {
		return Vec::new();
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
		return Vec::new();
	};

	regex.find_iter(&string).filter_map(|m| m.ok()).map(|m| m.as_str().to_string()).collect()
}

/// Splits a string into a list of substrings pulled from between separator characters as matched by a regular expression.
///
/// For example, splitting "Three, two, one... LIFTOFF" with pattern `\W+` (non-word characters) produces `["Three", "two", "one", "LIFTOFF"]`.
#[node_macro::node(category("Text: Regex"))]
fn regex_split(
	_: impl Ctx,
	/// The string to split into substrings.
	string: String,
	/// The regular expression pattern to split on. Matches are consumed and not included in the output.
	pattern: String,
	/// Match letters regardless of case.
	case_insensitive: bool,
	/// Make `^` and `$` match the start and end of each line, not just the whole string.
	multiline: bool,
) -> Vec<String> {
	if pattern.is_empty() {
		return vec![string];
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
		return vec![string];
	};

	regex.split(&string).filter_map(|s| s.ok()).map(|s| s.to_string()).collect()
}

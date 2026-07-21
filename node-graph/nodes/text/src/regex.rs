use crate::{expanded_count, locate_expanded};
use core_types::attribute::{Attr, End, Name, Start};
use core_types::extent::{LevelIn, ListIn, ValueIn};
use core_types::gpoll::{Extent, GPoll, GraphError, Interrupt};
use core_types::node::Lane;
use core_types::registry::types::SignedInteger;
use core_types::{Ctx, ExtractIndex, InjectIndex};

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

/// The pattern with its flag prefix compiled, or nothing for an empty or
/// invalid pattern (the latter logged).
fn compile_regex(pattern: &str, case_insensitive: bool, multiline: bool) -> Option<fancy_regex::Regex> {
	if pattern.is_empty() {
		return None;
	}
	let flags = match (case_insensitive, multiline) {
		(false, false) => "",
		(true, false) => "(?i)",
		(false, true) => "(?m)",
		(true, true) => "(?im)",
	};
	match fancy_regex::Regex::new(&format!("{flags}{pattern}")) {
		Ok(regex) => Some(regex),
		Err(_) => {
			log::error!("Invalid regex pattern: {pattern}");
			None
		}
	}
}

/// One matched substring with its byte range in the searched string and, for
/// a capture, the group's name.
struct Span {
	text: String,
	start: u64,
	end: u64,
	name: String,
}

/// The whole match then each capture group of the `match_index`-th match,
/// empty where the index resolves to no match.
fn capture_spans(regex: &fancy_regex::Regex, string: &str, match_index: f64) -> Vec<Span> {
	// Capture group names indexed positionally; index 0 (the whole match) is always None.
	let capture_names: Vec<Option<String>> = regex.capture_names().map(|name| name.map(str::to_string)).collect();

	// Collect all matches since we need to support negative indexing
	let matches: Vec<_> = regex.captures_iter(string).filter_map(|c| c.ok()).collect();
	let match_index = match_index as i32;
	let resolved_index = match match_index < 0 {
		true => match matches.len().checked_sub((-match_index) as usize) {
			Some(index) => index,
			None => return Vec::new(),
		},
		false => match_index as usize,
	};
	let Some(captures) = matches.get(resolved_index) else {
		return Vec::new();
	};

	(0..captures.len())
		.map(|i| {
			let captured = captures.get(i);
			Span {
				text: captured.map_or(String::new(), |m| m.as_str().to_string()),
				start: captured.map_or(0, |m| m.start() as u64),
				end: captured.map_or(0, |m| m.end() as u64),
				name: capture_names.get(i).cloned().flatten().unwrap_or_default(),
			}
		})
		.collect()
}

fn match_spans(regex: &fancy_regex::Regex, string: &str) -> Vec<Span> {
	regex
		.find_iter(string)
		.filter_map(|m| m.ok())
		.map(|m| Span {
			text: m.as_str().to_string(),
			start: m.start() as u64,
			end: m.end() as u64,
			name: String::new(),
		})
		.collect()
}

/// The parts of `string` between matches, the whole string without a usable pattern.
fn split_parts(regex: Option<&fancy_regex::Regex>, string: &str) -> Vec<String> {
	match regex {
		Some(regex) => regex.split(string).filter_map(|s| s.ok()).map(str::to_string).collect(),
		None => vec![string.to_string()],
	}
}

/// Finds a regex match in each string and returns its components, as one flat list where a match contributes the whole match (`$0`) followed by its capture groups (`$1`, `$2`, etc., if any).
///
/// The match index selects which non-overlapping occurrence to return (0 for the first match). A string contributes nothing if no match is found at the given index.
///
/// Each item carries `start` and `end` byte-offset attributes pointing into its original string, plus a `name` attribute holding
/// the capture group's name (empty for unnamed groups, and for index 0 which is the whole match).
#[node_macro::node(category(""), extent(regex_find_extent))]
fn regex_find<'e>(
	ctx: impl Ctx + ExtractArena<'e> + ExtractIndex + InjectIndex + Copy,
	/// The strings to search within.
	strings: IList<String>,
	/// The regular expression pattern to search for.
	pattern: String,
	/// Which non-overlapping occurrence of the pattern to return, starting from 0 for the first match. Negative indices count backwards from the last match.
	match_index: SignedInteger,
	/// Match letters regardless of case.
	case_insensitive: bool,
	/// Make `^` and `$` match the start and end of each line, not just the whole string.
	multiline: bool,
) -> Result<IList<(Lane<String>, Attr<'e, Start>, Attr<'e, End>, Attr<'e, Name>)>, Interrupt> {
	let regex = compile_regex(&pattern, case_insensitive, multiline);
	let (row, span) = locate_expanded(strings, ctx.index() as usize, |string| {
		regex.as_ref().map_or_else(Vec::new, |regex| capture_spans(regex, string, match_index))
	})
	.ok_or_else(|| Interrupt::from(GraphError::past_end()))?;
	let (name, _) = ctx.arena().alloc(span.name).ok_or_else(|| Interrupt::from(GraphError::new("the arena is exhausted")))?;
	Ok((strings.lane(row).map_element(span.text), Attr(span.start), Attr(span.end), Attr(name.as_str())))
}

/// The level holds every string's captures in order.
fn regex_find_extent(
	strings: ListIn<'_, String>,
	pattern: ValueIn<'_, String>,
	match_index: ValueIn<'_, f64>,
	case_insensitive: ValueIn<'_, bool>,
	multiline: ValueIn<'_, bool>,
	level: LevelIn,
) -> GPoll<Extent> {
	match level.top() {
		true => strings
			.get()
			.zip(pattern.get())
			.zip(match_index.get())
			.zip(case_insensitive.get())
			.zip(multiline.get())
			.map(|((((strings, pattern), match_index), case_insensitive), multiline)| {
				let regex = compile_regex(&pattern, case_insensitive, multiline);
				expanded_count(strings, |string| regex.as_ref().map_or(0, |regex| capture_spans(regex, string, match_index).len()))
			}),
		false => GPoll::Final(Extent::Exactly(1)),
	}
}

/// Finds all non-overlapping matches of a regular expression pattern in each string, returning one flat list of the matched substrings.
///
/// Each item carries `start` and `end` byte-offset attributes pointing into its original string.
#[node_macro::node(category("Text: Regex"), extent(regex_find_all_extent))]
fn regex_find_all<'e>(
	ctx: impl Ctx + ExtractArena<'e> + ExtractIndex + InjectIndex + Copy,
	/// The strings to search within.
	strings: IList<String>,
	/// The regular expression pattern to search for.
	pattern: String,
	/// Match letters regardless of case.
	case_insensitive: bool,
	/// Make `^` and `$` match the start and end of each line, not just the whole string.
	multiline: bool,
) -> Result<IList<(Lane<String>, Attr<'e, Start>, Attr<'e, End>)>, Interrupt> {
	let regex = compile_regex(&pattern, case_insensitive, multiline);
	let (row, span) =
		locate_expanded(strings, ctx.index() as usize, |string| regex.as_ref().map_or_else(Vec::new, |regex| match_spans(regex, string))).ok_or_else(|| Interrupt::from(GraphError::past_end()))?;
	Ok((strings.lane(row).map_element(span.text), Attr(span.start), Attr(span.end)))
}

/// The level holds every string's matches in order.
fn regex_find_all_extent(strings: ListIn<'_, String>, pattern: ValueIn<'_, String>, case_insensitive: ValueIn<'_, bool>, multiline: ValueIn<'_, bool>, level: LevelIn) -> GPoll<Extent> {
	match level.top() {
		true => strings
			.get()
			.zip(pattern.get())
			.zip(case_insensitive.get())
			.zip(multiline.get())
			.map(|(((strings, pattern), case_insensitive), multiline)| {
				let regex = compile_regex(&pattern, case_insensitive, multiline);
				expanded_count(strings, |string| regex.as_ref().map_or(0, |regex| match_spans(regex, string).len()))
			}),
		false => GPoll::Final(Extent::Exactly(1)),
	}
}

/// Splits each string into substrings pulled from between separator characters as matched by a regular expression, producing one flat list of all the substrings.
///
/// For example, splitting "Three, two, one... LIFTOFF" with pattern `\W+` (non-word characters) produces `["Three", "two", "one", "LIFTOFF"]`.
#[node_macro::node(category("Text: Regex"), extent(regex_split_extent))]
fn regex_split(
	ctx: impl Ctx + ExtractIndex + InjectIndex + Copy,
	/// The strings to split into substrings.
	strings: IList<String>,
	/// The regular expression pattern to split on. Matches are consumed and not included in the output.
	pattern: String,
	/// Match letters regardless of case.
	case_insensitive: bool,
	/// Make `^` and `$` match the start and end of each line, not just the whole string.
	multiline: bool,
) -> Result<IList<Lane<String>>, Interrupt> {
	let regex = compile_regex(&pattern, case_insensitive, multiline);
	let (row, part) = locate_expanded(strings, ctx.index() as usize, |string| split_parts(regex.as_ref(), string)).ok_or_else(|| Interrupt::from(GraphError::past_end()))?;
	Ok(strings.lane(row).map_element(part))
}

/// The level holds every string's parts in order.
fn regex_split_extent(strings: ListIn<'_, String>, pattern: ValueIn<'_, String>, case_insensitive: ValueIn<'_, bool>, multiline: ValueIn<'_, bool>, level: LevelIn) -> GPoll<Extent> {
	match level.top() {
		true => strings
			.get()
			.zip(pattern.get())
			.zip(case_insensitive.get())
			.zip(multiline.get())
			.map(|(((strings, pattern), case_insensitive), multiline)| {
				let regex = compile_regex(&pattern, case_insensitive, multiline);
				expanded_count(strings, |string| split_parts(regex.as_ref(), string).len())
			}),
		false => GPoll::Final(Extent::Exactly(1)),
	}
}

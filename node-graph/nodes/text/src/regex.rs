use core_types::list::{Item, List};
use core_types::{ATTR_END, ATTR_NAME, ATTR_START, Ctx};
use fancy_regex::{Expr, Regex};

/// Why a Regex node's pattern gives no result.
#[derive(Debug, thiserror::Error)]
pub enum RegexError {
	#[error("The regex pattern is invalid: {0}")]
	InvalidPattern(String),
	#[error("The regex pattern can't use `\\K` inside a lookaround")]
	KeepOutInLookaround,
	#[error("The regex pattern takes too many steps on this string ({0})")]
	TooComplex(fancy_regex::RuntimeError),
}

impl From<fancy_regex::Error> for RegexError {
	fn from(error: fancy_regex::Error) -> Self {
		// Leaves out the positions and quoted pattern the regex crates add, which count the flags prepended to the user's pattern
		let reason = match error {
			fancy_regex::Error::RuntimeError(error) => return Self::TooComplex(error),
			fancy_regex::Error::ParseError(_, error) => error.to_string(),
			fancy_regex::Error::CompileError(error) => match *error {
				fancy_regex::CompileError::InnerError(error) => {
					let reason = std::error::Error::source(&error).map_or_else(|| error.to_string(), ToString::to_string);
					reason.lines().find_map(|line| line.strip_prefix("error: ")).map_or_else(|| reason.clone(), str::to_string)
				}
				error => error.to_string(),
			},
			error => error.to_string(),
		};
		Self::InvalidPattern(reason)
	}
}

fn compile(pattern: &str, case_insensitive: bool, multiline: bool) -> Result<Regex, RegexError> {
	let flags = match (case_insensitive, multiline) {
		(false, false) => "",
		(true, false) => "(?i)",
		(false, true) => "(?m)",
		(true, true) => "(?im)",
	};
	Ok(Regex::new(&format!("{flags}{pattern}"))?)
}

/// Refuses a `\K` inside a lookaround for the nodes that slice the string between matches, which panics in fancy-regex once a match starts before the previous one ended.
fn refuse_keep_out_in_lookaround(pattern: &str) -> Result<(), RegexError> {
	let pattern = Expr::parse_tree(pattern)?.expr;

	fn push_groups<'a>(expr: &'a Expr, groups: &mut Vec<&'a Expr>) {
		if let Expr::Group(inner) = expr {
			groups.push(inner.as_ref());
		}
		for child in expr.children_iter() {
			push_groups(child, groups);
		}
	}
	let mut groups = vec![&pattern];
	push_groups(&pattern, &mut groups);

	// A subroutine call runs its group's `\K` where the call is, so a call inside a lookaround counts as a `\K` there
	let runs_keep_out = |groups_running: &[bool], expr: &Expr| {
		let runs = |expr: &Expr| matches!(expr, Expr::KeepOut) || matches!(expr, Expr::SubroutineCall(group) if groups_running.get(*group) == Some(&true));
		runs(expr) || expr.has_descendant(runs)
	};
	let mut groups_running = vec![false; groups.len()];
	loop {
		let next: Vec<bool> = groups.iter().map(|&group| runs_keep_out(&groups_running, group)).collect();
		if next == groups_running {
			break;
		}
		groups_running = next;
	}

	let keep_out_in_lookaround = |expr: &Expr| matches!(expr, Expr::LookAround(..)) && runs_keep_out(&groups_running, expr);
	if keep_out_in_lookaround(&pattern) || pattern.has_descendant(keep_out_in_lookaround) {
		return Err(RegexError::KeepOutInLookaround);
	}
	Ok(())
}

/// Checks whether the string contains a match for the given regular expression pattern. Optionally restricts the match to only the start and/or end of the string.
#[node_macro::node(category("String: Regex"), icon("NodeRegexContains"))]
fn regex_contains(
	_: impl Ctx,
	/// The string to search within.
	string: Item<String>,
	/// The regular expression pattern to search for.
	pattern: Item<String>,
	/// Match letters regardless of case.
	case_insensitive: Item<bool>,
	/// Make `^` and `$` match the start and end of each line, not just the whole string.
	multiline: Item<bool>,
	/// Only match if the pattern appears at the start of the string.
	at_start: Item<bool>,
	/// Only match if the pattern appears at the end of the string.
	at_end: Item<bool>,
) -> Result<Item<bool>, RegexError> {
	let (string, attributes) = string.into_parts();
	let pattern = pattern.element();
	let (case_insensitive, multiline, at_start, at_end) = (*case_insensitive.element(), *multiline.element(), *at_start.element(), *at_end.element());

	let regex = if at_start || at_end {
		// Parsing the pattern alone keeps a stray `)` in it from closing the anchoring group
		Expr::parse_tree(pattern)?;
		let (start, end) = (if at_start { "\\A" } else { "" }, if at_end { "\\z" } else { "" });
		// The newline ends a trailing `(?x)` comment, which would swallow the group's `)`, and the `(?x)` makes the newline whitespace otherwise
		compile(&format!("{start}(?:{pattern}(?x)\n){end}"), case_insensitive, multiline)?
	} else {
		compile(pattern, case_insensitive, multiline)?
	};

	Ok(Item::from_parts(regex.is_match(&string)?, attributes))
}

/// Replaces matches of a regular expression pattern in the string. The replacement string can reference captures: `$0` for the whole match and `$1`, `$2`, etc. for capture groups.
#[node_macro::node(category("String: Regex"), icon("NodeRegexReplace"))]
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
) -> Result<Item<String>, RegexError> {
	let mut string = string;
	let pattern = pattern.element().clone();
	let replacement = replacement.element().clone();
	let (replace_all, case_insensitive, multiline) = (*replace_all.element(), *case_insensitive.element(), *multiline.element());

	refuse_keep_out_in_lookaround(&pattern)?;
	let regex = compile(&pattern, case_insensitive, multiline)?;

	let limit = if replace_all { 0 } else { 1 };
	let result = regex.try_replacen(string.element(), limit, replacement.as_str())?.into_owned();

	*string.element_mut() = result;
	Ok(string)
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
	match_index: Item<i64>,
	/// Match letters regardless of case.
	case_insensitive: Item<bool>,
	/// Make `^` and `$` match the start and end of each line, not just the whole string.
	multiline: Item<bool>,
) -> Result<List<String>, RegexError> {
	let string = string.element();
	let pattern = pattern.element();
	let (match_index, case_insensitive, multiline) = (*match_index.element(), *case_insensitive.element(), *multiline.element());

	if pattern.is_empty() {
		return Ok(List::new());
	}

	let regex = compile(pattern, case_insensitive, multiline)?;

	// Capture group names indexed positionally; index 0 (the whole match) is always None.
	let capture_names: Vec<Option<String>> = regex.capture_names().map(|name| name.map(str::to_string)).collect();

	// Collect all matches since we need to support negative indexing
	let matches = regex.captures_iter(string).collect::<Result<Vec<_>, _>>()?;

	let resolved_index = if match_index < 0 {
		let from_end = match_index.unsigned_abs();
		if from_end > matches.len() as u64 {
			return Ok(List::new());
		}
		matches.len() - from_end as usize
	} else {
		match_index.min(matches.len() as i64) as usize
	};

	let Some(captures) = matches.get(resolved_index) else {
		return Ok(List::new());
	};

	// Index 0 is the whole match, 1+ are capture groups
	Ok((0..captures.len())
		.map(|i| {
			let captured = captures.get(i);
			let text = captured.map_or(String::new(), |m| m.as_str().to_string());
			let start = captured.map_or(0_i64, |m| m.start() as i64);
			let end = captured.map_or(0_i64, |m| m.end() as i64);
			let name = capture_names.get(i).cloned().flatten().unwrap_or_default();
			Item::new_from_element(text)
				.with_attribute(ATTR_START, start)
				.with_attribute(ATTR_END, end)
				.with_attribute(ATTR_NAME, name)
		})
		.collect())
}

/// Finds all non-overlapping matches of a regular expression pattern in the string, returning a list of the matched substrings.
///
/// Each item carries `start` and `end` byte-offset attributes pointing into the original string.
#[node_macro::node(category("String: Regex"), icon("NodeRegexFindAll"))]
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
) -> Result<List<String>, RegexError> {
	let string = string.element();
	let pattern = pattern.element();
	let (case_insensitive, multiline) = (*case_insensitive.element(), *multiline.element());

	if pattern.is_empty() {
		return Ok(List::new());
	}

	let regex = compile(pattern, case_insensitive, multiline)?;

	regex
		.find_iter(string)
		.map(|m| {
			let m = m?;
			Ok(Item::new_from_element(m.as_str().to_string())
				.with_attribute(ATTR_START, m.start() as i64)
				.with_attribute(ATTR_END, m.end() as i64))
		})
		.collect()
}

/// Splits a string into a list of substrings pulled from between separator characters as matched by a regular expression.
///
/// For example, splitting "Three, two, one... LIFTOFF" with pattern `\W+` (non-word characters) produces `["Three", "two", "one", "LIFTOFF"]`.
#[node_macro::node(category("String: Regex"), icon("NodeRegexSplit"))]
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
) -> Result<List<String>, RegexError> {
	let pattern = pattern.element().clone();
	let (case_insensitive, multiline) = (*case_insensitive.element(), *multiline.element());

	if pattern.is_empty() {
		return Ok(List::new_from_item(string));
	}

	refuse_keep_out_in_lookaround(&pattern)?;
	let regex = compile(&pattern, case_insensitive, multiline)?;

	regex.split(string.element()).map(|part| Ok(Item::new_from_element(part?.to_string()))).collect()
}

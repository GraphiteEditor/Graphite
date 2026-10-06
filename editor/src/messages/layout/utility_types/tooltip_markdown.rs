//! Tooltips render a subset of Markdown: `code` spans, fenced code blocks, `**bold**`, and `*italic*`, where a backslash makes a
//! following backslash, asterisk, or backtick literal. Text from outside the editor goes through these so it shows exactly as written.

/// Escapes text so its backslashes, asterisks, and backticks show as themselves rather than as formatting.
pub fn escape_markdown(text: &str) -> String {
	let mut escaped = String::with_capacity(text.len());
	for character in text.chars() {
		if matches!(character, '\\' | '*' | '`') {
			escaped.push('\\');
		}
		escaped.push(character);
	}
	escaped
}

/// Wraps text in a code span that nothing inside can close, as in CommonMark: the fence outruns every run of backticks within,
/// and a space pads the text where it would touch the fence or lose its own edge spaces to the span's trimming.
pub fn markdown_code_span(text: &str) -> String {
	if text.is_empty() {
		return String::new();
	}

	let longest_run = text.split(|character| character != '`').map(str::len).max().unwrap_or_default();
	let fence = "`".repeat(longest_run + 1);

	let touches_fence = text.starts_with('`') || text.ends_with('`');
	let loses_edge_spaces = text.starts_with(' ') && text.ends_with(' ') && text.contains(|character| character != ' ');
	let padding = if touches_fence || loses_edge_spaces { " " } else { "" };

	format!("{fence}{padding}{text}{padding}{fence}")
}

/// Wraps text in a fenced code block that nothing inside can close, as in CommonMark: the fence outruns every run of backticks within.
/// The fences are lines of their own, so the block goes at the start of a line.
pub fn markdown_code_block(text: &str) -> String {
	if text.is_empty() {
		return String::new();
	}

	let longest_run = text.split(|character| character != '`').map(str::len).max().unwrap_or_default();
	let fence = "`".repeat(longest_run.max(2) + 1);

	format!("{fence}\n{text}\n{fence}")
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn escaped_text_holds_no_formatting() {
		assert_eq!(escape_markdown("a*b*c"), "a\\*b\\*c");
		assert_eq!(escape_markdown("`code`"), "\\`code\\`");
		assert_eq!(escape_markdown("ends with \\"), "ends with \\\\");
		assert_eq!(escape_markdown("<b>&amp;</b>"), "<b>&amp;</b>", "HTML is the tooltip's to escape");
	}

	#[test]
	fn code_spans_contain_anything() {
		assert_eq!(markdown_code_span("x"), "`x`");
		assert_eq!(markdown_code_span("a`b"), "``a`b``");
		assert_eq!(markdown_code_span("a``b`"), "``` a``b` ```");
		assert_eq!(markdown_code_span("`"), "`` ` ``");
		assert_eq!(markdown_code_span(" x "), "`  x  `");
		assert_eq!(markdown_code_span(" "), "` `");
		assert_eq!(markdown_code_span("a\\"), "`a\\`", "a backslash is literal inside a span");
		assert_eq!(markdown_code_span("**<i>**"), "`**<i>**`");
		assert_eq!(markdown_code_span(""), "");
	}

	#[test]
	fn code_blocks_contain_anything() {
		assert_eq!(markdown_code_block("x\n^"), "```\nx\n^\n```");
		assert_eq!(markdown_code_block("```\n**<i>**"), "````\n```\n**<i>**\n````");
		assert_eq!(markdown_code_block("`"), "```\n`\n```");
		assert_eq!(markdown_code_block(""), "");
	}
}

// Marks where each code span or block goes while the prose around it is formatted, which HTML-escaped text can't hold since it has no `<`
const CODE_PLACEHOLDER = "<>";

export function escapeHtml(text: string): string {
	return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;").replace(/'/g, "&apos;");
}

// Renders the tooltip subset of Markdown: `code` spans, fenced code blocks, **bold**, and *italic*, where a backslash makes a following
// backslash, asterisk, or backtick literal. The text is HTML-escaped before anything else and code is never formatted, so no text yields other markup.
export function parseMarkdown(markdown: string | undefined): string | undefined {
	if (!markdown) return undefined;

	const escaped = escapeHtml(markdown);

	const codeElements: string[] = [];
	let text = "";
	let index = 0;
	while (index < escaped.length) {
		// A backslash makes a following backslash, asterisk, or backtick literal, written as an entity that no formatting matches
		if (escaped[index] === "\\" && ["\\", "*", "`"].includes(escaped[index + 1])) {
			text += `&#${escaped.charCodeAt(index + 1)};`;
			index += 2;
			continue;
		}

		if (escaped[index] === "`") {
			const fenceEnd = endOfBacktickRun(escaped, index);
			const fenceLength = fenceEnd - index;

			// A run of three or more backticks alone on a line opens a code block that only a line of at least as many closes, as in CommonMark
			const atLineStart = index === 0 || escaped[index - 1] === "\n";
			if (atLineStart && fenceLength >= 3 && escaped[fenceEnd] === "\n") {
				const contentStart = fenceEnd + 1;
				const { contentEnd, blockEnd } = endOfCodeBlock(escaped, contentStart, fenceLength);

				// The fence lines are the block's edges, so the line break before the block isn't a blank line above it
				if (text.endsWith("\n")) text = text.slice(0, -1);

				codeElements.push(`<pre><code>${escaped.slice(contentStart, contentEnd)}</code></pre>`);
				text += CODE_PLACEHOLDER;
				index = blockEnd;
				continue;
			}

			// Otherwise the run opens a code span that only a run of the same length closes
			const closing = nextBacktickRunOfLength(escaped, fenceEnd, fenceLength);
			if (closing === undefined) {
				text += escaped.slice(index, fenceEnd);
				index = fenceEnd;
				continue;
			}

			// One space is trimmed from each end when both have one, so a span can begin or end with a backtick
			let code = escaped.slice(fenceEnd, closing);
			if (code.startsWith(" ") && code.endsWith(" ") && /[^ ]/.test(code)) code = code.slice(1, -1);

			codeElements.push(`<code>${code}</code>`);
			text += CODE_PLACEHOLDER;
			index = closing + fenceLength;
			continue;
		}

		text += escaped[index];
		index += 1;
	}

	let codeElementIndex = 0;
	return (
		text
			// Bold
			.replace(/\*\*((?:(?!\*\*).)+)\*\*/g, "<strong>$1</strong>")
			// Italic
			.replace(/\*([^*]+)\*/g, "<em>$1</em>")
			// Code spans and blocks
			.replaceAll(CODE_PLACEHOLDER, () => codeElements[codeElementIndex++])
	);
}

// The end of a code block's content from `from`, and of the closing line of at least `fenceLength` backticks after it, or the text's end when none closes it
function endOfCodeBlock(text: string, from: number, fenceLength: number): { contentEnd: number; blockEnd: number } {
	const closingLine = new RegExp(`^\`{${fenceLength},}$`, "gm");
	closingLine.lastIndex = from;
	const closing = closingLine.exec(text);
	if (!closing) return { contentEnd: text.length, blockEnd: text.length };

	return { contentEnd: Math.max(from, closing.index - 1), blockEnd: Math.min(closing.index + closing[0].length + 1, text.length) };
}

// The index just past the run of backticks starting at `start`
function endOfBacktickRun(text: string, start: number): number {
	let end = start;
	while (text[end] === "`") end += 1;
	return end;
}

// The start of the next run of exactly `length` backticks at or after `from`, skipping longer and shorter runs
function nextBacktickRunOfLength(text: string, from: number, length: number): number | undefined {
	let start = text.indexOf("`", from);
	while (start !== -1) {
		const end = endOfBacktickRun(text, start);
		if (end - start === length) return start;
		start = text.indexOf("`", end);
	}
	return undefined;
}

// Necessary because innerText puts an extra newline character at the end when the text is more than one line.
export function cleanupTextInput(text: string): string {
	if (text[text.length - 1] === "\n") return text.slice(0, -1);
	return text;
}

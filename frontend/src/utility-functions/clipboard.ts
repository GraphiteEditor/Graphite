export function readAtCaret(cut: boolean): string | undefined {
	const element = window.document.activeElement;

	if (element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement) {
		const start = element.selectionStart;
		const end = element.selectionEnd;

		if ((!start && start !== 0) || (!end && end !== 0) || start === end) {
			return undefined;
		}

		const value = element.value;
		const selectedText = value.slice(start, end);

		if (cut) {
			element.value = value.slice(0, start) + value.slice(end);

			element.selectionStart = element.selectionEnd = start;
			element.dispatchEvent(new Event("input", { bubbles: true }));
		}

		return selectedText;
	}

	// A field that handles copying itself, like one displaying its text in other glyphs, gives the text it would put on the clipboard
	if (element instanceof HTMLElement && element.isContentEditable) {
		const clipboardData = new DataTransfer();
		const handled = !element.dispatchEvent(new ClipboardEvent(cut ? "cut" : "copy", { clipboardData, bubbles: true, cancelable: true }));
		if (handled) return clipboardData.getData("text/plain") || undefined;
	}

	const selection = window.getSelection();
	if (!selection || selection.rangeCount === 0) {
		return undefined;
	}

	const selectedText = String(selection);
	if (!selectedText) return undefined;

	if (cut) {
		const range = selection.getRangeAt(0);
		range.deleteContents();

		range.collapse(true);
		selection.removeAllRanges();
		selection.addRange(range);
	}

	return selectedText;
}

export function insertAtCaret(text: string) {
	const element = window.document.activeElement;

	if (!element) return;

	if (element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement) {
		const start = element.selectionStart;
		const end = element.selectionEnd;

		if ((!start && start !== 0) || (!end && end !== 0)) return;

		const value = element.value;

		element.value = value.slice(0, start) + text + value.slice(end);

		const newPos = start + text.length;
		element.selectionStart = element.selectionEnd = newPos;
	} else if (element instanceof HTMLElement && element.isContentEditable) {
		// A field that applies its own edits, like one displaying its text in other glyphs, takes the text as it would a paste
		const handled = !element.dispatchEvent(new InputEvent("beforeinput", { inputType: "insertFromPaste", data: text, bubbles: true, cancelable: true }));
		if (handled) return;

		const selection = window.getSelection();
		if (!selection || selection.rangeCount === 0) return;

		const range = selection.getRangeAt(0);
		range.deleteContents();

		const textNode = window.document.createTextNode(text);
		range.insertNode(textNode);

		range.setStartAfter(textNode);
		range.collapse(true);

		selection.removeAllRanges();
		selection.addRange(range);
	}

	element.dispatchEvent(new Event("input", { bubbles: true }));
}

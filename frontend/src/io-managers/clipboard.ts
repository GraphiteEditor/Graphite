import { type Editor } from "@graphite/editor";
import { TriggerClipboardWrite, TriggerSelectionRead, TriggerSelectionWrite } from "@graphite/messages";

export function createClipboardManager(editor: Editor) {
	// Subscribe to process backend event
	editor.subscriptions.subscribeJsMessage(TriggerClipboardWrite, (triggerTextCopy) => {
		// If the Clipboard API is supported in the browser, copy text to the clipboard
		navigator.clipboard?.writeText?.(triggerTextCopy.content);
	});
	editor.subscriptions.subscribeJsMessage(TriggerSelectionRead, async (data) => {
		editor.handle.readSelection(readAtCaret(data.cut), data.cut);
	});
	editor.subscriptions.subscribeJsMessage(TriggerSelectionWrite, async (data) => {
		insertAtCaret(data.content);
	});
}

function readAtCaret(cut: boolean): string | undefined {
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

	const selection = window.getSelection();
	if (!selection || selection.rangeCount === 0) {
		return undefined;
	}

	const selectedText = selection.toString();
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

function insertAtCaret(text: string) {
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

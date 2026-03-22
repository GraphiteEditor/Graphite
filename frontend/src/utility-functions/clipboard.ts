import { extractPixelData } from "/src/utility-functions/rasterization";
import { stripIndents } from "/src/utility-functions/strip-indents";
import type { EditorWrapper } from "/wrapper/pkg/graphite_wasm_wrapper";

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

export async function triggerClipboardRead(editor: EditorWrapper) {
	// In the try block, attempt to read from the Clipboard API, which may not have permission and may not be supported in all browsers
	// In the catch block, explain to the user why the paste failed and how to fix or work around the problem
	try {
		// Attempt to check if the clipboard permission is denied, and throw an error if that is the case
		// In Firefox, the `clipboard-read` permission isn't supported, so attempting to query it throws an error
		// In Safari, the entire Permissions API isn't supported, so the query never occurs and this block is skipped without an error and we assume we might have permission
		const permission = await navigator.permissions?.query({ name: "clipboard-read" });
		if (permission?.state === "denied") throw new Error("Permission denied");

		// Read the clipboard contents if the Clipboard API is available
		const clipboardItems = await navigator.clipboard.read();
		if (!clipboardItems) throw new Error("Clipboard API unsupported");

		// Read any layer data or images from the clipboard
		const success = await Promise.any(
			Array.from(clipboardItems).map(async (item) => {
				// Read plain text and, if it is a layer, pass it to the editor
				if (item.types.includes("text/plain")) {
					const blob = await item.getType("text/plain");
					const reader = new FileReader();
					reader.onload = () => {
						if (typeof reader.result === "string") editor.pasteText(reader.result);
					};
					reader.readAsText(blob);
					return true;
				}

				// Read an image from the clipboard and pass it to the editor to be loaded
				const imageType = item.types.find((type) => type.startsWith("image/"));

				// Import the actual SVG content if it's an SVG
				if (imageType?.includes("svg")) {
					const blob = await item.getType("text/plain");
					const reader = new FileReader();
					reader.onload = () => {
						if (typeof reader.result === "string") editor.pasteSvg(undefined, reader.result);
					};
					reader.readAsText(blob);
					return true;
				}

				// Import the bitmap image if it's an image
				if (imageType) {
					const blob = await item.getType(imageType);
					const reader = new FileReader();
					reader.onload = async () => {
						if (reader.result instanceof ArrayBuffer) {
							const imageData = await extractPixelData(new Blob([reader.result], { type: imageType }));
							editor.pasteImage(undefined, new Uint8Array(imageData.data), imageData.width, imageData.height);
						}
					};
					reader.readAsArrayBuffer(blob);
					return true;
				}

				// The API limits what kinds of data we can access, so we can get copied images and our text encodings of copied nodes, but not files (like
				// .graphite or even image files). However, the user can paste those with Ctrl+V, which we recommend they in the error message that's shown to them.
				return false;
			}),
		);

		if (!success) throw new Error("No valid clipboard data");
	} catch (err) {
		const unsupported = stripIndents`
			This browser does not support reading from the clipboard.
			Use the standard keyboard shortcut to paste instead.
			`;
		const denied = stripIndents`
			The browser's clipboard permission has been denied.

			Open the browser's website settings (usually accessible
			just left of the URL bar) to allow this permission.
			`;
		const nothing = stripIndents`
			No valid clipboard data was found. You may have better
			success pasting with the standard keyboard shortcut instead.
			`;

		const matchMessage = {
			"clipboard-read": unsupported,
			"Clipboard API unsupported": unsupported,
			"Permission denied": denied,
			"No valid clipboard data": nothing,
		};
		const message = Object.entries(matchMessage).find(([key]) => String(err).includes(key))?.[1] || String(err);

		editor.errorDialog("Cannot access clipboard", message);
	}
}

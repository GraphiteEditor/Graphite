import { type Editor } from "~/src/wasm-communication/editor";
import { TriggerTextCopy } from "~/src/wasm-communication/messages";

export function createClipboardManager(editor: Editor): void {
	// Subscribe to process backend event
	editor.subscriptions.subscribeJsMessage(TriggerTextCopy, (triggerTextCopy) => {
		// If the Clipboard API is supported in the browser, copy text to the clipboard
		navigator.clipboard?.writeText?.(triggerTextCopy.copyText);
	});
}

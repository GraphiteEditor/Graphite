import { type Editor } from "@graphite/editor";
import { TriggerClipboardWrite } from "@graphite/messages";

export function createClipboardManager(editor: Editor) {
	// Subscribe to process backend event
	editor.subscriptions.subscribeJsMessage(TriggerClipboardWrite, (triggerTextCopy) => {
		// If the Clipboard API is supported in the browser, copy text to the clipboard
		navigator.clipboard?.writeText?.(triggerTextCopy.content);
	});
}

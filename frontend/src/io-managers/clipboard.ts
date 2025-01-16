import { type Editor } from "@graphite/editor";
import { TriggerTextCopy } from "@graphite/messages";

export function createClipboardManager(editor: Editor) {
	// Subscribe to process backend event
	editor.subscriptions.subscribeJsMessage(TriggerTextCopy, (triggerTextCopy) => {
		// If the Clipboard API is supported in the browser, copy text to the clipboard
		navigator.clipboard?.writeText?.(triggerTextCopy.copyText);
	});
}

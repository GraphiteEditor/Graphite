import type { Editor } from "@graphite/editor";
import { insertAtCaret, readAtCaret } from "@graphite/utility-functions/clipboard";

let editorRef: Editor | undefined = undefined;

export function createClipboardManager(editor: Editor) {
	destroyClipboardManager();

	editorRef = editor;

	editor.subscriptions.subscribeFrontendMessage("TriggerClipboardWrite", (data) => {
		// If the Clipboard API is supported in the browser, copy text to the clipboard
		navigator.clipboard?.writeText?.(data.content);
	});

	editor.subscriptions.subscribeFrontendMessage("TriggerSelectionRead", async (data) => {
		editor.handle.readSelection(readAtCaret(data.cut), data.cut);
	});

	editor.subscriptions.subscribeFrontendMessage("TriggerSelectionWrite", async (data) => {
		insertAtCaret(data.content);
	});
}

export function destroyClipboardManager() {
	const editor = editorRef;
	if (!editor) return;

	editor.subscriptions.unsubscribeFrontendMessage("TriggerClipboardWrite");
	editor.subscriptions.unsubscribeFrontendMessage("TriggerSelectionRead");
	editor.subscriptions.unsubscribeFrontendMessage("TriggerSelectionWrite");
}

// Self-accepting HMR: tear down the old instance and re-create with the new module's code
import.meta.hot?.accept((newModule) => {
	if (editorRef) newModule?.createClipboardManager(editorRef);
});

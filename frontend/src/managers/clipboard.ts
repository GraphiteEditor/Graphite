import type { EditorHandle } from "@graphite/../wasm/pkg/graphite_wasm";
import type { SubscriptionRouter } from "@graphite/subscription-router";
import { insertAtCaret, readAtCaret } from "@graphite/utility-functions/clipboard";

let subscriptionsRef: SubscriptionRouter | undefined = undefined;
let editorRef: EditorHandle | undefined = undefined;

export function createClipboardManager(subscriptions: SubscriptionRouter, editor: EditorHandle) {
	destroyClipboardManager();

	subscriptionsRef = subscriptions;
	editorRef = editor;

	subscriptions.subscribeFrontendMessage("TriggerClipboardWrite", (data) => {
		// If the Clipboard API is supported in the browser, copy text to the clipboard
		navigator.clipboard?.writeText?.(data.content);
	});

	subscriptions.subscribeFrontendMessage("TriggerSelectionRead", async (data) => {
		editor.readSelection(readAtCaret(data.cut), data.cut);
	});

	subscriptions.subscribeFrontendMessage("TriggerSelectionWrite", async (data) => {
		insertAtCaret(data.content);
	});
}

export function destroyClipboardManager() {
	const subscriptions = subscriptionsRef;
	if (!subscriptions) return;

	subscriptions.unsubscribeFrontendMessage("TriggerClipboardWrite");
	subscriptions.unsubscribeFrontendMessage("TriggerSelectionRead");
	subscriptions.unsubscribeFrontendMessage("TriggerSelectionWrite");
}

// Self-accepting HMR: tear down the old instance and re-create with the new module's code
import.meta.hot?.accept((newModule) => {
	if (subscriptionsRef && editorRef) newModule?.createClipboardManager(subscriptionsRef, editorRef);
});

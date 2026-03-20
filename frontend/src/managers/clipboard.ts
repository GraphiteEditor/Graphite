import type { SubscriptionsRouter } from "/src/subscriptions-router";
import { insertAtCaret, readAtCaret } from "/src/utility-functions/clipboard";
import type { EditorHandle } from "/wasm/pkg/graphite_wasm";

let subscriptionsRouter: SubscriptionsRouter | undefined = undefined;
let editorHandle: EditorHandle | undefined = undefined;

export function createClipboardManager(subscriptions: SubscriptionsRouter, editor: EditorHandle) {
	destroyClipboardManager();

	subscriptionsRouter = subscriptions;
	editorHandle = editor;

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
	const subscriptions = subscriptionsRouter;
	if (!subscriptions) return;

	subscriptions.unsubscribeFrontendMessage("TriggerClipboardWrite");
	subscriptions.unsubscribeFrontendMessage("TriggerSelectionRead");
	subscriptions.unsubscribeFrontendMessage("TriggerSelectionWrite");
}

// Self-accepting HMR: tear down the old instance and re-create with the new module's code
import.meta.hot?.accept((newModule) => {
	if (subscriptionsRouter && editorHandle) newModule?.createClipboardManager(subscriptionsRouter, editorHandle);
});

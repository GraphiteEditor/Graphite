import type { SubscriptionsRouter } from "/src/subscriptions-router";
import { insertAtCaret, readAtCaret } from "/src/utility-functions/clipboard";
import type { EditorWrapper } from "/wrapper/pkg/graphite_wasm_wrapper";

let subscriptionsRouter: SubscriptionsRouter | undefined = undefined;
let editorWrapper: EditorWrapper | undefined = undefined;

export function createClipboardManager(subscriptions: SubscriptionsRouter, editor: EditorWrapper) {
	destroyClipboardManager();

	subscriptionsRouter = subscriptions;
	editorWrapper = editor;

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

	subscriptions.subscribeFrontendMessage("TriggerClipboardSvgWrite", (data) => {
		// Adopted from https://developer.mozilla.org/en-US/docs/Web/API/ClipboardItem#browser_compatibility
		if (ClipboardItem.supports("image/svg+xml")) {
			navigator.clipboard?.write?.([
				new ClipboardItem({
					"image/svg+xml": data.svg_string,
					"text/plain": data.graphite_json,
				}),
			]);
		} else {
			navigator.clipboard?.writeText?.(data.graphite_json);
		}
	});
}

export function destroyClipboardManager() {
	const subscriptions = subscriptionsRouter;
	if (!subscriptions) return;

	subscriptions.unsubscribeFrontendMessage("TriggerClipboardWrite");
	subscriptions.unsubscribeFrontendMessage("TriggerClipboardSvgWrite");
	subscriptions.unsubscribeFrontendMessage("TriggerSelectionRead");
	subscriptions.unsubscribeFrontendMessage("TriggerSelectionWrite");
}

// Self-accepting HMR: tear down the old instance and re-create with the new module's code
import.meta.hot?.accept((newModule) => {
	if (subscriptionsRouter && editorWrapper) newModule?.createClipboardManager(subscriptionsRouter, editorWrapper);
});

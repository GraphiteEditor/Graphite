import type { SubscriptionsRouter } from "/src/subscriptions-router";
import { insertAtCaret, readAtCaret } from "/src/utility-functions/clipboard";
import { targetIsTextField } from "/src/utility-functions/input";
import type { EditorWrapper } from "/wrapper/pkg/graphite_wasm_wrapper";

let subscriptionsRouter: SubscriptionsRouter | undefined = undefined;
let editorWrapper: EditorWrapper | undefined = undefined;

export function createClipboardManager(subscriptions: SubscriptionsRouter, editor: EditorWrapper) {
	destroyClipboardManager();

	subscriptionsRouter = subscriptions;
	editorWrapper = editor;

	subscriptions.subscribeFrontendMessage("TriggerSelectionRead", async (data) => {
		const content = readAtCaret(data.cut);
		// A text field with nothing selected has nothing to copy, rather than leaving the shortcut to act on the selected layers
		if (content === undefined && targetIsTextField(window.document.activeElement || undefined)) return;

		editor.readSelection(content, data.cut);
	});

	subscriptions.subscribeFrontendMessage("TriggerSelectionWrite", async (data) => {
		insertAtCaret(data.content);
	});

	subscriptions.subscribeFrontendMessage("TriggerClipboardSvgAndJsonWrite", (data) => {
		// Adopted from https://developer.mozilla.org/en-US/docs/Web/API/ClipboardItem#browser_compatibility
		if (ClipboardItem.supports("image/svg+xml") && data.svg_string !== undefined) {
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

	subscriptions.unsubscribeFrontendMessage("TriggerClipboardSvgAndJsonWrite");
	subscriptions.unsubscribeFrontendMessage("TriggerSelectionRead");
	subscriptions.unsubscribeFrontendMessage("TriggerSelectionWrite");
}

// Self-accepting HMR: tear down the old instance and re-create with the new module's code
import.meta.hot?.accept((newModule) => {
	if (subscriptionsRouter && editorWrapper) newModule?.createClipboardManager(subscriptionsRouter, editorWrapper);
});

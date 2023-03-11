import { type Editor } from "~/src/wasm-communication/editor";
import { TriggerVisitLink } from "~/src/wasm-communication/messages";

export function createHyperlinkManager(editor: Editor): void {
	// Subscribe to process backend event
	editor.subscriptions.subscribeJsMessage(TriggerVisitLink, async (triggerOpenLink) => {
		window.open(triggerOpenLink.url, "_blank");
	});
}

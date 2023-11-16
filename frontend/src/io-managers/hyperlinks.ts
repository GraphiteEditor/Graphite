import { type Editor } from "@graphite/wasm-communication/editor";
import { TriggerVisitLink } from "@graphite/wasm-communication/messages";

export function createHyperlinkManager(editor: Editor) {
	// Subscribe to process backend event
	editor.subscriptions.subscribeJsMessage(TriggerVisitLink, async (triggerOpenLink) => {
		window.open(triggerOpenLink.url, "_blank");
	});
}

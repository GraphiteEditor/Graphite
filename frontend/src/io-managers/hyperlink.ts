import { type Editor } from "@graphite/editor";
import { TriggerVisitLink } from "@graphite/messages";

export function createHyperlinkManager(editor: Editor) {
	// Subscribe to process backend event
	editor.subscriptions.subscribeJsMessage(TriggerVisitLink, async (data) => {
		window.open(data.url, "_blank");
	});
}

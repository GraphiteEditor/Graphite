import type { Editor } from "@graphite/editor";

export function createHyperlinkManager(editor: Editor) {
	// Subscribe to process backend event
	editor.subscriptions.subscribeJsMessage("TriggerVisitLink", async (data) => {
		window.open(data.url, "_blank");
	});
}

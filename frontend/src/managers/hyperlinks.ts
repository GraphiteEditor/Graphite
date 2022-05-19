import { Editor } from "@/interop/editor";
import { TriggerVisitLink } from "@/interop/messages";

export function createHyperlinkManager(editor: Editor): void {
	// Subscribe to process backend event
	editor.subscriptions.subscribeJsMessage(TriggerVisitLink, async (triggerOpenLink) => {
		window.open(triggerOpenLink.url, "_blank");
	});
}

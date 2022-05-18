import { Editor } from "@/interop/editor";
import { TriggerVisitLink } from "@/interop/messages";

export function createHyperlinkManager(editor: Editor): void {
	editor.subscriptions.subscribeJsMessage(TriggerVisitLink, async (triggerOpenLink) => {
		window.open(triggerOpenLink.url, "_blank");
	});
}

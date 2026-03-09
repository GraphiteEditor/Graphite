import type { Editor } from "@graphite/editor";

let currentCleanup: (() => void) | undefined;
let currentArgs: [Editor] | undefined;

export function createHyperlinkManager(editor: Editor): () => void {
	currentArgs = [editor];

	// Subscribe to process backend event
	editor.subscriptions.subscribeFrontendMessage("TriggerVisitLink", async (data) => {
		window.open(data.url, "_blank");
	});

	currentCleanup = () => {
		editor.subscriptions.unsubscribeFrontendMessage("TriggerVisitLink");
	};
	return currentCleanup;
}

// Self-accepting HMR: tear down the old instance and re-create with the new module's code
import.meta.hot?.accept((newModule) => {
	currentCleanup?.();
	if (currentArgs) newModule?.createHyperlinkManager(...currentArgs);
});

import type { Editor } from "@graphite/editor";

let editorRef: Editor | undefined = undefined;

export function createHyperlinkManager(editor: Editor) {
	destroyHyperlinkManager();

	editorRef = editor;

	editor.subscriptions.subscribeFrontendMessage("TriggerVisitLink", async (data) => {
		window.open(data.url, "_blank", "noopener");
	});
}

export function destroyHyperlinkManager() {
	const editor = editorRef;
	if (!editor) return;

	editor.subscriptions.unsubscribeFrontendMessage("TriggerVisitLink");
}

// Self-accepting HMR: tear down the old instance and re-create with the new module's code
import.meta.hot?.accept((newModule) => {
	if (editorRef) newModule?.createHyperlinkManager(editorRef);
});

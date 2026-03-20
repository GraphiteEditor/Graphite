import type { Editor } from "@graphite/editor";
import { localizeTimestamp } from "@graphite/utility-functions/time";

let editorRef: Editor | undefined = undefined;

export function createLocalizationManager(editor: Editor) {
	destroyLocalizationManager();

	editorRef = editor;

	editor.subscriptions.subscribeFrontendMessage("TriggerAboutGraphiteLocalizedCommitDate", (data) => {
		const localized = localizeTimestamp(data.commitDate);
		editor.handle.requestAboutGraphiteDialogWithLocalizedCommitDate(localized.timestamp, localized.year);
	});
}

export function destroyLocalizationManager() {
	const editor = editorRef;
	if (!editor) return;

	editor.subscriptions.unsubscribeFrontendMessage("TriggerAboutGraphiteLocalizedCommitDate");
}

// Self-accepting HMR: tear down the old instance and re-create with the new module's code
import.meta.hot?.accept((newModule) => {
	if (editorRef) newModule?.createLocalizationManager(editorRef);
});

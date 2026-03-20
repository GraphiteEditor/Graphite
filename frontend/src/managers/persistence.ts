import type { Editor } from "@graphite/editor";
import type { PortfolioStore } from "@graphite/stores/portfolio";
import { saveEditorPreferences, loadEditorPreferences, storeDocument, removeDocument, loadFirstDocument, loadRestDocuments, saveActiveDocument } from "@graphite/utility-functions/persistence";

let editorRef: Editor | undefined = undefined;
let portfolioStore: PortfolioStore | undefined = undefined;

export function createPersistenceManager(editor: Editor, portfolio: PortfolioStore) {
	destroyPersistenceManager();

	editorRef = editor;
	portfolioStore = portfolio;

	editor.subscriptions.subscribeFrontendMessage("TriggerSavePreferences", async (data) => {
		await saveEditorPreferences(data.preferences);
	});

	editor.subscriptions.subscribeFrontendMessage("TriggerLoadPreferences", async () => {
		await loadEditorPreferences(editor);
	});

	editor.subscriptions.subscribeFrontendMessage("TriggerPersistenceWriteDocument", async (data) => {
		await storeDocument(data, portfolio);
	});

	editor.subscriptions.subscribeFrontendMessage("TriggerPersistenceRemoveDocument", async (data) => {
		await removeDocument(String(data.documentId), portfolio);
	});

	editor.subscriptions.subscribeFrontendMessage("TriggerLoadFirstAutoSaveDocument", async () => {
		await loadFirstDocument(editor);
	});

	editor.subscriptions.subscribeFrontendMessage("TriggerLoadRestAutoSaveDocuments", async () => {
		await loadRestDocuments(editor);
	});

	editor.subscriptions.subscribeFrontendMessage("TriggerOpenLaunchDocuments", async () => {
		// TODO: Could be used to load documents from URL params or similar on launch
	});

	editor.subscriptions.subscribeFrontendMessage("TriggerSaveActiveDocument", async (data) => {
		await saveActiveDocument(data.documentId);
	});
}

export function destroyPersistenceManager() {
	const editor = editorRef;
	if (!editor) return;

	editor.subscriptions.unsubscribeFrontendMessage("TriggerSavePreferences");
	editor.subscriptions.unsubscribeFrontendMessage("TriggerLoadPreferences");
	editor.subscriptions.unsubscribeFrontendMessage("TriggerPersistenceWriteDocument");
	editor.subscriptions.unsubscribeFrontendMessage("TriggerPersistenceRemoveDocument");
	editor.subscriptions.unsubscribeFrontendMessage("TriggerLoadFirstAutoSaveDocument");
	editor.subscriptions.unsubscribeFrontendMessage("TriggerLoadRestAutoSaveDocuments");
	editor.subscriptions.unsubscribeFrontendMessage("TriggerOpenLaunchDocuments");
	editor.subscriptions.unsubscribeFrontendMessage("TriggerSaveActiveDocument");
}

// Self-accepting HMR: tear down the old instance and re-create with the new module's code
import.meta.hot?.accept((newModule) => {
	if (editorRef && portfolioStore) newModule?.createPersistenceManager(editorRef, portfolioStore);
});

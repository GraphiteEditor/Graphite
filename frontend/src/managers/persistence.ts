import type { PortfolioStore } from "/src/stores/portfolio";
import type { SubscriptionsRouter } from "/src/subscriptions-router";
import { saveEditorPreferences, loadEditorPreferences, storeDocument, removeDocument, loadFirstDocument, loadRestDocuments, saveActiveDocument } from "/src/utility-functions/persistence";
import type { EditorWrapper } from "/wrapper/pkg/graphite_wasm_wrapper";

let subscriptionsRouter: SubscriptionsRouter | undefined = undefined;
let editorWrapper: EditorWrapper | undefined = undefined;
let portfolioStore: PortfolioStore | undefined = undefined;

export function createPersistenceManager(subscriptions: SubscriptionsRouter, editor: EditorWrapper, portfolio: PortfolioStore) {
	destroyPersistenceManager();

	subscriptionsRouter = subscriptions;
	editorWrapper = editor;
	portfolioStore = portfolio;

	subscriptions.subscribeFrontendMessage("TriggerSavePreferences", async (data) => {
		await saveEditorPreferences(data.preferences);
	});

	subscriptions.subscribeFrontendMessage("TriggerLoadPreferences", async () => {
		await loadEditorPreferences(editor);
	});

	subscriptions.subscribeFrontendMessage("TriggerPersistenceWriteDocument", async (data) => {
		await storeDocument(data, portfolio);
	});

	subscriptions.subscribeFrontendMessage("TriggerPersistenceRemoveDocument", async (data) => {
		await removeDocument(String(data.documentId), portfolio);
	});

	subscriptions.subscribeFrontendMessage("TriggerLoadFirstAutoSaveDocument", async () => {
		await loadFirstDocument(editor);
	});

	subscriptions.subscribeFrontendMessage("TriggerLoadRestAutoSaveDocuments", async () => {
		await loadRestDocuments(editor);
	});

	subscriptions.subscribeFrontendMessage("TriggerOpenLaunchDocuments", async () => {
		// TODO: Could be used to load documents from URL params or similar on launch
	});

	subscriptions.subscribeFrontendMessage("TriggerSaveActiveDocument", async (data) => {
		await saveActiveDocument(data.documentId);
	});
}

export function destroyPersistenceManager() {
	const subscriptions = subscriptionsRouter;
	if (!subscriptions) return;

	subscriptions.unsubscribeFrontendMessage("TriggerSavePreferences");
	subscriptions.unsubscribeFrontendMessage("TriggerLoadPreferences");
	subscriptions.unsubscribeFrontendMessage("TriggerPersistenceWriteDocument");
	subscriptions.unsubscribeFrontendMessage("TriggerPersistenceRemoveDocument");
	subscriptions.unsubscribeFrontendMessage("TriggerLoadFirstAutoSaveDocument");
	subscriptions.unsubscribeFrontendMessage("TriggerLoadRestAutoSaveDocuments");
	subscriptions.unsubscribeFrontendMessage("TriggerOpenLaunchDocuments");
	subscriptions.unsubscribeFrontendMessage("TriggerSaveActiveDocument");
}

// Self-accepting HMR: tear down the old instance and re-create with the new module's code
import.meta.hot?.accept((newModule) => {
	if (subscriptionsRouter && editorWrapper && portfolioStore) newModule?.createPersistenceManager(subscriptionsRouter, editorWrapper, portfolioStore);
});

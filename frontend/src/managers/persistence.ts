import type { EditorHandle } from "@graphite/../wasm/pkg/graphite_wasm";
import type { PortfolioStore } from "@graphite/stores/portfolio";
import type { SubscriptionRouter } from "@graphite/subscription-router";
import { saveEditorPreferences, loadEditorPreferences, storeDocument, removeDocument, loadFirstDocument, loadRestDocuments, saveActiveDocument } from "@graphite/utility-functions/persistence";

let subscriptionsRef: SubscriptionRouter | undefined = undefined;
let editorRef: EditorHandle | undefined = undefined;
let portfolioStore: PortfolioStore | undefined = undefined;

export function createPersistenceManager(subscriptions: SubscriptionRouter, editor: EditorHandle, portfolio: PortfolioStore) {
	destroyPersistenceManager();

	subscriptionsRef = subscriptions;
	editorRef = editor;
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
	const subscriptions = subscriptionsRef;
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
	if (subscriptionsRef && editorRef && portfolioStore) newModule?.createPersistenceManager(subscriptionsRef, editorRef, portfolioStore);
});

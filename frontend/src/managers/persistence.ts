import type { PortfolioStore } from "/src/stores/portfolio";
import type { SubscriptionsRouter } from "/src/subscriptions-router";
import { saveEditorPreferences, loadEditorPreferences, writePersistedState, readPersistedState } from "/src/utility-functions/persistence";
import type { EditorWrapper } from "/wrapper/pkg/graphite_wasm_wrapper";

let subscriptionsRouter: SubscriptionsRouter | undefined = undefined;
let editorWrapper: EditorWrapper | undefined = undefined;
let portfolioStore: PortfolioStore | undefined = undefined;

export function createPersistenceManager(subscriptions: SubscriptionsRouter, editor: EditorWrapper, portfolio: PortfolioStore) {
	destroyPersistenceManager();

	subscriptionsRouter = subscriptions;
	editorWrapper = editor;
	portfolioStore = portfolio;

	// Run persistence operations in request order
	let tail: Promise<void> = Promise.resolve();
	const enqueue = (operation: () => Promise<void>): Promise<void> => {
		const next = tail.then(operation);
		tail = next.catch(() => undefined);
		return next;
	};

	subscriptions.subscribeFrontendMessage("TriggerSavePreferences", (data) => enqueue(() => saveEditorPreferences(data.preferences)));

	subscriptions.subscribeFrontendMessage("TriggerLoadPreferences", () => enqueue(() => loadEditorPreferences(editor)));

	subscriptions.subscribeFrontendMessage("TriggerPersistenceWriteState", (data) => enqueue(() => writePersistedState(data.state)));

	subscriptions.subscribeFrontendMessage("TriggerPersistenceReadState", () => enqueue(() => readPersistedState(editor)));

	subscriptions.subscribeFrontendMessage("TriggerOpenLaunchDocuments", async () => {
		// TODO: Could be used to load documents from URL params or similar on launch
	});
}

export function destroyPersistenceManager() {
	const subscriptions = subscriptionsRouter;
	if (!subscriptions) return;

	subscriptions.unsubscribeFrontendMessage("TriggerSavePreferences");
	subscriptions.unsubscribeFrontendMessage("TriggerLoadPreferences");
	subscriptions.unsubscribeFrontendMessage("TriggerPersistenceWriteState");
	subscriptions.unsubscribeFrontendMessage("TriggerPersistenceReadState");
	subscriptions.unsubscribeFrontendMessage("TriggerOpenLaunchDocuments");
}

// Self-accepting HMR: tear down the old instance and re-create with the new module's code
import.meta.hot?.accept((newModule) => {
	if (subscriptionsRouter && editorWrapper && portfolioStore) newModule?.createPersistenceManager(subscriptionsRouter, editorWrapper, portfolioStore);
});

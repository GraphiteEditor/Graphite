import { writable } from "svelte/store";
import type { Writable } from "svelte/store";
import type { SubscriptionsRouter } from "/src/subscriptions-router";
import type { HistoryPanelState } from "/wrapper/pkg/graphite_wasm_wrapper";

export type HistoryStore = ReturnType<typeof createHistoryStore>;

const initialState: HistoryPanelState = { rows: [], progress: [], more: false, following: undefined, session: false };

// Store state persisted across HMR to maintain reactive subscriptions in the component tree
const store: Writable<HistoryPanelState> = import.meta.hot?.data?.store || writable<HistoryPanelState>(initialState);

const { subscribe, set } = store;

let subscriptionsRouter: SubscriptionsRouter | undefined = undefined;

export function createHistoryStore(subscriptions: SubscriptionsRouter) {
	destroyHistoryStore();
	subscriptionsRouter = subscriptions;

	subscriptions.subscribeFrontendMessage("UpdateHistoryPanel", (data) => {
		set(data.state);
	});

	return { subscribe };
}

export function destroyHistoryStore() {
	const subscriptions = subscriptionsRouter;
	if (!subscriptions) return;

	subscriptions.unsubscribeFrontendMessage("UpdateHistoryPanel");
	subscriptionsRouter = undefined;
}

// Self-accepting HMR: keep the store instance so component subscriptions stay live
if (import.meta.hot) {
	import.meta.hot.accept();
	import.meta.hot.data.store = store;
}

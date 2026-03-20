import { tick } from "svelte";
import { writable } from "svelte/store";
import type { Writable } from "svelte/store";

import type { Layout } from "@graphite/../wasm/pkg/graphite_wasm";
import type { SubscriptionRouter } from "@graphite/subscription-router";
import { patchLayout } from "@graphite/utility-functions/widgets";

export type DocumentStore = ReturnType<typeof createDocumentStore>;

type DocumentStoreState = {
	toolOptionsLayout: Layout;
	documentBarLayout: Layout;
	toolShelfLayout: Layout;
	workingColorsLayout: Layout;
	nodeGraphControlBarLayout: Layout;
	graphViewOverlayOpen: boolean;
	fadeArtwork: number;
};
const initialState: DocumentStoreState = {
	toolOptionsLayout: [],
	documentBarLayout: [],
	toolShelfLayout: [],
	workingColorsLayout: [],
	nodeGraphControlBarLayout: [],
	graphViewOverlayOpen: false,
	fadeArtwork: 100,
};

let subscriptionsRef: SubscriptionRouter | undefined = undefined;

// Store state persisted across HMR to maintain reactive subscriptions in the component tree
const store: Writable<DocumentStoreState> = import.meta.hot?.data?.store || writable<DocumentStoreState>(initialState);
if (import.meta.hot) import.meta.hot.data.store = store;
const { subscribe, update } = store;

export function createDocumentStore(subscriptions: SubscriptionRouter) {
	destroyDocumentStore();

	subscriptionsRef = subscriptions;

	subscriptions.subscribeFrontendMessage("UpdateGraphFadeArtwork", (data) => {
		update((state) => {
			state.fadeArtwork = data.percentage;
			return state;
		});
	});

	subscriptions.subscribeLayoutUpdate("ToolOptions", async (data) => {
		await tick();

		update((state) => {
			patchLayout(state.toolOptionsLayout, data);
			return state;
		});
	});

	subscriptions.subscribeLayoutUpdate("DocumentBar", async (data) => {
		await tick();

		update((state) => {
			patchLayout(state.documentBarLayout, data);
			return state;
		});
	});

	subscriptions.subscribeLayoutUpdate("ToolShelf", async (data) => {
		await tick();

		update((state) => {
			patchLayout(state.toolShelfLayout, data);
			return state;
		});
	});

	subscriptions.subscribeLayoutUpdate("WorkingColors", async (data) => {
		await tick();

		update((state) => {
			patchLayout(state.workingColorsLayout, data);
			return state;
		});
	});

	subscriptions.subscribeLayoutUpdate("NodeGraphControlBar", async (data) => {
		await tick();

		update((state) => {
			patchLayout(state.nodeGraphControlBarLayout, data);
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateGraphViewOverlay", (data) => {
		update((state) => {
			state.graphViewOverlayOpen = data.open;
			return state;
		});
	});

	return { subscribe };
}

export function destroyDocumentStore() {
	const subscriptions = subscriptionsRef;
	if (!subscriptions) return;

	subscriptions.unsubscribeFrontendMessage("UpdateGraphFadeArtwork");
	subscriptions.unsubscribeFrontendMessage("UpdateGraphViewOverlay");
	subscriptions.unsubscribeLayoutUpdate("ToolOptions");
	subscriptions.unsubscribeLayoutUpdate("DocumentBar");
	subscriptions.unsubscribeLayoutUpdate("ToolShelf");
	subscriptions.unsubscribeLayoutUpdate("WorkingColors");
	subscriptions.unsubscribeLayoutUpdate("NodeGraphControlBar");
}

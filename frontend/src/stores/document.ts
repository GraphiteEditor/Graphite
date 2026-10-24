import { tick } from "svelte";
import { get, writable } from "svelte/store";
import type { Writable } from "svelte/store";
import type { SubscriptionsRouter } from "/src/subscriptions-router";
import { createLayout } from "/src/utility-functions/layout-state.svelte";
import { patchLayout } from "/src/utility-functions/widgets";
import type { Layout, RenderStatus } from "/wrapper/pkg/graphite_wasm_wrapper";

export type DocumentStore = ReturnType<typeof createDocumentStore>;

type DocumentStoreState = {
	toolOptionsLayout: Layout;
	documentBarLayout: Layout;
	toolShelfLayout: Layout;
	workingColorsLayout: Layout;
	nodeGraphControlBarLayout: Layout;
	graphViewOverlayOpen: boolean;
	fadeArtwork: number;
	renderStatus: RenderStatus;
};
const initialState: DocumentStoreState = {
	toolOptionsLayout: createLayout(),
	documentBarLayout: createLayout(),
	toolShelfLayout: createLayout(),
	workingColorsLayout: createLayout(),
	nodeGraphControlBarLayout: createLayout(),
	graphViewOverlayOpen: false,
	fadeArtwork: 100,
	renderStatus: "Rendering",
};

let subscriptionsRouter: SubscriptionsRouter | undefined = undefined;

// Store state persisted across HMR to maintain reactive subscriptions in the component tree
const store: Writable<DocumentStoreState> = import.meta.hot?.data?.store || writable<DocumentStoreState>(initialState);
if (import.meta.hot) import.meta.hot.data.store = store;
const { subscribe, update } = store;

export function createDocumentStore(subscriptions: SubscriptionsRouter) {
	destroyDocumentStore();

	subscriptionsRouter = subscriptions;

	subscriptions.subscribeFrontendMessage("UpdateGraphFadeArtwork", (data) => {
		update((state) => {
			state.fadeArtwork = data.percentage;
			return state;
		});
	});

	subscriptions.subscribeLayoutUpdate("ToolOptions", async (data) => {
		await tick();

		patchLayout(get(store).toolOptionsLayout, data);
	});

	subscriptions.subscribeLayoutUpdate("DocumentBar", async (data) => {
		await tick();

		patchLayout(get(store).documentBarLayout, data);
	});

	subscriptions.subscribeLayoutUpdate("ToolShelf", async (data) => {
		await tick();

		patchLayout(get(store).toolShelfLayout, data);
	});

	subscriptions.subscribeLayoutUpdate("WorkingColors", async (data) => {
		await tick();

		patchLayout(get(store).workingColorsLayout, data);
	});

	subscriptions.subscribeLayoutUpdate("NodeGraphControlBar", async (data) => {
		await tick();

		patchLayout(get(store).nodeGraphControlBarLayout, data);
	});

	subscriptions.subscribeFrontendMessage("UpdateGraphViewOverlay", (data) => {
		update((state) => {
			state.graphViewOverlayOpen = data.open;
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateRenderStatus", (data) => {
		update((state) => {
			state.renderStatus = data.status;
			return state;
		});
	});

	return { subscribe };
}

export function destroyDocumentStore() {
	const subscriptions = subscriptionsRouter;
	if (!subscriptions) return;

	subscriptions.unsubscribeFrontendMessage("UpdateGraphFadeArtwork");
	subscriptions.unsubscribeFrontendMessage("UpdateGraphViewOverlay");
	subscriptions.unsubscribeFrontendMessage("UpdateRenderStatus");
	subscriptions.unsubscribeLayoutUpdate("ToolOptions");
	subscriptions.unsubscribeLayoutUpdate("DocumentBar");
	subscriptions.unsubscribeLayoutUpdate("ToolShelf");
	subscriptions.unsubscribeLayoutUpdate("WorkingColors");
	subscriptions.unsubscribeLayoutUpdate("NodeGraphControlBar");
}

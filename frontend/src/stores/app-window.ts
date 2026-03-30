import { writable } from "svelte/store";
import type { Writable } from "svelte/store";
import type { SubscriptionsRouter } from "/src/subscriptions-router";
import type { AppWindowPlatform } from "/wrapper/pkg/graphite_wasm_wrapper";

export type AppWindowStore = ReturnType<typeof createAppWindowStore>;

type AppWindowStoreState = {
	platform: AppWindowPlatform;
	maximized: boolean;
	fullscreen: boolean;
	viewportHolePunch: boolean;
	uiScale: number;
};
const initialState: AppWindowStoreState = {
	platform: "Web",
	maximized: false,
	fullscreen: false,
	viewportHolePunch: false,
	uiScale: 1,
};

let subscriptionsRouter: SubscriptionsRouter | undefined = undefined;

// Store state persisted across HMR to maintain reactive subscriptions in the component tree
const store: Writable<AppWindowStoreState> = import.meta.hot?.data?.store || writable<AppWindowStoreState>(initialState);
if (import.meta.hot) import.meta.hot.data.store = store;
const { subscribe, update } = store;

export function createAppWindowStore(subscriptions: SubscriptionsRouter) {
	destroyAppWindowStore();

	subscriptionsRouter = subscriptions;

	subscriptions.subscribeFrontendMessage("UpdatePlatform", (data) => {
		update((state) => {
			state.platform = data.platform;
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateMaximized", (data) => {
		update((state) => {
			state.maximized = data.maximized;
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateFullscreen", (data) => {
		update((state) => {
			state.fullscreen = data.fullscreen;
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateViewportHolePunch", (data) => {
		update((state) => {
			state.viewportHolePunch = data.active;
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateUIScale", (data) => {
		update((state) => {
			state.uiScale = data.scale;
			return state;
		});
	});

	return { subscribe };
}

export function destroyAppWindowStore() {
	const subscriptions = subscriptionsRouter;
	if (!subscriptions) return;

	subscriptions.unsubscribeFrontendMessage("UpdatePlatform");
	subscriptions.unsubscribeFrontendMessage("UpdateMaximized");
	subscriptions.unsubscribeFrontendMessage("UpdateFullscreen");
	subscriptions.unsubscribeFrontendMessage("UpdateViewportHolePunch");
	subscriptions.unsubscribeFrontendMessage("UpdateUIScale");
}

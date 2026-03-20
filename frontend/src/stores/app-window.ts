import { writable } from "svelte/store";
import type { Writable } from "svelte/store";

import type { AppWindowPlatform } from "@graphite/../wasm/pkg/graphite_wasm";
import type { SubscriptionRouter } from "@graphite/subscription-router";

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

let subscriptionsRef: SubscriptionRouter | undefined = undefined;

// Store state persisted across HMR to maintain reactive subscriptions in the component tree
const store: Writable<AppWindowStoreState> = import.meta.hot?.data?.store || writable<AppWindowStoreState>(initialState);
if (import.meta.hot) import.meta.hot.data.store = store;
const { subscribe, update } = store;

export function createAppWindowStore(subscriptions: SubscriptionRouter) {
	destroyAppWindowStore();

	subscriptionsRef = subscriptions;

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
	const subscriptions = subscriptionsRef;
	if (!subscriptions) return;

	subscriptions.unsubscribeFrontendMessage("UpdatePlatform");
	subscriptions.unsubscribeFrontendMessage("UpdateMaximized");
	subscriptions.unsubscribeFrontendMessage("UpdateFullscreen");
	subscriptions.unsubscribeFrontendMessage("UpdateViewportHolePunch");
	subscriptions.unsubscribeFrontendMessage("UpdateUIScale");
}

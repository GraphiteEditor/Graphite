import { writable } from "svelte/store";
import type { Writable } from "svelte/store";

import type { AppWindowPlatform } from "@graphite/../wasm/pkg/graphite_wasm";
import type { Editor } from "@graphite/editor";

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

// Store state persisted across HMR to maintain reactive subscriptions in the component tree
const store: Writable<AppWindowStoreState> = import.meta.hot?.data?.store || writable<AppWindowStoreState>(initialState);
if (import.meta.hot) import.meta.hot.data.store = store;
const { subscribe, update } = store;

export function createAppWindowStore(editor: Editor) {
	// Set up message subscriptions on creation
	editor.subscriptions.subscribeFrontendMessage("UpdatePlatform", (data) => {
		update((state) => {
			state.platform = data.platform;
			return state;
		});
	});
	editor.subscriptions.subscribeFrontendMessage("UpdateMaximized", (data) => {
		update((state) => {
			state.maximized = data.maximized;
			return state;
		});
	});
	editor.subscriptions.subscribeFrontendMessage("UpdateFullscreen", (data) => {
		update((state) => {
			state.fullscreen = data.fullscreen;
			return state;
		});
	});
	editor.subscriptions.subscribeFrontendMessage("UpdateViewportHolePunch", (data) => {
		update((state) => {
			state.viewportHolePunch = data.active;
			return state;
		});
	});
	editor.subscriptions.subscribeFrontendMessage("UpdateUIScale", (data) => {
		update((state) => {
			state.uiScale = data.scale;
			return state;
		});
	});

	function destroy() {
		editor.subscriptions.unsubscribeFrontendMessage("UpdatePlatform");
		editor.subscriptions.unsubscribeFrontendMessage("UpdateMaximized");
		editor.subscriptions.unsubscribeFrontendMessage("UpdateFullscreen");
		editor.subscriptions.unsubscribeFrontendMessage("UpdateViewportHolePunch");
		editor.subscriptions.unsubscribeFrontendMessage("UpdateUIScale");
	}

	currentCleanup = destroy;
	currentArgs = [editor];
	return {
		subscribe,
		destroy,
	};
}
export type AppWindowStore = ReturnType<typeof createAppWindowStore>;

// Self-accepting HMR: tear down the old instance and re-create with the new module's code
let currentCleanup: (() => void) | undefined;
let currentArgs: [Editor] | undefined;
import.meta.hot?.accept((newModule) => {
	currentCleanup?.();
	if (currentArgs) newModule?.createAppWindowStore(...currentArgs);
});

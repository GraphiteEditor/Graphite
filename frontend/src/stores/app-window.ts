import { writable } from "svelte/store";
import type { Writable } from "svelte/store";

import type { AppWindowPlatform } from "@graphite/../wasm/pkg/graphite_wasm";
import type { Editor } from "@graphite/editor";

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

let editorRef: Editor | undefined = undefined;

// Store state persisted across HMR to maintain reactive subscriptions in the component tree
const store: Writable<AppWindowStoreState> = import.meta.hot?.data?.store || writable<AppWindowStoreState>(initialState);
if (import.meta.hot) import.meta.hot.data.store = store;
const { subscribe, update } = store;

export function createAppWindowStore(editor: Editor) {
	editorRef = editor;

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

	return { subscribe };
}

export function destroyAppWindowStore() {
	const editor = editorRef;
	if (!editor) return;

	editor.subscriptions.unsubscribeFrontendMessage("UpdatePlatform");
	editor.subscriptions.unsubscribeFrontendMessage("UpdateMaximized");
	editor.subscriptions.unsubscribeFrontendMessage("UpdateFullscreen");
	editor.subscriptions.unsubscribeFrontendMessage("UpdateViewportHolePunch");
	editor.subscriptions.unsubscribeFrontendMessage("UpdateUIScale");
}

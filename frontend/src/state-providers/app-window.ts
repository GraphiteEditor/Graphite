import { writable } from "svelte/store";

import type { AppWindowPlatform } from "@graphite/../wasm/pkg/graphite_wasm";
import type { Editor } from "@graphite/editor";

export function createAppWindowState(editor: Editor) {
	const { subscribe, update } = writable<{
		platform: AppWindowPlatform;
		maximized: boolean;
		fullscreen: boolean;
		viewportHolePunch: boolean;
		uiScale: number;
	}>({
		platform: "Web",
		maximized: false,
		fullscreen: false,
		viewportHolePunch: false,
		uiScale: 1,
	});

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

	return {
		subscribe,
		destroy,
	};
}
export type AppWindowState = ReturnType<typeof createAppWindowState>;

// This store is bound to the component tree via setContext() and can't be hot-replaced, so we force a full page reload
import.meta.hot?.accept(() => location.reload());

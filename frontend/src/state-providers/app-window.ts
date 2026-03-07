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

	return {
		subscribe,
	};
}
export type AppWindowState = ReturnType<typeof createAppWindowState>;

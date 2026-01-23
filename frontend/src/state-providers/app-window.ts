import { writable } from "svelte/store";

import { type Editor } from "@graphite/editor";
import { type AppWindowPlatform, UpdatePlatform, UpdateViewportHolePunch, UpdateMaximized, UpdateFullscreen, UpdateUIScale } from "@graphite/messages";

export function createAppWindowState(editor: Editor) {
	const { subscribe, update } = writable({
		platform: "Web" as AppWindowPlatform,
		maximized: false,
		fullscreen: false,
		viewportHolePunch: false,
		uiScale: 1,
	});

	// Set up message subscriptions on creation
	editor.subscriptions.subscribeJsMessage(UpdatePlatform, (data) => {
		update((state) => {
			state.platform = data.platform;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateMaximized, (data) => {
		update((state) => {
			state.maximized = data.maximized;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateFullscreen, (data) => {
		update((state) => {
			state.fullscreen = data.fullscreen;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateViewportHolePunch, (data) => {
		update((state) => {
			state.viewportHolePunch = data.active;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateUIScale, (data) => {
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

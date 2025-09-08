import { writable } from "svelte/store";

import { type Editor } from "@graphite/editor";
import { type AppWindowPlatform, UpdatePlatform, UpdateViewportHolePunch, UpdateWindowState } from "@graphite/messages";

export function createAppWindowState(editor: Editor) {
	const { subscribe, update } = writable({
		platform: "Web" as AppWindowPlatform,
		maximized: false,
		viewportHolePunch: false,
	});

	// Set up message subscriptions on creation
	editor.subscriptions.subscribeJsMessage(UpdatePlatform, (updatePlatform) => {
		update((state) => {
			state.platform = updatePlatform.platform;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateWindowState, (updateWindowState) => {
		update((state) => {
			state.maximized = updateWindowState.maximized;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateViewportHolePunch, (viewportHolePunch) => {
		update((state) => {
			state.viewportHolePunch = viewportHolePunch.active;
			return state;
		});
	});

	return {
		subscribe,
	};
}
export type AppWindowState = ReturnType<typeof createAppWindowState>;

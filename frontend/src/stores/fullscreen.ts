import { get, writable } from "svelte/store";
import type { Writable } from "svelte/store";

import type { Editor } from "@graphite/editor";

type FullscreenStoreState = {
	windowFullscreen: boolean;
	keyboardLocked: boolean;
};
const initialState: FullscreenStoreState = {
	windowFullscreen: false,
	keyboardLocked: false,
};

// Store state persisted across HMR to maintain reactive subscriptions in the component tree
const store: Writable<FullscreenStoreState> = import.meta.hot?.data?.store || writable<FullscreenStoreState>(initialState);
if (import.meta.hot) import.meta.hot.data.store = store;
const { subscribe, update } = store;

export function createFullscreenStore(editor: Editor) {
	editor.subscriptions.subscribeFrontendMessage("WindowFullscreen", () => {
		toggleFullscreen();
	});

	function destroy() {
		editor.subscriptions.unsubscribeFrontendMessage("WindowFullscreen");
	}

	currentCleanup = destroy;
	currentArgs = [editor];
	return {
		subscribe,
		destroy,
	};
}
export type FullscreenStore = ReturnType<typeof createFullscreenStore>;

export function fullscreenModeChanged() {
	update((state) => {
		state.windowFullscreen = Boolean(document.fullscreenElement);
		if (!state.windowFullscreen) state.keyboardLocked = false;
		return state;
	});
}

export async function enterFullscreen() {
	await document.documentElement.requestFullscreen();

	const keyboardLockApiSupported = navigator.keyboard !== undefined && "lock" in navigator.keyboard;

	if (keyboardLockApiSupported && navigator.keyboard) {
		await navigator.keyboard.lock(["ControlLeft", "ControlRight"]);

		update((state) => {
			state.keyboardLocked = true;
			return state;
		});
	}
}

export async function exitFullscreen() {
	await document.exitFullscreen();
}

export async function toggleFullscreen() {
	const state = get(store);
	if (state.windowFullscreen) await exitFullscreen();
	else await enterFullscreen();
}

// Self-accepting HMR: tear down the old instance and re-create with the new module's code
let currentCleanup: (() => void) | undefined;
let currentArgs: [Editor] | undefined;
import.meta.hot?.accept((newModule) => {
	currentCleanup?.();
	if (currentArgs) newModule?.createFullscreenStore(...currentArgs);
});

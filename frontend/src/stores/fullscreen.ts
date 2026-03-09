import { writable } from "svelte/store";

import type { Editor } from "@graphite/editor";

export function createFullscreenState(editor: Editor) {
	// Experimental Keyboard API: https://developer.mozilla.org/en-US/docs/Web/API/Navigator/keyboard
	const keyboardLockApiSupported: Readonly<boolean> = navigator.keyboard !== undefined && "lock" in navigator.keyboard;

	const { subscribe, update } = writable({
		windowFullscreen: false,
		keyboardLocked: false,
		keyboardLockApiSupported,
	});

	function fullscreenModeChanged() {
		update((state) => {
			state.windowFullscreen = Boolean(document.fullscreenElement);
			if (!state.windowFullscreen) state.keyboardLocked = false;
			return state;
		});
	}

	async function enterFullscreen() {
		await document.documentElement.requestFullscreen();

		if (keyboardLockApiSupported && navigator.keyboard) {
			await navigator.keyboard.lock(["ControlLeft", "ControlRight"]);

			update((state) => {
				state.keyboardLocked = true;
				return state;
			});
		}
	}

	async function exitFullscreen() {
		await document.exitFullscreen();
	}

	async function toggleFullscreen() {
		return new Promise((resolve, reject) => {
			update((state) => {
				if (state.windowFullscreen) exitFullscreen().then(resolve).catch(reject);
				else enterFullscreen().then(resolve).catch(reject);

				return state;
			});
		});
	}

	editor.subscriptions.subscribeFrontendMessage("WindowFullscreen", () => {
		toggleFullscreen();
	});

	function destroy() {
		editor.subscriptions.unsubscribeFrontendMessage("WindowFullscreen");
	}

	return {
		subscribe,
		fullscreenModeChanged,
		enterFullscreen,
		exitFullscreen,
		toggleFullscreen,
		destroy,
	};
}
export type FullscreenState = ReturnType<typeof createFullscreenState>;

// This store is bound to the component tree via setContext() and can't be hot-replaced, so we force a full page reload
import.meta.hot?.accept(() => location.reload());

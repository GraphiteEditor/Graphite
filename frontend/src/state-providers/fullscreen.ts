import { writable } from "svelte/store";

import { type Editor } from "@graphite/editor";

export function createFullscreenState(_: Editor) {
	// Experimental Keyboard API: https://developer.mozilla.org/en-US/docs/Web/API/Navigator/keyboard
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	const keyboardLockApiSupported: Readonly<boolean> = "keyboard" in navigator && (navigator as any).keyboard && "lock" in (navigator as any).keyboard;

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

		if (keyboardLockApiSupported) {
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			await (navigator as any).keyboard.lock(["ControlLeft", "ControlRight"]);

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

	return {
		subscribe,
		fullscreenModeChanged,
		enterFullscreen,
		exitFullscreen,
		toggleFullscreen,
	};
}
export type FullscreenState = ReturnType<typeof createFullscreenState>;

import { reactive, readonly } from "vue";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createFullscreenState() {
	const state = reactive({
		windowFullscreen: false,
		keyboardLocked: false,
	});

	function fullscreenModeChanged(): void {
		state.windowFullscreen = Boolean(document.fullscreenElement);
		if (!state.windowFullscreen) state.keyboardLocked = false;
	}

	async function enterFullscreen(): Promise<void> {
		await document.documentElement.requestFullscreen();

		if (keyboardLockApiSupported) {
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			await (navigator as any).keyboard.lock(["ControlLeft", "ControlRight"]);
			state.keyboardLocked = true;
		}
	}

	async function exitFullscreen(): Promise<void> {
		await document.exitFullscreen();
	}

	async function toggleFullscreen(): Promise<void> {
		if (state.windowFullscreen) await exitFullscreen();
		else await enterFullscreen();
	}

	// Experimental Keyboard API: https://developer.mozilla.org/en-US/docs/Web/API/Navigator/keyboard
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	const keyboardLockApiSupported: Readonly<boolean> = "keyboard" in navigator && (navigator as any).keyboard && "lock" in (navigator as any).keyboard;

	return {
		state: readonly(state) as typeof state,
		fullscreenModeChanged,
		enterFullscreen,
		exitFullscreen,
		toggleFullscreen,
		keyboardLockApiSupported,
	};
}
export type FullscreenState = ReturnType<typeof createFullscreenState>;

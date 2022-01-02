import { reactive, readonly } from "vue";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createFullscreenState() {
	const state = reactive({
		windowFullscreen: false,
		keyboardLocked: false,
	});

	const fullscreenModeChanged = (): void => {
		state.windowFullscreen = Boolean(document.fullscreenElement);
		if (!state.windowFullscreen) state.keyboardLocked = false;
	};

	// Experimental Keyboard API: https://developer.mozilla.org/en-US/docs/Web/API/Navigator/keyboard
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	const keyboardLockApiSupported: Readonly<boolean> = "keyboard" in navigator && "lock" in (navigator as any).keyboard;

	const enterFullscreen = async (): Promise<void> => {
		await document.documentElement.requestFullscreen();

		if (keyboardLockApiSupported) {
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			await (navigator as any).keyboard.lock(["ControlLeft", "ControlRight"]);
			state.keyboardLocked = true;
		}
	};

	// eslint-disable-next-line class-methods-use-this
	const exitFullscreen = async (): Promise<void> => {
		await document.exitFullscreen();
	};

	const toggleFullscreen = async (): Promise<void> => {
		if (state.windowFullscreen) await exitFullscreen();
		else await enterFullscreen();
	};

	return {
		state: readonly(state),
		keyboardLockApiSupported,
		enterFullscreen,
		exitFullscreen,
		toggleFullscreen,
		fullscreenModeChanged,
	};
}
export type FullscreenState = ReturnType<typeof createFullscreenState>;

import { reactive, readonly } from "vue";

export function createFullscreenState() {
	const state = reactive({
		windowFullscreen: false,
		keyboardLocked: false,
	});

	const fullscreenModeChanged = () => {
		state.windowFullscreen = Boolean(document.fullscreenElement);
		if (!state.windowFullscreen) state.keyboardLocked = false;
	};

	// Experimental Keyboard API: https://developer.mozilla.org/en-US/docs/Web/API/Navigator/keyboard
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	const keyboardLockApiSupported: Readonly<boolean> = "keyboard" in navigator && "lock" in (navigator as any).keyboard;

	const enterFullscreen = async () => {
		await document.documentElement.requestFullscreen();

		if (keyboardLockApiSupported) {
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			await (navigator as any).keyboard.lock(["ControlLeft", "ControlRight"]);
			state.keyboardLocked = true;
		}
	};

	// eslint-disable-next-line class-methods-use-this
	const exitFullscreen = async () => {
		await document.exitFullscreen();
	};

	const toggleFullscreen = async () => {
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

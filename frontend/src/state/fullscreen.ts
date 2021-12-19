import { reactive, readonly } from "vue";

export function createFullscreenState() {
	const state = reactive({
		keyboardLocked: false,
	});

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
		if (isFullscreen()) await exitFullscreen();
		else await enterFullscreen();
	};

	const isFullscreen = (): boolean => {
		return Boolean(document.fullscreenElement);
	};

	const isKeyboardLocked = (): boolean => {
		return state.keyboardLocked;
	};

	return {
		state: readonly(state),
		keyboardLockApiSupported,
		enterFullscreen,
		exitFullscreen,
		toggleFullscreen,
		isFullscreen,
		isKeyboardLocked,
	};
}
export type FullscreenState = ReturnType<typeof createFullscreenState>;

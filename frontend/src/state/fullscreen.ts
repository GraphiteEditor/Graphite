import { reactive, readonly } from "vue";

export type FullscreenState = ReturnType<typeof makeFullscreenState>;
export default function makeFullscreenState() {
	const state = reactive({
		windowFullscreen: false,
		keyboardLocked: false,
	});

	function fullscreenModeChanged() {
		state.windowFullscreen = Boolean(document.fullscreenElement);
		if (!state.windowFullscreen) state.keyboardLocked = false;
	}

	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	const keyboardLockApiSupported = "keyboard" in navigator && "lock" in (navigator as any).keyboard;

	async function enterFullscreen() {
		await document.documentElement.requestFullscreen();

		if (keyboardLockApiSupported) {
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			await (navigator as any).keyboard.lock(["ControlLeft", "ControlRight"]);
			state.keyboardLocked = true;
		}
	}

	async function exitFullscreen() {
		await document.exitFullscreen();
	}

	async function toggleFullscreen() {
		if (state.windowFullscreen) await exitFullscreen();
		else await enterFullscreen();
	}

	return {
		state: readonly(state),
		fullscreenModeChanged,
		keyboardLockApiSupported,
		enterFullscreen,
		exitFullscreen,
		toggleFullscreen,
	};
}

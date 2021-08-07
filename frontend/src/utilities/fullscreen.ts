import { reactive, readonly } from "vue";

const state = reactive({
	windowFullscreen: false,
	keyboardLocked: false,
});

export function fullscreenModeChanged() {
	state.windowFullscreen = Boolean(document.fullscreenElement);
	if (!state.windowFullscreen) state.keyboardLocked = false;
}

export function keyboardLockApiSupported(): boolean {
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	return "keyboard" in navigator && "lock" in (navigator as any).keyboard;
}

export async function enterFullscreen() {
	await document.documentElement.requestFullscreen();

	if (keyboardLockApiSupported()) {
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		await (navigator as any).keyboard.lock(["ControlLeft", "ControlRight"]);
		state.keyboardLocked = true;
	}
}

export async function exitFullscreen() {
	await document.exitFullscreen();
}

export async function toggleFullscreen() {
	if (state.windowFullscreen) await exitFullscreen();
	else await enterFullscreen();
}

export default readonly(state);

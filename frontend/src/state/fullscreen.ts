import { reactive } from "vue";

export class FullscreenState {
	private state = reactive({
		windowFullscreen: false,
		keyboardLocked: false,
	});

	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	readonly keyboardLockApiSupported = "keyboard" in navigator && "lock" in (navigator as any).keyboard;

	fullscreenModeChanged() {
		this.state.windowFullscreen = Boolean(document.fullscreenElement);
		if (!this.state.windowFullscreen) this.state.keyboardLocked = false;
	}

	async enterFullscreen() {
		await document.documentElement.requestFullscreen();

		if (this.keyboardLockApiSupported) {
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			await (navigator as any).keyboard.lock(["ControlLeft", "ControlRight"]);
			this.state.keyboardLocked = true;
		}
	}

	isFullscreen(): boolean {
		return this.state.windowFullscreen;
	}

	isKeyboardLocked(): boolean {
		return this.state.keyboardLocked;
	}

	// eslint-disable-next-line class-methods-use-this
	async exitFullscreen() {
		await document.exitFullscreen();
	}

	async toggleFullscreen() {
		if (this.state.windowFullscreen) await this.exitFullscreen();
		else await this.enterFullscreen();
	}
}

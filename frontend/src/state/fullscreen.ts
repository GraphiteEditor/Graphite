import { reactive } from "vue";

export class FullscreenState {
	private state = reactive({
		keyboardLocked: false,
	});

	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	readonly keyboardLockApiSupported = "keyboard" in navigator && "lock" in (navigator as any).keyboard;

	async enterFullscreen() {
		await document.documentElement.requestFullscreen();

		if (this.keyboardLockApiSupported) {
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			await (navigator as any).keyboard.lock(["ControlLeft", "ControlRight"]);
			this.state.keyboardLocked = true;
		}
	}

	// eslint-disable-next-line class-methods-use-this
	isFullscreen(): boolean {
		return Boolean(document.fullscreenElement);
	}

	isKeyboardLocked(): boolean {
		return this.state.keyboardLocked;
	}

	// eslint-disable-next-line class-methods-use-this
	async exitFullscreen() {
		await document.exitFullscreen();
	}

	async toggleFullscreen() {
		if (this.isFullscreen()) await this.exitFullscreen();
		else await this.enterFullscreen();
	}
}

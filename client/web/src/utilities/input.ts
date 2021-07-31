import { toggleFullscreen } from "@/utilities/fullscreen";
import { dialogIsVisible, dismissDialog } from "@/utilities/dialog";

const wasm = import("@/../wasm/pkg");

export function shouldRedirectKeyboardEventToBackend(e: KeyboardEvent): boolean {
	// Don't redirect user input from text entry into HTML elements
	const target = e.target as HTMLElement;
	if (target.nodeName === "INPUT" || target.nodeName === "TEXTAREA" || target.isContentEditable) return false;

	// Don't redirect when a modal is covering the workspace
	if (dialogIsVisible()) return false;

	// Don't redirect a fullscreen request
	if (e.key.toLowerCase() === "f11" && e.type === "keydown" && !e.repeat) {
		e.preventDefault();
		toggleFullscreen();
		return false;
	}

	// Don't redirect a reload request
	if (e.key.toLowerCase() === "f5") return false;

	// Don't redirect debugging tools
	if (e.key.toLowerCase() === "f12") return false;
	if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === "c") return false;
	if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === "i") return false;
	if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === "j") return false;

	// Redirect to the backend
	return true;
}

export async function handleKeyDown(e: KeyboardEvent) {
	if (shouldRedirectKeyboardEventToBackend(e)) {
		e.preventDefault();
		const { on_key_down } = await wasm;
		const modifiers = makeModifiersBitfield(e.ctrlKey, e.shiftKey, e.altKey);
		on_key_down(e.key, modifiers);
		return;
	}

	if (e.key === "Escape" && dialogIsVisible()) dismissDialog();
}

export async function handleKeyUp(e: KeyboardEvent) {
	if (shouldRedirectKeyboardEventToBackend(e)) {
		e.preventDefault();
		const { on_key_up } = await wasm;
		const modifiers = makeModifiersBitfield(e.ctrlKey, e.shiftKey, e.altKey);
		on_key_up(e.key, modifiers);
	}
}

export async function handleMouseDown(e: MouseEvent) {
	if (dialogIsVisible() && e.target && !(e.target as HTMLElement).closest(".dialog-modal .floating-menu-content")) {
		dismissDialog();

		e.preventDefault();
		e.stopPropagation();
	}
}

export function makeModifiersBitfield(control: boolean, shift: boolean, alt: boolean): number {
	// eslint-disable-next-line no-bitwise
	return Number(control) | (Number(shift) << 1) | (Number(alt) << 2);
}

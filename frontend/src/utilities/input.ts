import { toggleFullscreen } from "@/utilities/fullscreen";
import { dialogIsVisible, dismissDialog, submitDialog } from "@/utilities/dialog";
import { panicProxy } from "@/utilities/panic-proxy";

const wasm = import("@/../wasm/pkg").then(panicProxy);

let viewportMouseInteractionOngoing = false;
let editingTextField: HTMLTextAreaElement | undefined;

// Keyboard events

function shouldRedirectKeyboardEventToBackend(e: KeyboardEvent): boolean {
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

export async function onKeyDown(e: KeyboardEvent) {
	if (shouldRedirectKeyboardEventToBackend(e)) {
		e.preventDefault();
		const modifiers = makeModifiersBitfield(e);
		(await wasm).on_key_down(e.key, modifiers);
		return;
	}

	if (dialogIsVisible()) {
		if (e.key === "Escape") dismissDialog();
		if (e.key === "Enter") {
			submitDialog();

			// Prevent the Enter key from acting like a click on the last clicked button, which might reopen the dialog
			e.preventDefault();
		}
	}
}

export async function onKeyUp(e: KeyboardEvent) {
	if (shouldRedirectKeyboardEventToBackend(e)) {
		e.preventDefault();
		const modifiers = makeModifiersBitfield(e);
		(await wasm).on_key_up(e.key, modifiers);
	}
}

// Mouse events

export async function onMouseMove(e: MouseEvent) {
	if (!e.buttons) viewportMouseInteractionOngoing = false;

	const modifiers = makeModifiersBitfield(e);
	(await wasm).on_mouse_move(e.clientX, e.clientY, e.buttons, modifiers);
}

export async function onMouseDown(e: MouseEvent) {
	const target = e.target && (e.target as HTMLElement);
	const inCanvas = target && target.closest(".canvas");
	const inDialog = target && target.closest(".dialog-modal .floating-menu-content");

	// Block middle mouse button auto-scroll mode
	if (e.button === 1) e.preventDefault();

	if (dialogIsVisible() && !inDialog) {
		dismissDialog();
		e.preventDefault();
		e.stopPropagation();
	}

	if (inCanvas) {
		if (target.nodeName === "TEXTAREA") {
			editingTextField = target as HTMLTextAreaElement;
		} else if (editingTextField) {
			if (editingTextField.dataset.path) {
				(await wasm).on_input_changed(editingTextField.dataset.path, editingTextField.value);
			} else {
				console.error("Edited text had not path attribute");
			}
			editingTextField = undefined;
		} else viewportMouseInteractionOngoing = true;
	}

	if (viewportMouseInteractionOngoing) {
		const modifiers = makeModifiersBitfield(e);
		(await wasm).on_mouse_down(e.clientX, e.clientY, e.buttons, modifiers);
	}
}

export async function onMouseUp(e: MouseEvent) {
	if (!e.buttons) viewportMouseInteractionOngoing = false;

	const modifiers = makeModifiersBitfield(e);
	(await wasm).on_mouse_up(e.clientX, e.clientY, e.buttons, modifiers);
}

export async function onMouseScroll(e: WheelEvent) {
	const target = e.target && (e.target as HTMLElement);
	const inCanvas = target && target.closest(".canvas");

	if (inCanvas) {
		e.preventDefault();
		const modifiers = makeModifiersBitfield(e);
		(await wasm).on_mouse_scroll(e.clientX, e.clientY, e.buttons, e.deltaX, e.deltaY, e.deltaZ, modifiers);
	}
}

export async function onWindowResize() {
	const viewports = Array.from(document.querySelectorAll(".canvas"));
	const boundsOfViewports = viewports.map((canvas) => {
		const bounds = canvas.getBoundingClientRect();
		return [bounds.left, bounds.top, bounds.right, bounds.bottom];
	});

	const flattened = boundsOfViewports.flat();
	const data = Float64Array.from(flattened);

	if (boundsOfViewports.length > 0) (await wasm).bounds_of_viewports(data);
}

export function makeModifiersBitfield(e: MouseEvent | KeyboardEvent): number {
	return Number(e.ctrlKey) | (Number(e.shiftKey) << 1) | (Number(e.altKey) << 2);
}

import { DialogState } from "@/state/dialog";
import { FullscreenState } from "@/state/fullscreen";
import { EditorWasm } from "./wasm-loader";

let viewportMouseInteractionOngoing = false;

// Keyboard events

function shouldRedirectKeyboardEventToBackend(e: KeyboardEvent, fullscreenState: FullscreenState, dialogState: DialogState): boolean {
	// Don't redirect user input from text entry into HTML elements
	const target = e.target as HTMLElement;
	if (target.nodeName === "INPUT" || target.nodeName === "TEXTAREA" || target.isContentEditable) return false;

	// Don't redirect when a modal is covering the workspace
	if (dialogState.dialogIsVisible()) return false;

	// Don't redirect a fullscreen request
	if (e.key.toLowerCase() === "f11" && e.type === "keydown" && !e.repeat) {
		e.preventDefault();
		fullscreenState.toggleFullscreen();
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

function onKeyDown(editor: EditorWasm, fullscreenState: FullscreenState, dialogState: DialogState, e: KeyboardEvent) {
	if (shouldRedirectKeyboardEventToBackend(e, fullscreenState, dialogState)) {
		e.preventDefault();
		const modifiers = makeModifiersBitfield(e);
		editor.on_key_down(e.key, modifiers);
		return;
	}

	if (dialogState.dialogIsVisible()) {
		if (e.key === "Escape") dialogState.dismissDialog();
		if (e.key === "Enter") {
			dialogState.submitDialog();

			// Prevent the Enter key from acting like a click on the last clicked button, which might reopen the dialog
			e.preventDefault();
		}
	}
}

function onKeyUp(editor: EditorWasm, fullscreenState: FullscreenState, dialogState: DialogState, e: KeyboardEvent) {
	if (shouldRedirectKeyboardEventToBackend(e, fullscreenState, dialogState)) {
		e.preventDefault();
		const modifiers = makeModifiersBitfield(e);
		editor.on_key_up(e.key, modifiers);
	}
}

// Mouse events

function onMouseMove(editor: EditorWasm, e: MouseEvent) {
	if (!e.buttons) viewportMouseInteractionOngoing = false;

	const modifiers = makeModifiersBitfield(e);
	editor.on_mouse_move(e.clientX, e.clientY, e.buttons, modifiers);
}

function onMouseDown(editor: EditorWasm, dialogState: DialogState, e: MouseEvent) {
	const target = e.target && (e.target as HTMLElement);
	const inCanvas = target && target.closest(".canvas");
	const inDialog = target && target.closest(".dialog-modal .floating-menu-content");

	// Block middle mouse button auto-scroll mode
	if (e.button === 1) e.preventDefault();

	if (dialogState.dialogIsVisible() && !inDialog) {
		dialogState.dismissDialog();
		e.preventDefault();
		e.stopPropagation();
	}

	if (inCanvas) viewportMouseInteractionOngoing = true;

	if (viewportMouseInteractionOngoing) {
		const modifiers = makeModifiersBitfield(e);
		editor.on_mouse_down(e.clientX, e.clientY, e.buttons, modifiers);
	}
}

function onMouseUp(editor: EditorWasm, e: MouseEvent) {
	if (!e.buttons) viewportMouseInteractionOngoing = false;

	const modifiers = makeModifiersBitfield(e);
	editor.on_mouse_up(e.clientX, e.clientY, e.buttons, modifiers);
}

function onMouseScroll(editor: EditorWasm, e: WheelEvent) {
	const target = e.target && (e.target as HTMLElement);
	const inCanvas = target && target.closest(".canvas");

	if (inCanvas) {
		e.preventDefault();
		const modifiers = makeModifiersBitfield(e);
		editor.on_mouse_scroll(e.clientX, e.clientY, e.buttons, e.deltaX, e.deltaY, e.deltaZ, modifiers);
	}
}

function onWindowResize(editor: EditorWasm, container: Element) {
	const viewports = Array.from(container.querySelectorAll(".canvas"));
	const boundsOfViewports = viewports.map((canvas) => {
		const bounds = canvas.getBoundingClientRect();
		return [bounds.left, bounds.top, bounds.right, bounds.bottom];
	});

	const flattened = boundsOfViewports.flat();
	const data = Float64Array.from(flattened);

	if (boundsOfViewports.length > 0) editor.bounds_of_viewports(data);
}

function makeModifiersBitfield(e: MouseEvent | KeyboardEvent): number {
	return Number(e.ctrlKey) | (Number(e.shiftKey) << 1) | (Number(e.altKey) << 2);
}

// We need to keep a reference to any listener we add, otherwise we can't remove it.
const activeListeners = new WeakMap<EditorWasm, () => void>();

export function mountInput(editor: EditorWasm, container: HTMLElement, fullscreenState: FullscreenState, dialogState: DialogState) {
	const resize = () => onWindowResize(editor, container);
	const contextmenu = (e: MouseEvent) => e.preventDefault();
	const keyup = (e: KeyboardEvent) => onKeyUp(editor, fullscreenState, dialogState, e);
	const keydown = (e: KeyboardEvent) => onKeyDown(editor, fullscreenState, dialogState, e);
	const mousemove = (e: MouseEvent) => onMouseMove(editor, e);
	const mousedown = (e: MouseEvent) => onMouseDown(editor, dialogState, e);
	const mouseup = (e: MouseEvent) => onMouseUp(editor, e);
	const wheel = (e: WheelEvent) => onMouseScroll(editor, e);

	window.addEventListener("resize", resize);
	resize();

	container.addEventListener("contextmenu", contextmenu);

	container.addEventListener("keyup", keyup);
	container.addEventListener("keydown", keydown);

	window.addEventListener("mousemove", mousemove);
	container.addEventListener("mousedown", mousedown);
	window.addEventListener("mouseup", mouseup);

	container.addEventListener("wheel", wheel, { passive: false });

	activeListeners.set(editor, () => {
		window.removeEventListener("resize", resize);

		container.removeEventListener("contextmenu", contextmenu);

		container.removeEventListener("keyup", keyup);
		container.removeEventListener("keydown", keydown);

		window.removeEventListener("mousemove", mousemove);
		container.removeEventListener("mousedown", mousedown);
		window.removeEventListener("mouseup", mouseup);

		container.removeEventListener("wheel", wheel);
	});
}

export function unmountInput(editor: EditorWasm) {
	const cleanup = activeListeners.get(editor);
	if (!cleanup) return;
	activeListeners.delete(editor);

	cleanup();
}

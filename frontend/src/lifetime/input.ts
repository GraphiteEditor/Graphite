import { DialogState } from "@/state/dialog";
import { FullscreenState } from "@/state/fullscreen";
import { DocumentsState } from "@/state/documents";
import { EditorState } from "@/state/wasm-loader";

type EventName = keyof HTMLElementEventMap | keyof WindowEventHandlersEventMap;
interface EventListenerTarget {
	addEventListener: typeof window.addEventListener;
	removeEventListener: typeof window.removeEventListener;
}

export function createInputManager(editor: EditorState, container: HTMLElement, dialog: DialogState, document: DocumentsState, fullscreen: FullscreenState) {
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	const listeners: { target: EventListenerTarget; eventName: EventName; action: (event: any) => void; options?: boolean | AddEventListenerOptions }[] = [
		{ target: window, eventName: "resize", action: () => onWindowResize(container) },
		{ target: window, eventName: "beforeunload", action: (e) => onBeforeUnload(e) },
		{ target: window.document, eventName: "contextmenu", action: (e) => e.preventDefault() },
		{ target: window.document, eventName: "fullscreenchange", action: () => fullscreen.fullscreenModeChanged() },
		{ target: window, eventName: "keyup", action: (e) => onKeyUp(e) },
		{ target: window, eventName: "keydown", action: (e) => onKeyDown(e) },
		{ target: window, eventName: "pointermove", action: (e) => onPointerMove(e) },
		{ target: window, eventName: "pointerdown", action: (e) => onPointerDown(e) },
		{ target: window, eventName: "pointerup", action: (e) => onPointerUp(e) },
		{ target: window, eventName: "mousedown", action: (e) => onMouseDown(e) },
		{ target: window, eventName: "wheel", action: (e) => onMouseScroll(e), options: { passive: false } },
	];

	let viewportPointerInteractionOngoing = false;

	// Keyboard events

	const shouldRedirectKeyboardEventToBackend = (e: KeyboardEvent): boolean => {
		// Don't redirect user input from text entry into HTML elements
		const { target } = e;
		if (target instanceof HTMLElement && (target.nodeName === "INPUT" || target.nodeName === "TEXTAREA" || target.isContentEditable)) return false;

		// Don't redirect when a modal is covering the workspace
		if (dialog.dialogIsVisible()) return false;

		// Don't redirect a fullscreen request
		if (e.key.toLowerCase() === "f11" && e.type === "keydown" && !e.repeat) {
			e.preventDefault();
			fullscreen.toggleFullscreen();
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
	};

	const onKeyDown = (e: KeyboardEvent) => {
		if (shouldRedirectKeyboardEventToBackend(e)) {
			e.preventDefault();
			const modifiers = makeModifiersBitfield(e);
			editor.instance.on_key_down(e.key, modifiers);
			return;
		}

		if (dialog.dialogIsVisible()) {
			if (e.key === "Escape") dialog.dismissDialog();
			if (e.key === "Enter") {
				dialog.submitDialog();

				// Prevent the Enter key from acting like a click on the last clicked button, which might reopen the dialog
				e.preventDefault();
			}
		}
	};

	const onKeyUp = (e: KeyboardEvent) => {
		if (shouldRedirectKeyboardEventToBackend(e)) {
			e.preventDefault();
			const modifiers = makeModifiersBitfield(e);
			editor.instance.on_key_up(e.key, modifiers);
		}
	};

	// Pointer events

	const onPointerMove = (e: PointerEvent) => {
		if (!e.buttons) viewportPointerInteractionOngoing = false;

		const modifiers = makeModifiersBitfield(e);
		editor.instance.on_mouse_move(e.clientX, e.clientY, e.buttons, modifiers);
	};

	const onPointerDown = (e: PointerEvent) => {
		const { target } = e;
		const inCanvas = target instanceof Element && target.closest(".canvas");
		const inDialog = target instanceof Element && target.closest(".dialog-modal .floating-menu-content");

		if (dialog.dialogIsVisible() && !inDialog) {
			dialog.dismissDialog();
			e.preventDefault();
			e.stopPropagation();
		}

		if (inCanvas) viewportPointerInteractionOngoing = true;

		if (viewportPointerInteractionOngoing) {
			const modifiers = makeModifiersBitfield(e);
			editor.instance.on_mouse_down(e.clientX, e.clientY, e.buttons, modifiers);
		}
	};

	const onPointerUp = (e: PointerEvent) => {
		if (!e.buttons) viewportPointerInteractionOngoing = false;

		const modifiers = makeModifiersBitfield(e);
		editor.instance.on_mouse_up(e.clientX, e.clientY, e.buttons, modifiers);
	};

	// Mouse events

	const onMouseDown = (e: MouseEvent) => {
		// Block middle mouse button auto-scroll mode (the circlar widget that appears and allows quick scrolling by moving the cursor above or below it)
		// This has to be in `mousedown`, not `pointerdown`, to avoid blocking Vue's middle click detection on HTML elements
		if (e.button === 1) e.preventDefault();
	};

	const onMouseScroll = (e: WheelEvent) => {
		const { target } = e;
		const inCanvas = target instanceof Element && target.closest(".canvas");

		const horizontalScrollableElement = target instanceof Element && target.closest(".scrollable-x");
		if (horizontalScrollableElement && e.deltaY !== 0) {
			horizontalScrollableElement.scrollTo(horizontalScrollableElement.scrollLeft + e.deltaY, 0);
			return;
		}

		if (inCanvas) {
			e.preventDefault();
			const modifiers = makeModifiersBitfield(e);
			editor.instance.on_mouse_scroll(e.clientX, e.clientY, e.buttons, e.deltaX, e.deltaY, e.deltaZ, modifiers);
		}
	};

	// Window events

	const onWindowResize = (container: HTMLElement) => {
		const viewports = Array.from(container.querySelectorAll(".canvas"));
		const boundsOfViewports = viewports.map((canvas) => {
			const bounds = canvas.getBoundingClientRect();
			return [bounds.left, bounds.top, bounds.right, bounds.bottom];
		});

		const flattened = boundsOfViewports.flat();
		const data = Float64Array.from(flattened);

		if (boundsOfViewports.length > 0) editor.instance.bounds_of_viewports(data);
	};

	const onBeforeUnload = (e: BeforeUnloadEvent) => {
		const activeDocument = document.state.documents[document.state.activeDocumentIndex];
		if (!activeDocument.is_saved) editor.instance.trigger_auto_save(activeDocument.id);

		// Skip the message if the editor crashed, since work is already lost
		if (editor.instance.has_crashed()) return;

		// Skip the message during development, since it's annoying when testing
		if (process.env.NODE_ENV === "development") return;

		const allDocumentsSaved = document.state.documents.reduce((acc, doc) => acc && doc.is_saved, true);
		if (!allDocumentsSaved) {
			e.returnValue = "Unsaved work will be lost if the web browser tab is closed. Close anyway?";
			e.preventDefault();
		}
	};

	// Event bindings

	const addListeners = () => {
		listeners.forEach(({ target, eventName, action, options }) => target.addEventListener(eventName, action, options));
	};

	const removeListeners = () => {
		listeners.forEach(({ target, eventName, action }) => target.removeEventListener(eventName, action));
	};

	// Run on creation
	addListeners();
	onWindowResize(container);

	return {
		removeListeners,
	};
}
export type InputManager = ReturnType<typeof createInputManager>;

export function makeModifiersBitfield(e: WheelEvent | PointerEvent | KeyboardEvent): number {
	return Number(e.ctrlKey) | (Number(e.shiftKey) << 1) | (Number(e.altKey) << 2);
}

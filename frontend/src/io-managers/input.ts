import { DialogState } from "@/state-providers/dialog";
import { FullscreenState } from "@/state-providers/fullscreen";
import { PortfolioState } from "@/state-providers/portfolio";
import { makeKeyboardModifiersBitfield, textInputCleanup, getLatinKey } from "@/utility-functions/keyboard-entry";
import { Editor } from "@/wasm-communication/editor";

type EventName = keyof HTMLElementEventMap | keyof WindowEventHandlersEventMap | "modifyinputfield";
type EventListenerTarget = {
	addEventListener: typeof window.addEventListener;
	removeEventListener: typeof window.removeEventListener;
};

export function createInputManager(editor: Editor, container: HTMLElement, dialog: DialogState, document: PortfolioState, fullscreen: FullscreenState): () => void {
	const app = window.document.querySelector("[data-app]") as HTMLElement | undefined;
	app?.focus();

	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	const listeners: { target: EventListenerTarget; eventName: EventName; action: (event: any) => void; options?: boolean | AddEventListenerOptions }[] = [
		{ target: window, eventName: "resize", action: (): void => onWindowResize(container) },
		{ target: window, eventName: "beforeunload", action: (e: BeforeUnloadEvent): void => onBeforeUnload(e) },
		{ target: window.document, eventName: "contextmenu", action: (e: MouseEvent): void => e.preventDefault() },
		{ target: window.document, eventName: "fullscreenchange", action: (): void => fullscreen.fullscreenModeChanged() },
		{ target: window, eventName: "keyup", action: (e: KeyboardEvent): void => onKeyUp(e) },
		{ target: window, eventName: "keydown", action: (e: KeyboardEvent): void => onKeyDown(e) },
		{ target: window, eventName: "pointermove", action: (e: PointerEvent): void => onPointerMove(e) },
		{ target: window, eventName: "pointerdown", action: (e: PointerEvent): void => onPointerDown(e) },
		{ target: window, eventName: "pointerup", action: (e: PointerEvent): void => onPointerUp(e) },
		{ target: window, eventName: "dblclick", action: (e: PointerEvent): void => onDoubleClick(e) },
		{ target: window, eventName: "mousedown", action: (e: MouseEvent): void => onMouseDown(e) },
		{ target: window, eventName: "wheel", action: (e: WheelEvent): void => onMouseScroll(e), options: { passive: false } },
		{ target: window, eventName: "modifyinputfield", action: (e: CustomEvent): void => onModifyInputField(e) },
		{ target: window.document.body, eventName: "paste", action: (e: ClipboardEvent): void => onPaste(e) },
		{
			target: app as EventListenerTarget,
			eventName: "blur",
			action: (): void => blurApp(),
		},
	];

	let viewportPointerInteractionOngoing = false;
	let textInput = undefined as undefined | HTMLDivElement;
	let canvasFocused = true;

	function blurApp(): void {
		canvasFocused = false;
	}

	// Keyboard events

	function shouldRedirectKeyboardEventToBackend(e: KeyboardEvent): boolean {
		// Don't redirect when a modal is covering the workspace
		if (dialog.dialogIsVisible()) return false;

		const key = getLatinKey(e);
		if (!key) return false;

		// Don't redirect user input from text entry into HTML elements
		if (key !== "escape" && !(e.ctrlKey && key === "enter") && targetIsTextField(e.target)) {
			return false;
		}

		// Don't redirect paste
		if (key === "v" && e.ctrlKey) return false;

		// Don't redirect a fullscreen request
		if (key === "f11" && e.type === "keydown" && !e.repeat) {
			e.preventDefault();
			fullscreen.toggleFullscreen();
			return false;
		}

		// Don't redirect a reload request
		if (key === "f5") return false;

		// Don't redirect debugging tools
		if (key === "f12" || key === "f8") return false;
		if ((e.ctrlKey || e.metaKey) && e.shiftKey && key === "c") return false;
		if ((e.ctrlKey || e.metaKey) && e.shiftKey && key === "i") return false;
		if ((e.ctrlKey || e.metaKey) && e.shiftKey && key === "j") return false;

		// Don't redirect tab or enter if not in canvas (to allow navigating elements)
		if (!canvasFocused && !targetIsTextField(e.target) && ["tab", "enter", " ", "arrowdown", "arrowup", "arrowleft", "arrowright"].includes(key.toLowerCase())) return false;

		// Redirect to the backend
		return true;
	}

	function onKeyDown(e: KeyboardEvent): void {
		const key = getLatinKey(e);
		if (!key) return;

		if (shouldRedirectKeyboardEventToBackend(e)) {
			e.preventDefault();
			const modifiers = makeKeyboardModifiersBitfield(e);
			editor.instance.on_key_down(key, modifiers);
			return;
		}

		if (dialog.dialogIsVisible()) {
			if (key === "escape") dialog.dismissDialog();
		}
	}

	function onKeyUp(e: KeyboardEvent): void {
		const key = getLatinKey(e);
		if (!key) return;

		if (shouldRedirectKeyboardEventToBackend(e)) {
			e.preventDefault();
			const modifiers = makeKeyboardModifiersBitfield(e);
			editor.instance.on_key_up(key, modifiers);
		}
	}

	// Pointer events

	// While any pointer button is already down, additional button down events are not reported, but they are sent as `pointermove` events and these are handled in the backend
	function onPointerMove(e: PointerEvent): void {
		if (!e.buttons) viewportPointerInteractionOngoing = false;

		// Don't redirect pointer movement to the backend if there's no ongoing interaction and it's over a floating menu on top of the canvas
		// TODO: A better approach is to pass along a boolean to the backend's input preprocessor so it can know if it's being occluded by the GUI.
		// TODO: This would allow it to properly decide to act on removing hover focus from something that was hovered in the canvas before moving over the GUI.
		// TODO: Further explanation: https://github.com/GraphiteEditor/Graphite/pull/623#discussion_r866436197
		const inFloatingMenu = e.target instanceof Element && e.target.closest("[data-floating-menu-content]");
		if (!viewportPointerInteractionOngoing && inFloatingMenu) return;

		const { target } = e;
		const newInCanvas = (target instanceof Element && target.closest("[data-canvas]")) instanceof Element && !targetIsTextField(window.document.activeElement);
		if (newInCanvas && !canvasFocused) {
			canvasFocused = true;
			app?.focus();
		}

		const modifiers = makeKeyboardModifiersBitfield(e);
		editor.instance.on_mouse_move(e.clientX, e.clientY, e.buttons, modifiers);
	}

	function onPointerDown(e: PointerEvent): void {
		const { target } = e;
		const isTargetingCanvas = target instanceof Element && target.closest("[data-canvas]");
		const inDialog = target instanceof Element && target.closest("[data-dialog-modal] [data-floating-menu-content]");
		const inTextInput = target === textInput;

		if (dialog.dialogIsVisible() && !inDialog) {
			dialog.dismissDialog();
			e.preventDefault();
			e.stopPropagation();
		}

		if (!inTextInput) {
			if (textInput) editor.instance.on_change_text(textInputCleanup(textInput.innerText));
			else viewportPointerInteractionOngoing = isTargetingCanvas instanceof Element;
		}

		if (viewportPointerInteractionOngoing) {
			const modifiers = makeKeyboardModifiersBitfield(e);
			editor.instance.on_mouse_down(e.clientX, e.clientY, e.buttons, modifiers);
		}
	}

	function onPointerUp(e: PointerEvent): void {
		if (!e.buttons) viewportPointerInteractionOngoing = false;

		if (!textInput) {
			const modifiers = makeKeyboardModifiersBitfield(e);
			editor.instance.on_mouse_up(e.clientX, e.clientY, e.buttons, modifiers);
		}
	}

	function onDoubleClick(e: PointerEvent): void {
		if (!e.buttons) viewportPointerInteractionOngoing = false;

		if (!textInput) {
			const modifiers = makeKeyboardModifiersBitfield(e);
			editor.instance.on_double_click(e.clientX, e.clientY, e.buttons, modifiers);
		}
	}

	// Mouse events

	function onMouseDown(e: MouseEvent): void {
		// Block middle mouse button auto-scroll mode (the circlar widget that appears and allows quick scrolling by moving the cursor above or below it)
		// This has to be in `mousedown`, not `pointerdown`, to avoid blocking Vue's middle click detection on HTML elements
		if (e.button === 1) e.preventDefault();
	}

	function onMouseScroll(e: WheelEvent): void {
		const { target } = e;
		const isTargetingCanvas = target instanceof Element && target.closest("[data-canvas]");

		// Redirect vertical scroll wheel movement into a horizontal scroll on a horizontally scrollable element
		// There seems to be no possible way to properly employ the browser's smooth scrolling interpolation
		const horizontalScrollableElement = target instanceof Element && target.closest("[data-scrollable-x]");
		if (horizontalScrollableElement && e.deltaY !== 0) {
			horizontalScrollableElement.scrollTo(horizontalScrollableElement.scrollLeft + e.deltaY, 0);
			return;
		}

		if (isTargetingCanvas) {
			e.preventDefault();
			const modifiers = makeKeyboardModifiersBitfield(e);
			editor.instance.on_mouse_scroll(e.clientX, e.clientY, e.buttons, e.deltaX, e.deltaY, e.deltaZ, modifiers);
		}
	}

	function onModifyInputField(e: CustomEvent): void {
		textInput = e.detail;
	}

	// Window events

	function onWindowResize(container: HTMLElement): void {
		const viewports = Array.from(container.querySelectorAll("[data-canvas]"));
		const boundsOfViewports = viewports.map((canvas) => {
			const bounds = canvas.getBoundingClientRect();
			return [bounds.left, bounds.top, bounds.right, bounds.bottom];
		});

		const flattened = boundsOfViewports.flat();
		const data = Float64Array.from(flattened);

		if (boundsOfViewports.length > 0) editor.instance.bounds_of_viewports(data);
	}

	function onBeforeUnload(e: BeforeUnloadEvent): void {
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
	}

	function onPaste(e: ClipboardEvent): void {
		const dataTransfer = e.clipboardData;
		if (!dataTransfer || targetIsTextField(e.target)) return;
		e.preventDefault();

		Array.from(dataTransfer.items).forEach((item) => {
			if (item.type === "text/plain") {
				item.getAsString((text) => {
					if (text.startsWith("graphite/layer: ")) {
						editor.instance.paste_serialized_data(text.substring(16, text.length));
					}
				});
			}

			const file = item.getAsFile();
			if (file?.type.startsWith("image")) {
				file.arrayBuffer().then((buffer): void => {
					const u8Array = new Uint8Array(buffer);

					editor.instance.paste_image(file.type, u8Array, undefined, undefined);
				});
			}
		});
	}

	function targetIsTextField(target: EventTarget | HTMLElement | null): boolean {
		return target instanceof HTMLElement && (target.nodeName === "INPUT" || target.nodeName === "TEXTAREA" || target.isContentEditable);
	}

	// Event bindings

	function bindListeners(): void {
		// Add event bindings for the lifetime of the application
		listeners.forEach(({ target, eventName, action, options }) => target.addEventListener(eventName, action, options));
	}
	function unbindListeners(): void {
		// Remove event bindings after the lifetime of the application (or on hot-module replacement during development)
		listeners.forEach(({ target, eventName, action, options }) => target.removeEventListener(eventName, action, options));
	}

	// Initialization

	// Bind the event listeners
	bindListeners();
	// Resize on creation
	onWindowResize(container);

	// Return the destructor
	return unbindListeners;
}

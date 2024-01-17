import { get } from "svelte/store";

import { type DialogState } from "@graphite/state-providers/dialog";
import { type DocumentState } from "@graphite/state-providers/document";
import { type FullscreenState } from "@graphite/state-providers/fullscreen";
import { type PortfolioState } from "@graphite/state-providers/portfolio";
import { makeKeyboardModifiersBitfield, textInputCleanup, getLocalizedScanCode } from "@graphite/utility-functions/keyboard-entry";
import { platformIsMac } from "@graphite/utility-functions/platform";
import { extractPixelData } from "@graphite/utility-functions/rasterization";
import { stripIndents } from "@graphite/utility-functions/strip-indents";
import { type Editor } from "@graphite/wasm-communication/editor";
import { TriggerPaste } from "@graphite/wasm-communication/messages";

type EventName = keyof HTMLElementEventMap | keyof WindowEventHandlersEventMap | "modifyinputfield";
type EventListenerTarget = {
	addEventListener: typeof window.addEventListener;
	removeEventListener: typeof window.removeEventListener;
};

export function createInputManager(editor: Editor, dialog: DialogState, portfolio: PortfolioState, document: DocumentState, fullscreen: FullscreenState): () => void {
	const app = window.document.querySelector("[data-app-container]") as HTMLElement | undefined;
	app?.focus();

	let viewportPointerInteractionOngoing = false;
	let textToolInteractiveInputElement = undefined as undefined | HTMLDivElement;
	let canvasFocused = true;

	// Event listeners

	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	const listeners: { target: EventListenerTarget; eventName: EventName; action: (event: any) => void; options?: AddEventListenerOptions }[] = [
		{ target: window, eventName: "resize", action: () => onWindowResize(window.document.body) },
		{ target: window, eventName: "beforeunload", action: (e: BeforeUnloadEvent) => onBeforeUnload(e) },
		{ target: window, eventName: "keyup", action: (e: KeyboardEvent) => onKeyUp(e) },
		{ target: window, eventName: "keydown", action: (e: KeyboardEvent) => onKeyDown(e) },
		{ target: window, eventName: "pointermove", action: (e: PointerEvent) => onPointerMove(e) },
		{ target: window, eventName: "pointerdown", action: (e: PointerEvent) => onPointerDown(e) },
		{ target: window, eventName: "pointerup", action: (e: PointerEvent) => onPointerUp(e) },
		{ target: window, eventName: "mousedown", action: (e: MouseEvent) => onMouseDown(e) },
		{ target: window, eventName: "mouseup", action: (e: MouseEvent) => onPotentialDoubleClick(e) },
		{ target: window, eventName: "wheel", action: (e: WheelEvent) => onWheelScroll(e), options: { passive: false } },
		{ target: window, eventName: "modifyinputfield", action: (e: CustomEvent) => onModifyInputField(e) },
		{ target: window, eventName: "focusout", action: () => (canvasFocused = false) },
		{ target: window.document, eventName: "contextmenu", action: (e: MouseEvent) => onContextMenu(e) },
		{ target: window.document, eventName: "fullscreenchange", action: () => fullscreen.fullscreenModeChanged() },
		{ target: window.document.body, eventName: "paste", action: (e: ClipboardEvent) => onPaste(e) },
	];

	// Event bindings

	function bindListeners() {
		// Add event bindings for the lifetime of the application
		listeners.forEach(({ target, eventName, action, options }) => target.addEventListener(eventName, action, options));
	}
	function unbindListeners() {
		// Remove event bindings after the lifetime of the application (or on hot-module replacement during development)
		listeners.forEach(({ target, eventName, action, options }) => target.removeEventListener(eventName, action, options));
	}

	// Keyboard events

	async function shouldRedirectKeyboardEventToBackend(e: KeyboardEvent): Promise<boolean> {
		// Don't redirect when a dialog is covering the workspace
		if (get(dialog).visible) return false;

		const key = await getLocalizedScanCode(e);

		// TODO: Switch to a system where everything is sent to the backend, then the input preprocessor makes decisions and kicks some inputs back to the frontend
		const accelKey = platformIsMac() ? e.metaKey : e.ctrlKey;

		// Don't redirect user input from text entry into HTML elements
		if (targetIsTextField(e.target || undefined) && key !== "Escape" && !(accelKey && ["Enter", "NumpadEnter"].includes(key))) return false;

		// Don't redirect paste
		if (key === "KeyV" && accelKey) return false;

		// Don't redirect a fullscreen request
		if (key === "F11" && e.type === "keydown" && !e.repeat) {
			e.preventDefault();
			fullscreen.toggleFullscreen();
			return false;
		}

		// Don't redirect a reload request
		if (key === "F5") return false;
		if (key === "KeyR" && accelKey) return false;

		// Don't redirect debugging tools
		if (["F12", "F8"].includes(key)) return false;
		if (["KeyC", "KeyI", "KeyJ"].includes(key) && accelKey && e.shiftKey) return false;

		// Don't redirect tab or enter if not in canvas (to allow navigating elements)
		if (!canvasFocused && !targetIsTextField(e.target || undefined) && ["Tab", "Enter", "NumpadEnter", "Space", "ArrowDown", "ArrowLeft", "ArrowRight", "ArrowUp"].includes(key)) return false;

		// Redirect to the backend
		return true;
	}

	async function onKeyDown(e: KeyboardEvent) {
		const key = await getLocalizedScanCode(e);

		const NO_KEY_REPEAT_MODIFIER_KEYS = ["ControlLeft", "ControlRight", "ShiftLeft", "ShiftRight", "MetaLeft", "MetaRight", "AltLeft", "AltRight", "AltGraph", "CapsLock", "Fn", "FnLock"];
		if (e.repeat && NO_KEY_REPEAT_MODIFIER_KEYS.includes(key)) return;

		if (await shouldRedirectKeyboardEventToBackend(e)) {
			e.preventDefault();
			const modifiers = makeKeyboardModifiersBitfield(e);
			editor.instance.onKeyDown(key, modifiers, e.repeat);
			return;
		}

		if (get(dialog).visible && key === "Escape") {
			dialog.dismissDialog();
		}
	}

	async function onKeyUp(e: KeyboardEvent) {
		const key = await getLocalizedScanCode(e);

		if (await shouldRedirectKeyboardEventToBackend(e)) {
			e.preventDefault();
			const modifiers = makeKeyboardModifiersBitfield(e);
			editor.instance.onKeyUp(key, modifiers, e.repeat);
		}
	}

	// Pointer events

	// While any pointer button is already down, additional button down events are not reported, but they are sent as `pointermove` events and these are handled in the backend
	function onPointerMove(e: PointerEvent) {
		if (!e.buttons) viewportPointerInteractionOngoing = false;

		// Don't redirect pointer movement to the backend if there's no ongoing interaction and it's over a floating menu, or the graph overlay, on top of the canvas
		// TODO: A better approach is to pass along a boolean to the backend's input preprocessor so it can know if it's being occluded by the GUI.
		// TODO: This would allow it to properly decide to act on removing hover focus from something that was hovered in the canvas before moving over the GUI.
		// TODO: Further explanation: https://github.com/GraphiteEditor/Graphite/pull/623#discussion_r866436197
		const inFloatingMenu = e.target instanceof Element && e.target.closest("[data-floating-menu-content]");
		const inGraphOverlay = get(document).graphViewOverlayOpen;
		if (!viewportPointerInteractionOngoing && (inFloatingMenu || inGraphOverlay)) return;

		const { target } = e;
		const newInCanvasArea = (target instanceof Element && target.closest("[data-viewport], [data-graph]")) instanceof Element && !targetIsTextField(window.document.activeElement || undefined);
		if (newInCanvasArea && !canvasFocused) {
			canvasFocused = true;
			app?.focus();
		}

		const modifiers = makeKeyboardModifiersBitfield(e);
		editor.instance.onMouseMove(e.clientX, e.clientY, e.buttons, modifiers);
	}

	function onMouseDown(e: MouseEvent) {
		// Block middle mouse button auto-scroll mode (the circlar gizmo that appears and allows quick scrolling by moving the cursor above or below it)
		if (e.button === 1) e.preventDefault();
	}

	function onPointerDown(e: PointerEvent) {
		const { target } = e;
		const isTargetingCanvas = target instanceof Element && target.closest("[data-viewport]");
		const inDialog = target instanceof Element && target.closest("[data-dialog] [data-floating-menu-content]");
		const inTextInput = target === textToolInteractiveInputElement;

		if (get(dialog).visible && !inDialog) {
			dialog.dismissDialog();
			e.preventDefault();
			e.stopPropagation();
		}

		if (!inTextInput) {
			if (textToolInteractiveInputElement) editor.instance.onChangeText(textInputCleanup(textToolInteractiveInputElement.innerText));
			else viewportPointerInteractionOngoing = isTargetingCanvas instanceof Element;
		}

		if (viewportPointerInteractionOngoing) {
			const modifiers = makeKeyboardModifiersBitfield(e);
			editor.instance.onMouseDown(e.clientX, e.clientY, e.buttons, modifiers);
		}
	}

	function onPointerUp(e: PointerEvent) {
		if (!e.buttons) viewportPointerInteractionOngoing = false;

		if (textToolInteractiveInputElement) return;

		const modifiers = makeKeyboardModifiersBitfield(e);
		editor.instance.onMouseUp(e.clientX, e.clientY, e.buttons, modifiers);
	}

	function onPotentialDoubleClick(e: MouseEvent) {
		if (textToolInteractiveInputElement) return;

		// Allow only double-clicks
		if (e.detail !== 2) return;

		// `e.buttons` is always 0 in the `mouseup` event, so we have to convert from `e.button` instead
		let buttons = 1;
		if (e.button === 0) buttons = 1; // LMB
		if (e.button === 1) buttons = 4; // MMB
		if (e.button === 2) buttons = 2; // RMB

		const modifiers = makeKeyboardModifiersBitfield(e);
		editor.instance.onDoubleClick(e.clientX, e.clientY, buttons, modifiers);
	}

	// Mouse events

	function onWheelScroll(e: WheelEvent) {
		const { target } = e;
		const isTargetingCanvas = target instanceof Element && target.closest("[data-viewport]");

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
			editor.instance.onWheelScroll(e.clientX, e.clientY, e.buttons, e.deltaX, e.deltaY, e.deltaZ, modifiers);
		}
	}

	function onContextMenu(e: MouseEvent) {
		if (!targetIsTextField(e.target || undefined) && e.target !== textToolInteractiveInputElement) {
			e.preventDefault();
		}
	}

	// Receives a custom event dispatched when the user begins interactively editing with the text tool.
	// We keep a copy of the text input element to check against when it's active for text entry.
	function onModifyInputField(e: CustomEvent) {
		textToolInteractiveInputElement = e.detail;
	}

	// Window events

	function onWindowResize(container: HTMLElement) {
		const viewports = Array.from(container.querySelectorAll("[data-viewport]"));
		const boundsOfViewports = viewports.map((canvas) => {
			const bounds = canvas.getBoundingClientRect();
			return [bounds.left, bounds.top, bounds.right, bounds.bottom];
		});

		const flattened = boundsOfViewports.flat();
		const data = Float64Array.from(flattened);

		if (boundsOfViewports.length > 0) editor.instance.boundsOfViewports(data);
	}

	async function onBeforeUnload(e: BeforeUnloadEvent) {
		const activeDocument = get(portfolio).documents[get(portfolio).activeDocumentIndex];
		if (activeDocument && !activeDocument.isAutoSaved) editor.instance.triggerAutoSave(activeDocument.id);

		// Skip the message if the editor crashed, since work is already lost
		if (await editor.instance.hasCrashed()) return;

		// Skip the message during development, since it's annoying when testing
		if (await editor.instance.inDevelopmentMode()) return;

		const allDocumentsSaved = get(portfolio).documents.reduce((acc, doc) => acc && doc.isSaved, true);
		if (!allDocumentsSaved) {
			e.returnValue = "Unsaved work will be lost if the web browser tab is closed. Close anyway?";
			e.preventDefault();
		}
	}

	function onPaste(e: ClipboardEvent) {
		const dataTransfer = e.clipboardData;
		if (!dataTransfer || targetIsTextField(e.target || undefined)) return;
		e.preventDefault();

		Array.from(dataTransfer.items).forEach(async (item) => {
			if (item.type === "text/plain") {
				item.getAsString((text) => {
					if (text.startsWith("graphite/layer: ")) {
						editor.instance.pasteSerializedData(text.substring(16, text.length));
					} else if (text.startsWith("graphite/nodes: ")) {
						editor.instance.pasteSerializedNodes(text.substring(16, text.length));
					}
				});
			}

			const file = item.getAsFile();

			if (file?.type === "svg") {
				const text = await file.text();
				editor.instance.pasteSvg(text);

				return;
			}

			if (file?.type.startsWith("image")) {
				const imageData = await extractPixelData(file);
				editor.instance.pasteImage(new Uint8Array(imageData.data), imageData.width, imageData.height);
			}
		});
	}

	// Frontend message subscriptions

	editor.subscriptions.subscribeJsMessage(TriggerPaste, async () => {
		// In the try block, attempt to read from the Clipboard API, which may not have permission and may not be supported in all browsers
		// In the catch block, explain to the user why the paste failed and how to fix or work around the problem
		try {
			// Attempt to check if the clipboard permission is denied, and throw an error if that is the case
			// In Firefox, the `clipboard-read` permission isn't supported, so attempting to query it throws an error
			// In Safari, the entire Permissions API isn't supported, so the query never occurs and this block is skipped without an error and we assume we might have permission
			const clipboardRead = "clipboard-read" as PermissionName;
			const permission = await navigator.permissions?.query({ name: clipboardRead });
			if (permission?.state === "denied") throw new Error("Permission denied");

			// Read the clipboard contents if the Clipboard API is available
			const clipboardItems = await navigator.clipboard.read();
			if (!clipboardItems) throw new Error("Clipboard API unsupported");

			// Read any layer data or images from the clipboard
			Array.from(clipboardItems).forEach(async (item) => {
				// Read plain text and, if it is a layer, pass it to the editor
				if (item.types.includes("text/plain")) {
					const blob = await item.getType("text/plain");
					const reader = new FileReader();
					reader.onload = () => {
						const text = reader.result as string;

						if (text.startsWith("graphite/layer: ")) {
							editor.instance.pasteSerializedData(text.substring(16, text.length));
						}
					};
					reader.readAsText(blob);
				}

				// Read an image from the clipboard and pass it to the editor to be loaded
				const imageType = item.types.find((type) => type.startsWith("image/"));

				if (imageType === "svg") {
					const blob = await item.getType("text/plain");
					const reader = new FileReader();
					reader.onload = () => {
						const text = reader.result as string;
						editor.instance.pasteSvg(text);
					};
					reader.readAsText(blob);

					return;
				}

				if (imageType) {
					const blob = await item.getType(imageType);
					const reader = new FileReader();
					reader.onload = async () => {
						if (reader.result instanceof ArrayBuffer) {
							const imageData = await extractPixelData(new Blob([reader.result], { type: imageType }));
							editor.instance.pasteImage(new Uint8Array(imageData.data), imageData.width, imageData.height);
						}
					};
					reader.readAsArrayBuffer(blob);
				}
			});
		} catch (err) {
			const unsupported = stripIndents`
				This browser does not support reading from the clipboard.
				Use the keyboard shortcut to paste instead.
				`;
			const denied = stripIndents`
				The browser's clipboard permission has been denied.

				Open the browser's website settings (usually accessible
				just left of the URL) to allow this permission.
				`;

			const matchMessage = {
				"clipboard-read": unsupported,
				"Clipboard API unsupported": unsupported,
				"Permission denied": denied,
			};
			const message = Object.entries(matchMessage).find(([key]) => String(err).includes(key))?.[1] || String(err);

			editor.instance.errorDialog("Cannot access clipboard", message);
		}
	});

	// Initialization

	// Bind the event listeners
	bindListeners();
	// Resize on creation
	onWindowResize(window.document.body);

	// Return the destructor
	return unbindListeners;
}

function targetIsTextField(target: EventTarget | HTMLElement | undefined): boolean {
	return target instanceof HTMLElement && (target.nodeName === "INPUT" || target.nodeName === "TEXTAREA" || target.isContentEditable);
}

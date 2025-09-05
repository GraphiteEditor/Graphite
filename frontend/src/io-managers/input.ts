import { get } from "svelte/store";

import { type Editor } from "@graphite/editor";
import { TriggerPaste } from "@graphite/messages";
import { type DialogState } from "@graphite/state-providers/dialog";
import { type DocumentState } from "@graphite/state-providers/document";
import { type FullscreenState } from "@graphite/state-providers/fullscreen";
import { type PortfolioState } from "@graphite/state-providers/portfolio";
import { makeKeyboardModifiersBitfield, textInputCleanup, getLocalizedScanCode } from "@graphite/utility-functions/keyboard-entry";
import { platformIsMac } from "@graphite/utility-functions/platform";
import { extractPixelData } from "@graphite/utility-functions/rasterization";
import { stripIndents } from "@graphite/utility-functions/strip-indents";
import { updateBoundsOfViewports } from "@graphite/utility-functions/viewports";

const BUTTON_LEFT = 0;
const BUTTON_MIDDLE = 1;
const BUTTON_RIGHT = 2;
const BUTTON_BACK = 3;
const BUTTON_FORWARD = 4;

export const PRESS_REPEAT_DELAY_MS = 400;
export const PRESS_REPEAT_INTERVAL_MS = 72;
export const PRESS_REPEAT_INTERVAL_RAPID_MS = 10;

type EventName = keyof HTMLElementEventMap | keyof WindowEventHandlersEventMap | "modifyinputfield" | "pointerlockchange" | "pointerlockerror";
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
	let inPointerLock = false;
	const shakeSamples: { x: number; y: number; time: number }[] = [];
	let lastShakeTime = 0;

	// Event listeners

	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	const listeners: { target: EventListenerTarget; eventName: EventName; action: (event: any) => void; options?: AddEventListenerOptions }[] = [
		{ target: window, eventName: "resize", action: () => updateBoundsOfViewports(editor) },
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
		{ target: window.document, eventName: "pointerlockchange", action: onPointerLockChange },
		{ target: window.document, eventName: "pointerlockerror", action: onPointerLockChange },
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
		potentiallyRestoreCanvasFocus(e);
		if (!canvasFocused && !targetIsTextField(e.target || undefined) && ["Tab", "Enter", "NumpadEnter", "Space", "ArrowDown", "ArrowLeft", "ArrowRight", "ArrowUp"].includes(key)) return false;

		// Don't redirect if a MenuList is open
		if (window.document.querySelector("[data-floating-menu-content]")) return false;

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
			editor.handle.onKeyDown(key, modifiers, e.repeat);
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
			editor.handle.onKeyUp(key, modifiers, e.repeat);
		}
	}

	// Pointer events

	// While any pointer button is already down, additional button down events are not reported, but they are sent as `pointermove` events and these are handled in the backend
	function onPointerMove(e: PointerEvent) {
		potentiallyRestoreCanvasFocus(e);

		if (!e.buttons) viewportPointerInteractionOngoing = false;

		// Don't redirect pointer movement to the backend if there's no ongoing interaction and it's over a floating menu, or the graph overlay, on top of the canvas
		// TODO: A better approach is to pass along a boolean to the backend's input preprocessor so it can know if it's being occluded by the GUI.
		// TODO: This would allow it to properly decide to act on removing hover focus from something that was hovered in the canvas before moving over the GUI.
		// TODO: Further explanation: https://github.com/GraphiteEditor/Graphite/pull/623#discussion_r866436197
		const inFloatingMenu = e.target instanceof Element && e.target.closest("[data-floating-menu-content]");
		const inGraphOverlay = get(document).graphViewOverlayOpen;
		if (!viewportPointerInteractionOngoing && (inFloatingMenu || inGraphOverlay)) return;

		const modifiers = makeKeyboardModifiersBitfield(e);
		if (detectShake(e)) editor.handle.onMouseShake(e.clientX, e.clientY, e.buttons, modifiers);
		editor.handle.onMouseMove(e.clientX, e.clientY, e.buttons, modifiers);
	}

	function onPointerDown(e: PointerEvent) {
		potentiallyRestoreCanvasFocus(e);

		const { target } = e;
		const isTargetingCanvas = target instanceof Element && target.closest("[data-viewport], [data-viewport-container], [data-node-graph]");
		const inDialog = target instanceof Element && target.closest("[data-dialog] [data-floating-menu-content]");
		const inContextMenu = target instanceof Element && target.closest("[data-context-menu]");
		const inTextInput = target === textToolInteractiveInputElement;

		if (get(dialog).visible && !inDialog) {
			dialog.dismissDialog();
			e.preventDefault();
			e.stopPropagation();
		}

		if (!inTextInput && !inContextMenu) {
			if (textToolInteractiveInputElement) {
				const isLeftOrRightClick = e.button === BUTTON_RIGHT || e.button === BUTTON_LEFT;
				editor.handle.onChangeText(textInputCleanup(textToolInteractiveInputElement.innerText), isLeftOrRightClick);
			} else {
				viewportPointerInteractionOngoing = isTargetingCanvas instanceof Element;
			}
		}

		if (viewportPointerInteractionOngoing && isTargetingCanvas instanceof Element) {
			const modifiers = makeKeyboardModifiersBitfield(e);
			editor.handle.onMouseDown(e.clientX, e.clientY, e.buttons, modifiers);
		}
	}

	function onPointerUp(e: PointerEvent) {
		potentiallyRestoreCanvasFocus(e);

		// Don't let the browser navigate back or forward when using the buttons on some mice
		// TODO: This works in Chrome but not in Firefox
		// TODO: Possible workaround: use the browser's history API to block navigation:
		// TODO: <https://stackoverflow.com/questions/57102502/preventing-mouse-fourth-and-fifth-buttons-from-navigating-back-forward-in-browse>
		if (e.button === BUTTON_BACK || e.button === BUTTON_FORWARD) e.preventDefault();

		if (!e.buttons) viewportPointerInteractionOngoing = false;

		if (textToolInteractiveInputElement) return;

		const modifiers = makeKeyboardModifiersBitfield(e);
		editor.handle.onMouseUp(e.clientX, e.clientY, e.buttons, modifiers);
	}

	// Mouse events

	function onPotentialDoubleClick(e: MouseEvent) {
		if (textToolInteractiveInputElement || inPointerLock) return;

		// Allow only events within the viewport or node graph boundaries
		const { target } = e;
		const isTargetingCanvas = target instanceof Element && target.closest("[data-viewport], [data-viewport-container], [data-node-graph]");
		if (!(isTargetingCanvas instanceof Element)) return;

		// Allow only repeated increments of double-clicks (not 1, 3, 5, etc.)
		if (e.detail % 2 == 1) return;

		// `e.buttons` is always 0 in the `mouseup` event, so we have to convert from `e.button` instead
		let buttons = 1;
		if (e.button === BUTTON_LEFT) buttons = 1; // Left
		if (e.button === BUTTON_RIGHT) buttons = 2; // Right
		if (e.button === BUTTON_MIDDLE) buttons = 4; // Middle
		if (e.button === BUTTON_BACK) buttons = 8; // Back
		if (e.button === BUTTON_FORWARD) buttons = 16; // Forward

		const modifiers = makeKeyboardModifiersBitfield(e);
		editor.handle.onDoubleClick(e.clientX, e.clientY, buttons, modifiers);
	}

	function onMouseDown(e: MouseEvent) {
		// Block middle mouse button auto-scroll mode (the circlar gizmo that appears and allows quick scrolling by moving the cursor above or below it)
		if (e.button === BUTTON_MIDDLE) e.preventDefault();
	}

	function onContextMenu(e: MouseEvent) {
		if (!targetIsTextField(e.target || undefined) && e.target !== textToolInteractiveInputElement) {
			e.preventDefault();
		}
	}

	function onPointerLockChange() {
		inPointerLock = Boolean(window.document.pointerLockElement);
	}

	// Wheel events

	function onWheelScroll(e: WheelEvent) {
		const { target } = e;
		const isTargetingCanvas = target instanceof Element && target.closest("[data-viewport], [data-viewport-container], [data-node-graph]");

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
			editor.handle.onWheelScroll(e.clientX, e.clientY, e.buttons, e.deltaX, e.deltaY, e.deltaZ, modifiers);
		}
	}

	// Receives a custom event dispatched when the user begins interactively editing with the text tool.
	// We keep a copy of the text input element to check against when it's active for text entry.
	function onModifyInputField(e: CustomEvent) {
		textToolInteractiveInputElement = e.detail;
	}

	// Window events

	async function onBeforeUnload(e: BeforeUnloadEvent) {
		const activeDocument = get(portfolio).documents[get(portfolio).activeDocumentIndex];
		if (activeDocument && !activeDocument.details.isAutoSaved) editor.handle.triggerAutoSave(activeDocument.id);

		// Skip the message if the editor crashed, since work is already lost
		if (await editor.handle.hasCrashed()) return;

		// Skip the message during development, since it's annoying when testing
		if (await editor.handle.inDevelopmentMode()) return;

		const allDocumentsSaved = get(portfolio).documents.reduce((acc, doc) => acc && doc.details.isSaved, true);
		if (!allDocumentsSaved) {
			e.returnValue = "Unsaved work will be lost if the web browser tab is closed. Close anyway?";
			e.preventDefault();
		}
	}

	function onPaste(e: ClipboardEvent) {
		const dataTransfer = e.clipboardData;
		if (!dataTransfer || targetIsTextField(e.target || undefined)) return;
		e.preventDefault();

		const LAYER_DATA = "graphite/layer: ";
		const NODES_DATA = "graphite/nodes: ";
		const VECTOR_DATA = "graphite/vector: ";

		Array.from(dataTransfer.items).forEach(async (item) => {
			if (item.type === "text/plain") {
				item.getAsString((text) => {
					if (text.startsWith(LAYER_DATA)) {
						editor.handle.pasteSerializedData(text.substring(LAYER_DATA.length, text.length));
					} else if (text.startsWith(NODES_DATA)) {
						editor.handle.pasteSerializedNodes(text.substring(NODES_DATA.length, text.length));
					} else if (text.startsWith(VECTOR_DATA)) {
						editor.handle.pasteSerializedVector(text.substring(VECTOR_DATA.length, text.length));
					}
				});
			}

			const file = item.getAsFile();
			if (!file) return;

			if (file.type.includes("svg")) {
				const text = await file.text();
				editor.handle.pasteSvg(file.name, text);
				return;
			}

			if (file.type.startsWith("image")) {
				const imageData = await extractPixelData(file);
				editor.handle.pasteImage(file.name, new Uint8Array(imageData.data), imageData.width, imageData.height);
			}

			const graphiteFileSuffix = "." + editor.handle.fileExtension();
			if (file.name.endsWith(graphiteFileSuffix)) {
				const content = await file.text();
				const documentName = file.name.slice(0, -graphiteFileSuffix.length);
				editor.handle.openDocumentFile(documentName, content);
			}
		});
	}

	function detectShake(e: PointerEvent | MouseEvent): boolean {
		const SENSITIVITY_DIRECTION_CHANGES = 3;
		const SENSITIVITY_DISTANCE_TO_DISPLACEMENT_RATIO = 0.1;
		const DETECTION_WINDOW_MS = 500;
		const DEBOUNCE_MS = 1000;

		// Add the current mouse position and time to our list of samples
		const now = Date.now();
		shakeSamples.push({ x: e.clientX, y: e.clientY, time: now });

		// Remove samples that are older than our time window
		while (shakeSamples.length > 0 && now - shakeSamples[0].time > DETECTION_WINDOW_MS) {
			shakeSamples.shift();
		}

		// We can't be shaking if it's too early in terms of samples or debounce time
		if (shakeSamples.length <= 3 || now - lastShakeTime <= DEBOUNCE_MS) return false;

		// Calculate the total distance traveled
		let totalDistanceSquared = 0;
		for (let i = 1; i < shakeSamples.length; i += 1) {
			const p1 = shakeSamples[i - 1];
			const p2 = shakeSamples[i];
			totalDistanceSquared += (p2.x - p1.x) ** 2 + (p2.y - p1.y) ** 2;
		}

		// Count the number of times the mouse changes direction significantly, and the average position of the mouse
		let directionChanges = 0;
		const averagePoint = { x: 0, y: 0 };
		let averagePointCount = 0;
		for (let i = 0; i < shakeSamples.length - 2; i += 1) {
			const p1 = shakeSamples[i];
			const p2 = shakeSamples[i + 1];
			const p3 = shakeSamples[i + 2];

			const vector1 = { x: p2.x - p1.x, y: p2.y - p1.y };
			const vector2 = { x: p3.x - p2.x, y: p3.y - p2.y };

			// Check if the dot product is negative, which indicates the angle between vectors is > 90 degrees
			if (vector1.x * vector2.x + vector1.y * vector2.y < 0) directionChanges += 1;

			averagePoint.x += p2.x;
			averagePoint.y += p2.y;
			averagePointCount += 1;
		}
		if (averagePointCount > 0) {
			averagePoint.x /= averagePointCount;
			averagePoint.y /= averagePointCount;
		}

		// Calculate the displacement (the distance between the first and last mouse positions)
		const lastPoint = shakeSamples[shakeSamples.length - 1];
		const displacementSquared = (lastPoint.x - averagePoint.x) ** 2 + (lastPoint.y - averagePoint.y) ** 2;

		// A shake is detected if the mouse has traveled a lot but not moved far, and has changed direction enough times
		if (SENSITIVITY_DISTANCE_TO_DISPLACEMENT_RATIO * totalDistanceSquared >= displacementSquared && directionChanges >= SENSITIVITY_DIRECTION_CHANGES) {
			lastShakeTime = now;
			shakeSamples.length = 0;

			return true;
		}

		return false;
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
			const success = await Promise.any(
				Array.from(clipboardItems).map(async (item) => {
					// Read plain text and, if it is a layer, pass it to the editor
					if (item.types.includes("text/plain")) {
						const blob = await item.getType("text/plain");
						const reader = new FileReader();
						reader.onload = () => {
							const text = reader.result as string;

							if (text.startsWith("graphite/layer: ")) {
								editor.handle.pasteSerializedData(text.substring(16, text.length));
							}
						};
						reader.readAsText(blob);
						return true;
					}

					// Read an image from the clipboard and pass it to the editor to be loaded
					const imageType = item.types.find((type) => type.startsWith("image/"));

					// Import the actual SVG content if it's an SVG
					if (imageType?.includes("svg")) {
						const blob = await item.getType("text/plain");
						const reader = new FileReader();
						reader.onload = () => {
							const text = reader.result as string;
							editor.handle.pasteSvg(undefined, text);
						};
						reader.readAsText(blob);
						return true;
					}

					// Import the bitmap image if it's an image
					if (imageType) {
						const blob = await item.getType(imageType);
						const reader = new FileReader();
						reader.onload = async () => {
							if (reader.result instanceof ArrayBuffer) {
								const imageData = await extractPixelData(new Blob([reader.result], { type: imageType }));
								editor.handle.pasteImage(undefined, new Uint8Array(imageData.data), imageData.width, imageData.height);
							}
						};
						reader.readAsArrayBuffer(blob);
						return true;
					}

					// The API limits what kinds of data we can access, so we can get copied images and our text encodings of copied nodes, but not files (like
					// .graphite or even image files). However, the user can paste those with Ctrl+V, which we recommend they in the error message that's shown to them.
					return false;
				}),
			);

			if (!success) throw new Error("No valid clipboard data");
		} catch (err) {
			const unsupported = stripIndents`
				This browser does not support reading from the clipboard.
				Use the standard keyboard shortcut to paste instead.
				`;
			const denied = stripIndents`
				The browser's clipboard permission has been denied.

				Open the browser's website settings (usually accessible
				just left of the URL) to allow this permission.
				`;
			const nothing = stripIndents`
				No valid clipboard data was found. You may have better
				luck pasting with the standard keyboard shortcut instead.
				`;

			const matchMessage = {
				"clipboard-read": unsupported,
				"Clipboard API unsupported": unsupported,
				"Permission denied": denied,
				"No valid clipboard data": nothing,
			};
			const message = Object.entries(matchMessage).find(([key]) => String(err).includes(key))?.[1] || String(err);

			editor.handle.errorDialog("Cannot access clipboard", message);
		}
	});

	// Helper functions

	function potentiallyRestoreCanvasFocus(e: Event) {
		const { target } = e;
		const newInCanvasArea =
			(target instanceof Element && target.closest("[data-viewport], [data-viewport-container], [data-graph]")) instanceof Element &&
			!targetIsTextField(window.document.activeElement || undefined);
		if (!canvasFocused && newInCanvasArea) {
			canvasFocused = true;
			app?.focus();
		}
	}

	// Initialization

	// Bind the event listeners
	bindListeners();
	// Resize on creation
	updateBoundsOfViewports(editor);

	// Return the destructor
	return unbindListeners;
}

function targetIsTextField(target: EventTarget | HTMLElement | undefined): boolean {
	return target instanceof HTMLElement && (target.nodeName === "INPUT" || target.nodeName === "TEXTAREA" || target.isContentEditable);
}

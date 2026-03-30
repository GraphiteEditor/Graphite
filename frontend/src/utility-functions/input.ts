import { get } from "svelte/store";
import type { DialogStore } from "/src/stores/dialog";
import type { DocumentStore } from "/src/stores/document";
import { toggleFullscreen } from "/src/stores/fullscreen";
import type { PortfolioStore } from "/src/stores/portfolio";
import { pasteFile } from "/src/utility-functions/files";
import { makeKeyboardModifiersBitfield, textInputCleanup, getLocalizedScanCode } from "/src/utility-functions/keyboard-entry";
import { operatingSystem } from "/src/utility-functions/platform";
import type { EditorWrapper } from "/wrapper/pkg/graphite_wasm_wrapper";
import { isPlatformNative } from "/wrapper/pkg/graphite_wasm_wrapper";

const BUTTON_LEFT = 0;
const BUTTON_MIDDLE = 1;
const BUTTON_RIGHT = 2;
const BUTTON_BACK = 3;
const BUTTON_FORWARD = 4;

let viewportPointerInteractionOngoing = false;
let textToolInteractiveInputElement: HTMLDivElement | undefined = undefined;
let canvasFocused = true;
let inPointerLock = false;
let lastShakeTime = 0;
const shakeSamples: { x: number; y: number; time: number }[] = [];

// Keyboard events

export async function shouldRedirectKeyboardEventToBackend(e: KeyboardEvent, dialogStore: DialogStore): Promise<boolean> {
	if (!dialogStore) return false;

	// Don't redirect when a dialog is covering the workspace
	if (get(dialogStore).visible) return false;

	const key = await getLocalizedScanCode(e);

	// TODO: Switch to a system where everything is sent to the backend, then the input preprocessor makes decisions and kicks some inputs back to the frontend
	const accelKey = operatingSystem() === "Mac" ? e.metaKey : e.ctrlKey;

	// Cut, copy, and paste is handled in the backend on desktop
	if (isPlatformNative() && accelKey && ["KeyX", "KeyC", "KeyV"].includes(key)) return true;
	// But on web, we want to not redirect paste
	if (!isPlatformNative() && key === "KeyV" && accelKey) return false;

	// Don't redirect user input from text entry into HTML elements
	if (targetIsTextField(e.target || undefined) && key !== "Escape" && !(accelKey && ["Enter", "NumpadEnter"].includes(key))) return false;

	// Don't redirect tab or enter if not in canvas (to allow navigating elements)
	potentiallyRestoreCanvasFocus(e);
	if (
		!canvasFocused &&
		!targetIsTextField(e.target || undefined) &&
		["Tab", "Enter", "NumpadEnter", "Space", "ArrowDown", "ArrowLeft", "ArrowRight", "ArrowUp"].includes(key) &&
		!(e.ctrlKey || e.metaKey || e.altKey)
	)
		return false;

	// Don't redirect if a MenuList is open
	if (window.document.querySelector("[data-floating-menu-content]")) return false;

	// Web-only keyboard shortcuts
	if (!isPlatformNative()) {
		// Don't redirect a fullscreen request, but process it immediately instead
		if (((operatingSystem() !== "Mac" && key === "F11") || (operatingSystem() === "Mac" && e.ctrlKey && e.metaKey && key === "KeyF")) && e.type === "keydown" && !e.repeat) {
			e.preventDefault();
			toggleFullscreen();
			return false;
		}

		// Don't redirect a reload request
		if (key === "F5") return false;
		if (key === "KeyR" && accelKey) return false;

		// Don't redirect debugging tools
		if (["F12", "F8"].includes(key)) return false;
		if (["KeyC", "KeyI", "KeyJ"].includes(key) && accelKey && e.shiftKey) return false;
	}

	// Redirect to the backend
	return true;
}

export async function onKeyDown(e: KeyboardEvent, editor: EditorWrapper, dialogStore: DialogStore) {
	const key = await getLocalizedScanCode(e);

	const NO_KEY_REPEAT_MODIFIER_KEYS = ["ControlLeft", "ControlRight", "ShiftLeft", "ShiftRight", "MetaLeft", "MetaRight", "AltLeft", "AltRight", "AltGraph", "CapsLock", "Fn", "FnLock"];
	if (e.repeat && NO_KEY_REPEAT_MODIFIER_KEYS.includes(key)) return;

	if (await shouldRedirectKeyboardEventToBackend(e, dialogStore)) {
		e.preventDefault();
		const modifiers = makeKeyboardModifiersBitfield(e);
		editor.onKeyDown(key, modifiers, e.repeat);
		return;
	}

	if (get(dialogStore).visible && key === "Escape") {
		editor.onDialogDismiss();
	}
}

export async function onKeyUp(e: KeyboardEvent, editor: EditorWrapper, dialogStore: DialogStore) {
	const key = await getLocalizedScanCode(e);

	if (await shouldRedirectKeyboardEventToBackend(e, dialogStore)) {
		e.preventDefault();
		const modifiers = makeKeyboardModifiersBitfield(e);
		editor.onKeyUp(key, modifiers, e.repeat);
	}
}

// Pointer events

// While any pointer button is already down, additional button down events are not reported, but they are sent as `pointermove` events and these are handled in the backend
export function onPointerMove(e: PointerEvent, editor: EditorWrapper, documentStore: DocumentStore) {
	potentiallyRestoreCanvasFocus(e);

	if (!e.buttons) viewportPointerInteractionOngoing = false;

	// Don't redirect pointer movement to the backend if there's no ongoing interaction and it's over a floating menu, or the graph overlay, on top of the canvas
	// TODO: A better approach is to pass along a boolean to the backend's input preprocessor so it can know if it's being occluded by the GUI.
	// TODO: This would allow it to properly decide to act on removing hover focus from something that was hovered in the canvas before moving over the GUI.
	// TODO: Further explanation: https://github.com/GraphiteEditor/Graphite/pull/623#discussion_r866436197
	const inFloatingMenu = e.target instanceof Element && e.target.closest("[data-floating-menu-content]");
	const inGraphOverlay = get(documentStore).graphViewOverlayOpen;
	if (!viewportPointerInteractionOngoing && (inFloatingMenu || inGraphOverlay)) return;

	const modifiers = makeKeyboardModifiersBitfield(e);
	if (detectShake(e)) editor.onMouseShake(e.clientX, e.clientY, e.buttons, modifiers);
	editor.onMouseMove(e.clientX, e.clientY, e.buttons, modifiers);
}

export function onPointerDown(e: PointerEvent, editor: EditorWrapper, dialogStore: DialogStore) {
	potentiallyRestoreCanvasFocus(e);

	const inFloatingMenu = e.target instanceof Element && e.target.closest("[data-floating-menu-content]");
	const isTargetingCanvas = !inFloatingMenu && e.target instanceof Element && e.target.closest("[data-viewport], [data-viewport-container], [data-node-graph]");
	const inDialog = e.target instanceof Element && e.target.closest("[data-dialog] [data-floating-menu-content]");
	const inContextMenu = e.target instanceof Element && e.target.closest("[data-context-menu]");
	const inTextInput = e.target === textToolInteractiveInputElement;

	if (get(dialogStore).visible && !inDialog) {
		editor.onDialogDismiss();
		e.preventDefault();
		e.stopPropagation();
	}

	if (!inTextInput && !inContextMenu) {
		if (textToolInteractiveInputElement) {
			const isLeftOrRightClick = e.button === BUTTON_RIGHT || e.button === BUTTON_LEFT;
			editor.onChangeText(textInputCleanup(textToolInteractiveInputElement.innerText), isLeftOrRightClick);
		} else {
			viewportPointerInteractionOngoing = isTargetingCanvas instanceof Element;
		}
	}

	if (viewportPointerInteractionOngoing && isTargetingCanvas instanceof Element) {
		const modifiers = makeKeyboardModifiersBitfield(e);
		editor.onMouseDown(e.clientX, e.clientY, e.buttons, modifiers);
	}
}

export function onPointerUp(e: PointerEvent, editor: EditorWrapper) {
	potentiallyRestoreCanvasFocus(e);

	// Don't let the browser navigate back or forward when using the buttons on some mice
	// TODO: This works in Chrome but not in Firefox
	// TODO: Possible workaround: use the browser's history API to block navigation:
	// TODO: <https://stackoverflow.com/questions/57102502/preventing-mouse-fourth-and-fifth-buttons-from-navigating-back-forward-in-browse>
	if (e.button === BUTTON_BACK || e.button === BUTTON_FORWARD) e.preventDefault();

	if (!e.buttons) viewportPointerInteractionOngoing = false;

	if (textToolInteractiveInputElement) return;

	const modifiers = makeKeyboardModifiersBitfield(e);
	editor.onMouseUp(e.clientX, e.clientY, e.buttons, modifiers);
}

// Mouse events

export function onPotentialDoubleClick(e: MouseEvent, editor: EditorWrapper) {
	if (textToolInteractiveInputElement || inPointerLock) return;

	// Allow only events within the viewport or node graph boundaries
	const isTargetingCanvas = e.target instanceof Element && e.target.closest("[data-viewport], [data-viewport-container], [data-node-graph]");
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
	editor.onDoubleClick(e.clientX, e.clientY, buttons, modifiers);
}

export function onMouseDown(e: MouseEvent) {
	// Block middle mouse button auto-scroll mode (the circular gizmo that appears and allows quick scrolling by moving the cursor above or below it)
	if (e.button === BUTTON_MIDDLE) e.preventDefault();
}

export function onContextMenu(e: MouseEvent) {
	if (!targetIsTextField(e.target || undefined) && e.target !== textToolInteractiveInputElement) {
		e.preventDefault();
	}
}

export function onPointerLockChange() {
	inPointerLock = Boolean(window.document.pointerLockElement);
}

// Wheel events

export function onWheelScroll(e: WheelEvent, editor: EditorWrapper) {
	const isTargetingCanvas = e.target instanceof Element && e.target.closest("[data-viewport], [data-viewport-container], [data-node-graph]");

	// Prevent zooming the entire page when using Ctrl + scroll wheel outside of the viewport
	if (e.ctrlKey && !isTargetingCanvas) {
		e.preventDefault();
	}

	// Redirect vertical scroll wheel movement into a horizontal scroll on a horizontally scrollable element
	// There seems to be no possible way to properly employ the browser's smooth scrolling interpolation
	const horizontalScrollableElement = e.target instanceof Element && e.target.closest("[data-scrollable-x]");
	if (horizontalScrollableElement && e.deltaY !== 0) {
		horizontalScrollableElement.scrollTo(horizontalScrollableElement.scrollLeft + e.deltaY, 0);
		return;
	}

	if (isTargetingCanvas) {
		e.preventDefault();
		const modifiers = makeKeyboardModifiersBitfield(e);
		editor.onWheelScroll(e.clientX, e.clientY, e.buttons, e.deltaX, e.deltaY, e.deltaZ, modifiers);
	}
}

// Receives a custom event dispatched when the user begins interactively editing with the text tool.
// We keep a copy of the text input element to check against when it's active for text entry.
export function onModifyInputField(e: CustomEvent) {
	textToolInteractiveInputElement = e.detail;
}

// Window events

export async function onBeforeUnload(e: BeforeUnloadEvent, editor: EditorWrapper, portfolioStore: PortfolioStore) {
	const activeDocument = get(portfolioStore).documents[get(portfolioStore).activeDocumentIndex];
	if (activeDocument && !activeDocument.details.isAutoSaved) editor.triggerAutoSave(activeDocument.id);

	// Skip the message if the editor crashed, since work is already lost
	if (await editor.hasCrashed()) return;

	// Skip the message during development, since it's annoying when testing
	if (await editor.inDevelopmentMode()) return;

	const allDocumentsSaved = get(portfolioStore).documents.reduce((acc, doc) => acc && doc.details.isSaved, true);
	if (!allDocumentsSaved) {
		e.returnValue = "Unsaved work will be lost if the web browser tab is closed. Close anyway?";
		e.preventDefault();
	}
}

export function onPaste(e: ClipboardEvent, editor: EditorWrapper) {
	const dataTransfer = e.clipboardData;
	if (!dataTransfer || targetIsTextField(e.target || undefined)) return;
	e.preventDefault();

	Array.from(dataTransfer.items).forEach(async (item) => {
		if (item.type === "text/plain") item.getAsString((text) => editor.pasteText(text));
		await pasteFile(item, editor);
	});
}

export function onFocusOut() {
	canvasFocused = false;
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

function targetIsTextField(target: EventTarget | HTMLElement | undefined): boolean {
	return target instanceof HTMLElement && (target.nodeName === "INPUT" || target.nodeName === "TEXTAREA" || target.isContentEditable);
}

function potentiallyRestoreCanvasFocus(e: Event) {
	const appElement = window.document.querySelector("[data-app-container]");
	const app = appElement instanceof HTMLElement ? appElement : null;

	const newInCanvasArea =
		(e.target instanceof Element && e.target.closest("[data-viewport], [data-viewport-container], [data-graph]")) instanceof Element &&
		!targetIsTextField(window.document.activeElement || undefined);
	if (!canvasFocused && newInCanvasArea) {
		canvasFocused = true;
		app?.focus();
	}
}

import type { DialogStore } from "/src/stores/dialog";
import type { DocumentStore } from "/src/stores/document";
import { fullscreenModeChanged } from "/src/stores/fullscreen";
import type { PortfolioStore } from "/src/stores/portfolio";
import type { SubscriptionsRouter } from "/src/subscriptions-router";
import { triggerClipboardRead } from "/src/utility-functions/clipboard";
import {
	onBeforeUnload,
	onKeyUp,
	onKeyDown,
	onPointerMove,
	onPointerDown,
	onPointerUp,
	onMouseDown,
	onPotentialDoubleClick,
	onWheelScroll,
	onModifyInputField,
	onFocusOut,
	onContextMenu,
	onPaste,
	onPointerLockChange,
} from "/src/utility-functions/input";
import type { EditorHandle } from "/wasm/pkg/graphite_wasm";

type EventName = keyof HTMLElementEventMap | keyof WindowEventHandlersEventMap | "modifyinputfield" | "pointerlockchange" | "pointerlockerror";
type EventListenerTarget = {
	addEventListener: typeof window.addEventListener;
	removeEventListener: typeof window.removeEventListener;
};
type Listener = { target: EventListenerTarget; eventName: EventName; action(event: Event): void; options?: AddEventListenerOptions };

export const PRESS_REPEAT_DELAY_MS = 400;
export const PRESS_REPEAT_INTERVAL_MS = 72;
export const PRESS_REPEAT_INTERVAL_RAPID_MS = 10;
const listeners: Listener[] = [
	{ target: window, eventName: "beforeunload", action: (e: BeforeUnloadEvent) => editorHandle && portfolioStore && onBeforeUnload(e, editorHandle, portfolioStore) },
	{ target: window, eventName: "keyup", action: (e: KeyboardEvent) => editorHandle && dialogStore && onKeyUp(e, editorHandle, dialogStore) },
	{ target: window, eventName: "keydown", action: (e: KeyboardEvent) => editorHandle && dialogStore && onKeyDown(e, editorHandle, dialogStore) },
	{ target: window, eventName: "pointermove", action: (e: PointerEvent) => editorHandle && documentStore && onPointerMove(e, editorHandle, documentStore) },
	{ target: window, eventName: "pointerdown", action: (e: PointerEvent) => editorHandle && dialogStore && onPointerDown(e, editorHandle, dialogStore) },
	{ target: window, eventName: "pointerup", action: (e: PointerEvent) => editorHandle && onPointerUp(e, editorHandle) },
	{ target: window, eventName: "mousedown", action: (e: MouseEvent) => onMouseDown(e) },
	{ target: window, eventName: "mouseup", action: (e: MouseEvent) => editorHandle && onPotentialDoubleClick(e, editorHandle) },
	{ target: window, eventName: "wheel", action: (e: WheelEvent) => editorHandle && onWheelScroll(e, editorHandle), options: { passive: false } },
	{ target: window, eventName: "modifyinputfield", action: (e: CustomEvent) => onModifyInputField(e) },
	{ target: window, eventName: "focusout", action: () => onFocusOut() },
	{ target: window.document, eventName: "contextmenu", action: (e: MouseEvent) => onContextMenu(e) },
	{ target: window.document, eventName: "fullscreenchange", action: () => fullscreenModeChanged() },
	{ target: window.document.body, eventName: "paste", action: (e: ClipboardEvent) => editorHandle && onPaste(e, editorHandle) },
	{ target: window.document, eventName: "pointerlockchange", action: onPointerLockChange },
	{ target: window.document, eventName: "pointerlockerror", action: onPointerLockChange },
];

let subscriptionsRouter: SubscriptionsRouter | undefined = undefined;
let editorHandle: EditorHandle | undefined = undefined;
let dialogStore: DialogStore | undefined = undefined;
let portfolioStore: PortfolioStore | undefined = undefined;
let documentStore: DocumentStore | undefined = undefined;

export function createInputManager(subscriptions: SubscriptionsRouter, editor: EditorHandle, dialog: DialogStore, portfolio: PortfolioStore, doc: DocumentStore) {
	destroyInputManager();

	subscriptionsRouter = subscriptions;
	editorHandle = editor;
	dialogStore = dialog;
	portfolioStore = portfolio;
	documentStore = doc;

	subscriptions.subscribeFrontendMessage("TriggerClipboardRead", () => {
		triggerClipboardRead(editor);
	});

	subscriptions.subscribeFrontendMessage("WindowPointerLockMove", (data) => {
		// Desktop app only: dispatch custom pointer lock movement events
		const event = new CustomEvent("pointerlockmove", { detail: { x: data.position[0], y: data.position[1] } });
		window.dispatchEvent(event);
	});

	// Add event bindings for the lifetime of the application
	listeners.forEach(({ target, eventName, action, options }) => target.addEventListener(eventName, action, options));

	// Focus the app container
	const app = window.document.querySelector("[data-app-container]");
	if (app instanceof HTMLElement) app.focus();
}

// Return the destructor
export function destroyInputManager() {
	const subscriptions = subscriptionsRouter;
	if (!subscriptions) return;

	subscriptions.unsubscribeFrontendMessage("TriggerClipboardRead");
	subscriptions.unsubscribeFrontendMessage("WindowPointerLockMove");

	// Remove event bindings after the lifetime of the application (or on hot-module replacement during development)
	listeners.forEach(({ target, eventName, action, options }) => target.removeEventListener(eventName, action, options));
}

// Self-accepting HMR: tear down the old instance and re-create with the new module's code
import.meta.hot?.accept((newModule) => {
	if (subscriptionsRouter && editorHandle && dialogStore && portfolioStore && documentStore)
		newModule?.createInputManager(subscriptionsRouter, editorHandle, dialogStore, portfolioStore, documentStore);
});

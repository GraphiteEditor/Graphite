import type { Editor } from "@graphite/editor";
import type { DialogStore } from "@graphite/stores/dialog";
import type { DocumentStore } from "@graphite/stores/document";
import { fullscreenModeChanged } from "@graphite/stores/fullscreen";
import type { PortfolioStore } from "@graphite/stores/portfolio";
import type { SubscriptionRouter } from "@graphite/subscription-router";
import { triggerClipboardRead } from "@graphite/utility-functions/clipboard";
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
} from "@graphite/utility-functions/input";

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
	{ target: window, eventName: "beforeunload", action: (e: BeforeUnloadEvent) => editorRef && portfolioStore && onBeforeUnload(e, editorRef, portfolioStore) },
	{ target: window, eventName: "keyup", action: (e: KeyboardEvent) => editorRef && dialogStore && onKeyUp(e, editorRef, dialogStore) },
	{ target: window, eventName: "keydown", action: (e: KeyboardEvent) => editorRef && dialogStore && onKeyDown(e, editorRef, dialogStore) },
	{ target: window, eventName: "pointermove", action: (e: PointerEvent) => editorRef && documentStore && onPointerMove(e, editorRef, documentStore) },
	{ target: window, eventName: "pointerdown", action: (e: PointerEvent) => editorRef && dialogStore && onPointerDown(e, editorRef, dialogStore) },
	{ target: window, eventName: "pointerup", action: (e: PointerEvent) => editorRef && onPointerUp(e, editorRef) },
	{ target: window, eventName: "mousedown", action: (e: MouseEvent) => onMouseDown(e) },
	{ target: window, eventName: "mouseup", action: (e: MouseEvent) => editorRef && onPotentialDoubleClick(e, editorRef) },
	{ target: window, eventName: "wheel", action: (e: WheelEvent) => editorRef && onWheelScroll(e, editorRef), options: { passive: false } },
	{ target: window, eventName: "modifyinputfield", action: (e: CustomEvent) => onModifyInputField(e) },
	{ target: window, eventName: "focusout", action: () => onFocusOut() },
	{ target: window.document, eventName: "contextmenu", action: (e: MouseEvent) => onContextMenu(e) },
	{ target: window.document, eventName: "fullscreenchange", action: () => fullscreenModeChanged() },
	{ target: window.document.body, eventName: "paste", action: (e: ClipboardEvent) => editorRef && onPaste(e, editorRef) },
	{ target: window.document, eventName: "pointerlockchange", action: onPointerLockChange },
	{ target: window.document, eventName: "pointerlockerror", action: onPointerLockChange },
];

let subscriptionsRef: SubscriptionRouter | undefined = undefined;
let editorRef: Editor | undefined = undefined;
let dialogStore: DialogStore | undefined = undefined;
let portfolioStore: PortfolioStore | undefined = undefined;
let documentStore: DocumentStore | undefined = undefined;

export function createInputManager(subscriptions: SubscriptionRouter, editor: Editor, dialog: DialogStore, portfolio: PortfolioStore, doc: DocumentStore) {
	destroyInputManager();

	subscriptionsRef = subscriptions;
	editorRef = editor;
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
	const subscriptions = subscriptionsRef;
	if (!subscriptions) return;

	subscriptions.unsubscribeFrontendMessage("TriggerClipboardRead");
	subscriptions.unsubscribeFrontendMessage("WindowPointerLockMove");

	// Remove event bindings after the lifetime of the application (or on hot-module replacement during development)
	listeners.forEach(({ target, eventName, action, options }) => target.removeEventListener(eventName, action, options));
}

// Self-accepting HMR: tear down the old instance and re-create with the new module's code
import.meta.hot?.accept((newModule) => {
	if (subscriptionsRef && editorRef && dialogStore && portfolioStore && documentStore) newModule?.createInputManager(subscriptionsRef, editorRef, dialogStore, portfolioStore, documentStore);
});

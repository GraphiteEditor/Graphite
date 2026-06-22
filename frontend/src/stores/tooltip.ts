import { writable } from "svelte/store";
import type { Writable } from "svelte/store";
import type { SubscriptionsRouter } from "/src/subscriptions-router";
import { operatingSystem } from "/src/utility-functions/platform";
import type { ActionShortcut } from "/wrapper/pkg/graphite_wasm_wrapper";

export type TooltipStore = ReturnType<typeof createTooltipStore>;

const SHOW_TOOLTIP_DELAY_MS = 500;

type TooltipStoreState = {
	visible: boolean;
	element: Element | undefined;
	position: { x: number; y: number };
	shiftClickShortcut: ActionShortcut | undefined;
	altClickShortcut: ActionShortcut | undefined;
	fullscreenShortcut: ActionShortcut | undefined;
};
const initialState: TooltipStoreState = {
	visible: false,
	element: undefined,
	position: { x: 0, y: 0 },
	shiftClickShortcut: undefined,
	altClickShortcut: undefined,
	fullscreenShortcut: undefined,
};

type Listener = { eventName: keyof DocumentEventMap; action(event: Event): void };
const tooltipEventListeners: Listener[] = [
	{ eventName: "mouseover", action: onMouseOver },
	{ eventName: "mousemove", action: onMouseMove },
	{ eventName: "mouseleave", action: onMouseLeave },
	{ eventName: "mousedown", action: closeTooltip },
	{ eventName: "keydown", action: closeTooltip },
	{ eventName: "wheel", action: closeTooltip },
];

let subscriptionsRouter: SubscriptionsRouter | undefined = undefined;
let tooltipTimeout: ReturnType<typeof setTimeout> | undefined = undefined;

// Store state persisted across HMR to maintain reactive subscriptions in the component tree
const store: Writable<TooltipStoreState> = import.meta.hot?.data?.store || writable<TooltipStoreState>(initialState);
if (import.meta.hot) import.meta.hot.data.store = store;
const { subscribe, update } = store;

export function createTooltipStore(subscriptions: SubscriptionsRouter) {
	destroyTooltipStore();

	subscriptionsRouter = subscriptions;

	subscriptions.subscribeFrontendMessage("SendShortcutShiftClick", async (data) => {
		update((state) => {
			state.shiftClickShortcut = data.shortcut;
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("SendShortcutAltClick", async (data) => {
		update((state) => {
			state.altClickShortcut = data.shortcut;
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("SendShortcutFullscreen", async (data) => {
		update((state) => {
			state.fullscreenShortcut = operatingSystem() === "Mac" ? data.shortcutMac : data.shortcut;
			return state;
		});
	});

	tooltipEventListeners.forEach(({ eventName, action }) => document.addEventListener(eventName, action));

	return { subscribe };
}

export function destroyTooltipStore() {
	const subscriptions = subscriptionsRouter;
	if (!subscriptions) return;

	if (tooltipTimeout) clearTimeout(tooltipTimeout);

	subscriptions.unsubscribeFrontendMessage("SendShortcutShiftClick");
	subscriptions.unsubscribeFrontendMessage("SendShortcutAltClick");
	subscriptions.unsubscribeFrontendMessage("SendShortcutFullscreen");

	tooltipEventListeners.forEach(({ eventName, action }) => document.removeEventListener(eventName, action));
}

// Listen for mouse movements onto tooltip-bearing HTML elements to track the future target of a tooltip
function onMouseOver(e: MouseEvent) {
	const element = (e.target instanceof Element && e.target.closest("[data-tooltip-label], [data-tooltip-description], [data-tooltip-shortcut]")) || undefined;

	update((state) => {
		state.visible = false;
		state.element = element;
		return state;
	});
}

// Listen for mouse movements to schedule and position the tooltip, or hide it immediately upon further movement
function onMouseMove(e: MouseEvent) {
	// Hide the tooltip now that the cursor has moved
	update((state) => {
		state.visible = false;
		return state;
	});

	// Before we schedule a new future tooltip appearance, we clear the existing one
	if (tooltipTimeout) clearTimeout(tooltipTimeout);

	// Don't show tooltips while mouse buttons are pressed
	if (e.buttons !== 0) return;

	// Schedule the tooltip to appear at this cursor position after a delay
	tooltipTimeout = setTimeout(() => {
		update((state) => {
			if (state.element) {
				state.visible = true;
				state.position = { x: e.clientX, y: e.clientY };
			}
			return state;
		});
	}, SHOW_TOOLTIP_DELAY_MS);
}

// Hide tooltip and cancel any pending timeout when the mouse leaves the application window
function onMouseLeave() {
	if (tooltipTimeout) clearTimeout(tooltipTimeout);
	closeTooltip();
}

// Stop showing a tooltip if the user clicks or presses a key, and require the user to first move out of the element before it can re-appear
function closeTooltip() {
	update((state) => {
		state.visible = false;
		state.element = undefined;
		return state;
	});
}

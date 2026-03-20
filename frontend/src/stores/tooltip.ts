import { writable } from "svelte/store";
import type { Writable } from "svelte/store";

import type { ActionShortcut } from "@graphite/../wasm/pkg/graphite_wasm";
import type { Editor } from "@graphite/editor";
import { operatingSystem } from "@graphite/utility-functions/platform";

export type TooltipStore = ReturnType<typeof createTooltipStore>;

const SHOW_TOOLTIP_DELAY_MS = 500;

let tooltipTimeout: ReturnType<typeof setTimeout> | undefined = undefined;

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

let editorRef: Editor | undefined = undefined;

// Store state persisted across HMR to maintain reactive subscriptions in the component tree
const store: Writable<TooltipStoreState> = import.meta.hot?.data?.store || writable<TooltipStoreState>(initialState);
if (import.meta.hot) import.meta.hot.data.store = store;
const { subscribe, update } = store;

export function createTooltipStore(editor: Editor) {
	destroyTooltipStore();

	editorRef = editor;

	document.addEventListener("mouseover", onMouseOver);
	document.addEventListener("mousemove", onMouseMove);
	document.addEventListener("mouseleave", onMouseLeave);
	document.addEventListener("mousedown", closeTooltip);
	document.addEventListener("keydown", closeTooltip);
	document.addEventListener("wheel", closeTooltip);

	editor.subscriptions.subscribeFrontendMessage("SendShortcutShiftClick", async (data) => {
		update((state) => {
			state.shiftClickShortcut = data.shortcut;
			return state;
		});
	});
	editor.subscriptions.subscribeFrontendMessage("SendShortcutAltClick", async (data) => {
		update((state) => {
			state.altClickShortcut = data.shortcut;
			return state;
		});
	});
	editor.subscriptions.subscribeFrontendMessage("SendShortcutFullscreen", async (data) => {
		update((state) => {
			state.fullscreenShortcut = operatingSystem() === "Mac" ? data.shortcutMac : data.shortcut;
			return state;
		});
	});

	return { subscribe };
}

export function destroyTooltipStore() {
	const editor = editorRef;
	if (!editor) return;

	if (tooltipTimeout) clearTimeout(tooltipTimeout);

	document.removeEventListener("mouseover", onMouseOver);
	document.removeEventListener("mousemove", onMouseMove);
	document.removeEventListener("mouseleave", onMouseLeave);
	document.removeEventListener("mousedown", closeTooltip);
	document.removeEventListener("keydown", closeTooltip);
	document.removeEventListener("wheel", closeTooltip);

	editor.subscriptions.unsubscribeFrontendMessage("SendShortcutShiftClick");
	editor.subscriptions.unsubscribeFrontendMessage("SendShortcutAltClick");
	editor.subscriptions.unsubscribeFrontendMessage("SendShortcutFullscreen");
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

import { writable } from "svelte/store";

import { type Editor } from "@graphite/editor";
import { SendShortcutAltClick, SendShortcutFullscreen, SendShortcutShiftClick, type ActionShortcut } from "@graphite/messages";
import { operatingSystem } from "@graphite/utility-functions/platform";

const SHOW_TOOLTIP_DELAY_MS = 500;

export function createTooltipState(editor: Editor) {
	const { subscribe, update } = writable({
		visible: false,
		element: undefined as Element | undefined,
		position: { x: 0, y: 0 },
		shiftClickShortcut: undefined as ActionShortcut | undefined,
		altClickShortcut: undefined as ActionShortcut | undefined,
		fullscreenShortcut: undefined as ActionShortcut | undefined,
	});

	let tooltipTimeout: ReturnType<typeof setTimeout> | undefined = undefined;

	// Listen for mouse movements onto tooltip-bearing HTML elements to track the future target of a tooltip
	document.addEventListener("mouseover", (e) => {
		const element = (e.target instanceof Element && e.target.closest("[data-tooltip-label], [data-tooltip-description], [data-tooltip-shortcut]")) || undefined;

		update((state) => {
			state.visible = false;
			state.element = element;
			return state;
		});
	});

	// Listen for mouse movements to schedule and position the tooltip, or hide it immediately upon further movement
	document.addEventListener("mousemove", (e) => {
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
	});

	editor.subscriptions.subscribeJsMessage(SendShortcutShiftClick, async (data) => {
		update((state) => {
			state.shiftClickShortcut = data.shortcut;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(SendShortcutAltClick, async (data) => {
		update((state) => {
			state.altClickShortcut = data.shortcut;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(SendShortcutFullscreen, async (data) => {
		update((state) => {
			state.fullscreenShortcut = operatingSystem() === "Mac" ? data.shortcutMac : data.shortcut;
			return state;
		});
	});

	document.addEventListener("mousedown", closeTooltip);
	document.addEventListener("keydown", closeTooltip);

	// Stop showing a tooltip if the user clicks or presses a key, and require the user to first move out of the element before it can re-appear
	function closeTooltip() {
		update((state) => {
			state.visible = false;
			state.element = undefined;
			return state;
		});
	}

	return {
		subscribe,
	};
}
export type TooltipState = ReturnType<typeof createTooltipState>;

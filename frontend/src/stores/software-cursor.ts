import { get, writable } from "svelte/store";
import type { Writable } from "svelte/store";

export type SoftwareCursorState = {
	visible: boolean;
	x: number;
	y: number;
};

const initialState: SoftwareCursorState = {
	visible: false,
	x: 0,
	y: 0,
};

// Store state persisted across HMR to maintain reactive subscriptions in the component tree
const store: Writable<SoftwareCursorState> = import.meta.hot?.data?.store || writable<SoftwareCursorState>(initialState);
if (import.meta.hot) import.meta.hot.data.store = store;

export const softwareCursor = store;

// The cursor drawn while G/R/S wraps the pointer around the viewport, positioned in viewport coordinates
export function setSoftwareCursor(cursor: SoftwareCursorState): void {
	store.set(cursor);
}

// Window position of the software cursor, or `undefined` while it isn't shown
export function softwareCursorClientPosition(): { x: number; y: number } | undefined {
	const cursor = get(store);
	if (!cursor.visible) return undefined;

	const bounds = window.document.querySelector("[data-viewport-container]")?.getBoundingClientRect();
	return { x: (bounds?.left || 0) + cursor.x, y: (bounds?.top || 0) + cursor.y };
}

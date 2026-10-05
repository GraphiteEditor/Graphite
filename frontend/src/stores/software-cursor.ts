import { writable } from "svelte/store";
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

// Drawn while G/R/S wraps the pointer, positioned in viewport coordinates
export function setSoftwareCursor(cursor: SoftwareCursorState): void {
	store.set(cursor);
}

// Clears the cursor when the component drawing it unmounts mid-transform, so a stale one can't reappear on remount
export function resetSoftwareCursor(): void {
	store.set(initialState);
}

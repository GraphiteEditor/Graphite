import { writable } from "svelte/store";
import type { Writable } from "svelte/store";

export type PanelDragState = {
	active: boolean;
	sourcePanelId: string | undefined;
	draggedTabLabel: string | undefined;
	sourceTabIndex: number;
	// Which panel's tab bar the pointer is currently hovering over (undefined if none)
	hoverTargetPanelId: string | undefined;
	hoverInsertionIndex: number | undefined;
	hoverInsertionMarkerLeft: number | undefined;
};

const initialState: PanelDragState = {
	active: false,
	sourcePanelId: undefined,
	draggedTabLabel: undefined,
	sourceTabIndex: 0,
	hoverTargetPanelId: undefined,
	hoverInsertionIndex: undefined,
	hoverInsertionMarkerLeft: undefined,
};

// Store state persisted across HMR to maintain reactive subscriptions in the component tree
const store: Writable<PanelDragState> = import.meta.hot?.data?.store || writable<PanelDragState>(initialState);
if (import.meta.hot) import.meta.hot.data.store = store;

export const panelDrag = store;

export function startCrossPanelDrag(sourcePanelId: string, draggedTabLabel: string, sourceTabIndex: number) {
	store.update((state) => {
		state.active = true;
		state.sourcePanelId = sourcePanelId;
		state.draggedTabLabel = draggedTabLabel;
		state.sourceTabIndex = sourceTabIndex;
		return state;
	});
}

export function endCrossPanelDrag() {
	store.update((state) => {
		state.active = false;
		state.sourcePanelId = undefined;
		state.draggedTabLabel = undefined;
		state.sourceTabIndex = 0;
		state.hoverTargetPanelId = undefined;
		state.hoverInsertionIndex = undefined;
		state.hoverInsertionMarkerLeft = undefined;
		return state;
	});
}

export function updateCrossPanelHover(hoverTargetPanelId: string | undefined, hoverInsertionIndex: number | undefined, hoverInsertionMarkerLeft: number | undefined) {
	store.update((state) => {
		state.hoverTargetPanelId = hoverTargetPanelId;
		state.hoverInsertionIndex = hoverInsertionIndex;
		state.hoverInsertionMarkerLeft = hoverInsertionMarkerLeft;
		return state;
	});
}

import { writable } from "svelte/store";
import type { Writable } from "svelte/store";
import type { PanelType } from "/wrapper/pkg/graphite_wasm_wrapper";

export type DockingEdge = "Left" | "Right" | "Top" | "Bottom" | "Center";

export type PanelDragState = {
	active: boolean;
	sourcePanelId: string | undefined;
	draggedTabs: PanelType[];
	sourceTabIndex: number;
	// Whether we're dragging an entire tab group (via the tab bar background) vs a single tab
	draggingGroup: boolean;
	// Hover state for tab bar insertion (existing behavior)
	hoverTargetPanelId: string | undefined;
	hoverInsertionIndex: number | undefined;
	hoverInsertionMarkerLeft: number | undefined;
	// Hover state for edge docking (new split creation)
	hoverDockingPanelId: string | undefined;
	hoverDockingEdge: DockingEdge | undefined;
};

const initialState: PanelDragState = {
	active: false,
	sourcePanelId: undefined,
	draggedTabs: [],
	sourceTabIndex: 0,
	draggingGroup: false,
	hoverTargetPanelId: undefined,
	hoverInsertionIndex: undefined,
	hoverInsertionMarkerLeft: undefined,
	hoverDockingPanelId: undefined,
	hoverDockingEdge: undefined,
};

// Store state persisted across HMR to maintain reactive subscriptions in the component tree
const store: Writable<PanelDragState> = import.meta.hot?.data?.store || writable<PanelDragState>(initialState);
if (import.meta.hot) import.meta.hot.data.store = store;

export const panelDrag = store;

export function startCrossPanelDrag(sourcePanelId: string, draggedTabs: PanelType[], sourceTabIndex: number, draggingGroup: boolean) {
	store.update((state) => {
		state.active = true;
		state.sourcePanelId = sourcePanelId;
		state.draggedTabs = draggedTabs;
		state.sourceTabIndex = sourceTabIndex;
		state.draggingGroup = draggingGroup;
		return state;
	});
}

export function endCrossPanelDrag() {
	store.update((state) => {
		state.active = false;
		state.sourcePanelId = undefined;
		state.draggedTabs = [];
		state.sourceTabIndex = 0;
		state.draggingGroup = false;
		state.hoverTargetPanelId = undefined;
		state.hoverInsertionIndex = undefined;
		state.hoverInsertionMarkerLeft = undefined;
		state.hoverDockingPanelId = undefined;
		state.hoverDockingEdge = undefined;
		return state;
	});
}

export function updateCrossPanelHover(hoverTargetPanelId: string | undefined, hoverInsertionIndex: number | undefined, hoverInsertionMarkerLeft: number | undefined) {
	store.update((state) => {
		state.hoverTargetPanelId = hoverTargetPanelId;
		state.hoverInsertionIndex = hoverInsertionIndex;
		state.hoverInsertionMarkerLeft = hoverInsertionMarkerLeft;
		// Clear docking state when hovering a tab bar
		state.hoverDockingPanelId = undefined;
		state.hoverDockingEdge = undefined;
		return state;
	});
}

export function updateDockingHover(panelId: string | undefined, edge: DockingEdge | undefined) {
	store.update((state) => {
		state.hoverDockingPanelId = panelId;
		state.hoverDockingEdge = edge;
		// Clear tab bar insertion state when hovering an edge
		state.hoverTargetPanelId = undefined;
		state.hoverInsertionIndex = undefined;
		state.hoverInsertionMarkerLeft = undefined;
		return state;
	});
}

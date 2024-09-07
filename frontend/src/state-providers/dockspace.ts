import { writable } from "svelte/store";

import type { Editor } from "@graphite/wasm-communication/editor";

import { UpdateDockspace, type DivisionOrPanel } from "@graphite/wasm-communication/messages";

export type TabType = string;

export const MIN_PANEL_SIZE = 100;

export type PanelIdentifier = bigint;

export type TabDragging = { panel: PanelIdentifier; tabIndex: number };

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createDockspaceState(editor: Editor) {
	const state = writable({
		divisionData: undefined as undefined | DivisionOrPanel,
		tabDragging: undefined as undefined | TabDragging,
	});
	const { subscribe, update } = state;

	editor.subscriptions.subscribeJsMessage(UpdateDockspace, (updateDockspace) =>
		update((state) => {
			state.divisionData = updateDockspace.root;
			return state;
		}),
	);

	const startDragging = (panel: PanelIdentifier, tabIndex: number) => {
		update((state) => {
			state.tabDragging = { panel, tabIndex };
			return state;
		});
	};

	const endDragging = () => {
		update((state) => {
			state.tabDragging = undefined;
			return state;
		});
	};

	return {
		subscribe,
		startDragging,
		endDragging,
	};
}
export type DockspaceState = ReturnType<typeof createDockspaceState>;

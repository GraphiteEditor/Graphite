import { writable } from "svelte/store";

import type { Editor } from "@graphite/wasm-communication/editor";

import { UpdateDockspace, type DivisionOrPanel } from "@graphite/wasm-communication/messages";

import Document from "@graphite/components/panels/Document.svelte";
import Layers from "@graphite/components/panels/Layers.svelte";
import Properties from "@graphite/components/panels/Properties.svelte";

const PANEL_COMPONENTS = {
	Document,
	Layers,
	Properties,
};
export type PanelType = keyof typeof PANEL_COMPONENTS;

export const MIN_PANEL_SIZE = 100;

export type PanelIdentifier = bigint;

export type PanelDragging = { panel: PanelIdentifier; tabIndex: number };

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createDockspaceState(editor: Editor) {
	const state = writable({
		panelComponents: PANEL_COMPONENTS,
		divisionData: undefined as undefined | DivisionOrPanel,
		panelDragging: undefined as undefined | PanelDragging,
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
			state.panelDragging = { panel, tabIndex };
			return state;
		});
	};

	const endDragging = () => {
		update((state) => {
			state.panelDragging = undefined;
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

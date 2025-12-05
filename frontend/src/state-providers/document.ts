import { tick } from "svelte";
import { writable } from "svelte/store";

import { type Editor } from "@graphite/editor";

import {
	patchLayout,
	UpdateDocumentBarLayout,
	UpdateToolOptionsLayout,
	UpdateToolShelfLayout,
	UpdateWorkingColorsLayout,
	UpdateNodeGraphControlBarLayout,
	UpdateGraphViewOverlay,
	UpdateGraphFadeArtwork,
} from "@graphite/messages";
import type { Layout } from "@graphite/messages";

export function createDocumentState(editor: Editor) {
	const state = writable({
		// Layouts
		toolOptionsLayout: [] as Layout,
		documentBarLayout: [] as Layout,
		toolShelfLayout: [] as Layout,
		workingColorsLayout: [] as Layout,
		nodeGraphControlBarLayout: [] as Layout,
		// Graph view overlay
		graphViewOverlayOpen: false,
		fadeArtwork: 100,
	});
	const { subscribe, update } = state;

	// Update layouts
	editor.subscriptions.subscribeJsMessage(UpdateGraphFadeArtwork, (updateGraphFadeArtwork) => {
		update((state) => {
			state.fadeArtwork = updateGraphFadeArtwork.percentage;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateToolOptionsLayout, async (updateToolOptionsLayout) => {
		await tick();

		update((state) => {
			patchLayout(state.toolOptionsLayout, updateToolOptionsLayout);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateDocumentBarLayout, async (updateDocumentBarLayout) => {
		await tick();

		update((state) => {
			patchLayout(state.documentBarLayout, updateDocumentBarLayout);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateToolShelfLayout, async (updateToolShelfLayout) => {
		await tick();

		update((state) => {
			patchLayout(state.toolShelfLayout, updateToolShelfLayout);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateWorkingColorsLayout, async (updateWorkingColorsLayout) => {
		await tick();

		update((state) => {
			patchLayout(state.workingColorsLayout, updateWorkingColorsLayout);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateNodeGraphControlBarLayout, (updateNodeGraphControlBarLayout) => {
		update((state) => {
			patchLayout(state.nodeGraphControlBarLayout, updateNodeGraphControlBarLayout);
			return state;
		});
	});

	// Show or hide the graph view overlay
	editor.subscriptions.subscribeJsMessage(UpdateGraphViewOverlay, (updateGraphViewOverlay) => {
		update((state) => {
			state.graphViewOverlayOpen = updateGraphViewOverlay.open;
			return state;
		});
	});

	return {
		subscribe,
	};
}
export type DocumentState = ReturnType<typeof createDocumentState>;

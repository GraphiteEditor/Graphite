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
	editor.subscriptions.subscribeJsMessage(UpdateGraphFadeArtwork, (data) => {
		update((state) => {
			state.fadeArtwork = data.percentage;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateToolOptionsLayout, async (data) => {
		await tick();

		update((state) => {
			patchLayout(state.toolOptionsLayout, data);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateDocumentBarLayout, async (data) => {
		await tick();

		update((state) => {
			patchLayout(state.documentBarLayout, data);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateToolShelfLayout, async (data) => {
		await tick();

		update((state) => {
			patchLayout(state.toolShelfLayout, data);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateWorkingColorsLayout, async (data) => {
		await tick();

		update((state) => {
			patchLayout(state.workingColorsLayout, data);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateNodeGraphControlBarLayout, (data) => {
		update((state) => {
			patchLayout(state.nodeGraphControlBarLayout, data);
			return state;
		});
	});

	// Show or hide the graph view overlay
	editor.subscriptions.subscribeJsMessage(UpdateGraphViewOverlay, (data) => {
		update((state) => {
			state.graphViewOverlayOpen = data.open;
			return state;
		});
	});

	return {
		subscribe,
	};
}
export type DocumentState = ReturnType<typeof createDocumentState>;

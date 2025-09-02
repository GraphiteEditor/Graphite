import { tick } from "svelte";
import { writable } from "svelte/store";

import { type Editor } from "@graphite/editor";

import {
	defaultWidgetLayout,
	patchWidgetLayout,
	UpdateDocumentBarLayout,
	UpdateDocumentModeLayout,
	UpdateToolOptionsLayout,
	UpdateToolShelfLayout,
	UpdateWorkingColorsLayout,
	UpdateNodeGraphControlBarLayout,
	UpdateGraphViewOverlay,
	UpdateGraphFadeArtwork,
} from "@graphite/messages";

export function createDocumentState(editor: Editor) {
	const state = writable({
		// Layouts
		documentModeLayout: defaultWidgetLayout(),
		toolOptionsLayout: defaultWidgetLayout(),
		documentBarLayout: defaultWidgetLayout(),
		toolShelfLayout: defaultWidgetLayout(),
		workingColorsLayout: defaultWidgetLayout(),
		nodeGraphControlBarLayout: defaultWidgetLayout(),
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
	editor.subscriptions.subscribeJsMessage(UpdateDocumentModeLayout, async (updateDocumentModeLayout) => {
		await tick();

		update((state) => {
			patchWidgetLayout(state.documentModeLayout, updateDocumentModeLayout);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateToolOptionsLayout, async (updateToolOptionsLayout) => {
		await tick();

		update((state) => {
			patchWidgetLayout(state.toolOptionsLayout, updateToolOptionsLayout);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateDocumentBarLayout, async (updateDocumentBarLayout) => {
		await tick();

		update((state) => {
			patchWidgetLayout(state.documentBarLayout, updateDocumentBarLayout);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateToolShelfLayout, async (updateToolShelfLayout) => {
		await tick();

		update((state) => {
			patchWidgetLayout(state.toolShelfLayout, updateToolShelfLayout);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateWorkingColorsLayout, async (updateWorkingColorsLayout) => {
		await tick();

		update((state) => {
			patchWidgetLayout(state.workingColorsLayout, updateWorkingColorsLayout);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateNodeGraphControlBarLayout, (updateNodeGraphControlBarLayout) => {
		update((state) => {
			patchWidgetLayout(state.nodeGraphControlBarLayout, updateNodeGraphControlBarLayout);
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

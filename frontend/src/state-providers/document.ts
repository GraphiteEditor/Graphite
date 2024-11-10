import { tick } from "svelte";
import { writable } from "svelte/store";

import { type Editor } from "@graphite/wasm-communication/editor";
import {
	defaultWidgetLayout,
	patchWidgetLayout,
	UpdateDocumentBarLayout,
	UpdateDocumentModeLayout,
	UpdateToolOptionsLayout,
	UpdateToolShelfLayout,
	UpdateWorkingColorsLayout,
	UpdateNodeGraphBarLayout,
	UpdateGraphViewOverlay,
	TriggerDelayedZoomCanvasToFitAll,
	UpdateGraphFadeArtwork,
} from "@graphite/wasm-communication/messages";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createDocumentState(editor: Editor) {
	const state = writable({
		// Layouts
		documentModeLayout: defaultWidgetLayout(),
		toolOptionsLayout: defaultWidgetLayout(),
		documentBarLayout: defaultWidgetLayout(),
		toolShelfLayout: defaultWidgetLayout(),
		workingColorsLayout: defaultWidgetLayout(),
		nodeGraphBarLayout: defaultWidgetLayout(),
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
			// `state.documentModeLayout` is mutated in the function
			patchWidgetLayout(state.documentModeLayout, updateDocumentModeLayout);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateToolOptionsLayout, async (updateToolOptionsLayout) => {
		await tick();

		update((state) => {
			// `state.documentModeLayout` is mutated in the function
			patchWidgetLayout(state.toolOptionsLayout, updateToolOptionsLayout);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateDocumentBarLayout, async (updateDocumentBarLayout) => {
		await tick();

		update((state) => {
			// `state.documentModeLayout` is mutated in the function
			patchWidgetLayout(state.documentBarLayout, updateDocumentBarLayout);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateToolShelfLayout, async (updateToolShelfLayout) => {
		await tick();

		update((state) => {
			// `state.documentModeLayout` is mutated in the function
			patchWidgetLayout(state.toolShelfLayout, updateToolShelfLayout);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateWorkingColorsLayout, async (updateWorkingColorsLayout) => {
		await tick();

		update((state) => {
			// `state.documentModeLayout` is mutated in the function
			patchWidgetLayout(state.workingColorsLayout, updateWorkingColorsLayout);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateNodeGraphBarLayout, (updateNodeGraphBarLayout) => {
		update((state) => {
			patchWidgetLayout(state.nodeGraphBarLayout, updateNodeGraphBarLayout);
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
	editor.subscriptions.subscribeJsMessage(TriggerDelayedZoomCanvasToFitAll, () => {
		// TODO: This is horribly hacky
		[0, 1, 10, 50, 100, 200, 300, 400, 500].forEach((delay) => {
			setTimeout(() => editor.handle.zoomCanvasToFitAll(), delay);
		});
	});

	return {
		subscribe,
	};
}
export type DocumentState = ReturnType<typeof createDocumentState>;

import { tick } from "svelte";

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
	TriggerDelayedZoomCanvasToFitAll,
	UpdateGraphFadeArtwork,
} from "@graphite/messages.svelte";

export const documentContextState = $state({
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

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createDocumentState(editor: Editor) {
	// Set up subscriptions (these run once when the function is called)
	editor.subscriptions.subscribeJsMessage(UpdateGraphFadeArtwork, (updateGraphFadeArtwork) => {
		documentContextState.fadeArtwork = updateGraphFadeArtwork.percentage;
	});

	editor.subscriptions.subscribeJsMessage(UpdateDocumentModeLayout, async (updateDocumentModeLayout) => {
		await tick();
		patchWidgetLayout(documentContextState.documentModeLayout, updateDocumentModeLayout);
	});

	editor.subscriptions.subscribeJsMessage(UpdateToolOptionsLayout, async (updateToolOptionsLayout) => {
		await tick();
		patchWidgetLayout(documentContextState.toolOptionsLayout, updateToolOptionsLayout);
	});

	editor.subscriptions.subscribeJsMessage(UpdateDocumentBarLayout, async (updateDocumentBarLayout) => {
		await tick();
		patchWidgetLayout(documentContextState.documentBarLayout, updateDocumentBarLayout);
	});

	editor.subscriptions.subscribeJsMessage(UpdateToolShelfLayout, async (updateToolShelfLayout) => {
		await tick();
		patchWidgetLayout(documentContextState.toolShelfLayout, updateToolShelfLayout);
	});

	editor.subscriptions.subscribeJsMessage(UpdateWorkingColorsLayout, async (updateWorkingColorsLayout) => {
		await tick();
		patchWidgetLayout(documentContextState.workingColorsLayout, updateWorkingColorsLayout);
	});

	editor.subscriptions.subscribeJsMessage(UpdateNodeGraphControlBarLayout, (updateNodeGraphControlBarLayout) => {
		patchWidgetLayout(documentContextState.nodeGraphControlBarLayout, updateNodeGraphControlBarLayout);
	});

	editor.subscriptions.subscribeJsMessage(UpdateGraphViewOverlay, (updateGraphViewOverlay) => {
		documentContextState.graphViewOverlayOpen = updateGraphViewOverlay.open;
	});

	editor.subscriptions.subscribeJsMessage(TriggerDelayedZoomCanvasToFitAll, () => {
		// TODO: This is horribly hacky
		[0, 1, 10, 50, 100, 200, 300, 400, 500].forEach((delay) => {
			setTimeout(() => editor.handle.zoomCanvasToFitAll(), delay);
		});
	});

	// Return the reactive state object directly
	return documentContextState;
}

export type DocumentState = ReturnType<typeof createDocumentState>;

import { tick } from "svelte";
import { writable } from "svelte/store";

import { type Editor } from "@graphite/wasm-communication/editor";
import {
	defaultWidgetLayout,
	patchWidgetLayout,
	TriggerRefreshBoundsOfViewports,
	UpdateDocumentBarLayout,
	UpdateDocumentModeLayout,
	UpdateToolOptionsLayout,
	UpdateToolShelfLayout,
	UpdateWorkingColorsLayout,
	UpdateNodeGraphBarLayout,
	TriggerGraphViewOverlay,
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
	});
	const { subscribe, update } = state;

	// Update layouts
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

	// Other
	editor.subscriptions.subscribeJsMessage(TriggerRefreshBoundsOfViewports, async () => {
		// Wait to display the unpopulated document panel (missing: tools, options bar content, scrollbar positioning, and canvas)
		await tick();
		// Wait to display the populated document panel
		await tick();

		// Request a resize event so the viewport gets measured now that the canvas is populated and positioned correctly
		window.dispatchEvent(new CustomEvent("resize"));
	});
	// Show or hide the graph view overlay
	editor.subscriptions.subscribeJsMessage(TriggerGraphViewOverlay, (triggerGraphViewOverlay) => {
		update((state) => {
			state.graphViewOverlayOpen = triggerGraphViewOverlay.open;
			return state;
		});
	});

	return {
		subscribe,
	};
}
export type DocumentState = ReturnType<typeof createDocumentState>;

import { tick } from "svelte";
import { writable, type Updater, type Writable } from "svelte/store";

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
	TriggerGraphViewOverlay,
	TriggerDelayedZoomCanvasToFitAll,
	type DocumentViewId,
	UpdateDocumentArtwork,
	UpdateEyedropperSamplingState,
} from "@graphite/wasm-communication/messages";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createDocumentState(editor: Editor) {
	const DEFAULT_VIEW_DATA = {
		// Layouts
		documentModeLayout: defaultWidgetLayout(),
		toolOptionsLayout: defaultWidgetLayout(),
		documentBarLayout: defaultWidgetLayout(),
		toolShelfLayout: defaultWidgetLayout(),
		workingColorsLayout: defaultWidgetLayout(),
		nodeGraphBarLayout: defaultWidgetLayout(),
		// Graph view overlay
		graphViewOverlayOpen: false,

		artwork: "",
		eyedropperSamplingState: UpdateEyedropperSamplingState,
	};
	const state = writable({ documentViews: new Map<DocumentViewId, Writable<typeof DEFAULT_VIEW_DATA>>() });
	const { subscribe, update } = state;

	function updateView(viewId: DocumentViewId, updater: Updater<typeof DEFAULT_VIEW_DATA>) {
		let run = false;
		subscribe((state) => {
			const view = state.documentViews.get(viewId);
			if (view) {
				run = true;
				view.update(updater);
			}
		})();
		if (!run)
			update((state) => {
				const view = writable(DEFAULT_VIEW_DATA);
				view.update(updater);
				state.documentViews.set(viewId, view);
				return state;
			});
	}

	// Update layouts
	editor.subscriptions.subscribeJsMessage(UpdateDocumentModeLayout, async (updateDocumentModeLayout) => {
		await tick();

		updateView(view, (state) => {
			// `state.documentModeLayout` is mutated in the function
			patchWidgetLayout(state.documentModeLayout, updateDocumentModeLayout);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateToolOptionsLayout, async (updateToolOptionsLayout) => {
		await tick();

		updateView(view, (state) => {
			// `state.documentModeLayout` is mutated in the function
			patchWidgetLayout(state.toolOptionsLayout, updateToolOptionsLayout);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateDocumentBarLayout, async (updateDocumentBarLayout) => {
		await tick();

		updateView(view, (state) => {
			// `state.documentModeLayout` is mutated in the function
			patchWidgetLayout(state.documentBarLayout, updateDocumentBarLayout);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateToolShelfLayout, async (updateToolShelfLayout) => {
		await tick();

		updateView(view, (state) => {
			// `state.documentModeLayout` is mutated in the function
			patchWidgetLayout(state.toolShelfLayout, updateToolShelfLayout);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateWorkingColorsLayout, async (updateWorkingColorsLayout) => {
		await tick();

		updateView(view, (state) => {
			// `state.documentModeLayout` is mutated in the function
			patchWidgetLayout(state.workingColorsLayout, updateWorkingColorsLayout);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateNodeGraphBarLayout, (updateNodeGraphBarLayout) => {
		updateView(view, (state) => {
			patchWidgetLayout(state.nodeGraphBarLayout, updateNodeGraphBarLayout);
			return state;
		});
	});

	// Show or hide the graph view overlay
	editor.subscriptions.subscribeJsMessage(TriggerGraphViewOverlay, (triggerGraphViewOverlay) => {
		updateView(view, (state) => {
			state.graphViewOverlayOpen = triggerGraphViewOverlay.open;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(TriggerDelayedZoomCanvasToFitAll, () => {
		setTimeout(() => editor.handle.zoomCanvasToFitAll(), 0);
	});

	// Update rendered SVGs
	editor.subscriptions.subscribeJsMessage(UpdateDocumentArtwork, async (data) => {
		updateView(view, (state) => {
			state.artwork = data.svg;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateEyedropperSamplingState, async (data) => {
		await tick();

		const { mousePosition, primaryColor, secondaryColor, setColorChoice } = data;
		const rgb = await updateEyedropperSamplingState(mousePosition, primaryColor, secondaryColor);

		if (setColorChoice && rgb) {
			if (setColorChoice === "Primary") editor.handle.updatePrimaryColor(...rgb, 1);
			if (setColorChoice === "Secondary") editor.handle.updateSecondaryColor(...rgb, 1);
		}
	});

	// Update scrollbars and rulers
	editor.subscriptions.subscribeJsMessage(UpdateDocumentScrollbars, async (data) => {
		await tick();

		const { position, size, multiplier } = data;
		updateDocumentScrollbars(position, size, multiplier);
	});
	editor.subscriptions.subscribeJsMessage(UpdateDocumentRulers, async (data) => {
		await tick();

		const { origin, spacing, interval, visible } = data;
		updateDocumentRulers(origin, spacing, interval, visible);
	});

	// Update mouse cursor icon
	editor.subscriptions.subscribeJsMessage(UpdateMouseCursor, async (data) => {
		await tick();

		const { cursor } = data;
		updateMouseCursor(cursor);
	});

	// Text entry
	editor.subscriptions.subscribeJsMessage(TriggerTextCommit, async () => {
		await tick();

		triggerTextCommit();
	});
	editor.subscriptions.subscribeJsMessage(DisplayEditableTextbox, async (data) => {
		await tick();

		displayEditableTextbox(data);
	});
	editor.subscriptions.subscribeJsMessage(DisplayEditableTextboxTransform, async (data) => {
		textInputMatrix = data.transform;
	});
	editor.subscriptions.subscribeJsMessage(DisplayRemoveEditableTextbox, async () => {
		await tick();

		displayRemoveEditableTextbox();
	});

	return {
		subscribe,
	};
}
export type DocumentState = ReturnType<typeof createDocumentState>;

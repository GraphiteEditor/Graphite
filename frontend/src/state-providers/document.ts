import { tick } from "svelte";
import { writable, type Readable, type Updater, type Writable } from "svelte/store";

import { type Editor } from "@graphite/wasm-communication/editor";
import type { Color } from "@graphite/wasm-communication/messages";
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
	UpdateDocumentScrollbars,
	UpdateDocumentRulers,
	UpdateMouseCursor,
	DisplayEditableTextbox,
	TriggerTextCommit,
	DisplayEditableTextboxTransform,
	DisplayRemoveEditableTextbox,
	type XY,
	type MouseCursorIcon,
} from "@graphite/wasm-communication/messages";

export type EyedropperState = {
	mousePosition: XY | undefined;
	primaryColor: string;
	secondaryColor: string;
	setColorChoice: "Primary" | "Secondary" | undefined;
};

export type TextInput = {
	text: string;
	lineWidth: undefined | number;
	fontSize: number;
	color: Color;
	url: string;
	transform: number[];
};
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
	eyedropperSamplingState: undefined as undefined | EyedropperState,
	scrollbar: {
		position: { x: 0.5, y: 0.5 } satisfies XY,
		size: { x: 0.5, y: 0.5 } satisfies XY,
		multiplier: { x: 0, y: 0 } satisfies XY,
	},
	rulers: {
		origin: { x: 0, y: 0 } satisfies XY,
		spacing: 100,
		interval: 100,
		visible: true,
	},
	cursor: "default" as MouseCursorIcon,
	textInput: undefined as undefined | TextInput,
};

const view = 41n;

function updateLayouts(editor: Editor, state: Writable<typeof DEFAULT_DOCUMENT_STATE>) {
	editor.subscriptions.subscribeJsMessage(UpdateDocumentModeLayout, async (updateDocumentModeLayout) => {
		await tick();

		updateView(state, view, (state) => {
			// `state.documentModeLayout` is mutated in the function
			patchWidgetLayout(state.documentModeLayout, updateDocumentModeLayout);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateToolOptionsLayout, async (updateToolOptionsLayout) => {
		await tick();

		updateView(state, view, (state) => {
			// `state.documentModeLayout` is mutated in the function
			patchWidgetLayout(state.toolOptionsLayout, updateToolOptionsLayout);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateDocumentBarLayout, async (updateDocumentBarLayout) => {
		await tick();

		updateView(state, view, (state) => {
			// `state.documentModeLayout` is mutated in the function
			patchWidgetLayout(state.documentBarLayout, updateDocumentBarLayout);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateToolShelfLayout, async (updateToolShelfLayout) => {
		await tick();

		updateView(state, view, (state) => {
			// `state.documentModeLayout` is mutated in the function
			patchWidgetLayout(state.toolShelfLayout, updateToolShelfLayout);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateWorkingColorsLayout, async (updateWorkingColorsLayout) => {
		await tick();

		updateView(state, view, (state) => {
			// `state.documentModeLayout` is mutated in the function
			patchWidgetLayout(state.workingColorsLayout, updateWorkingColorsLayout);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateNodeGraphBarLayout, (updateNodeGraphBarLayout) => {
		updateView(state, view, (state) => {
			patchWidgetLayout(state.nodeGraphBarLayout, updateNodeGraphBarLayout);
			return state;
		});
	});
}

const DEFAULT_DOCUMENT_STATE = { documentViews: new Map<DocumentViewId, Writable<typeof DEFAULT_VIEW_DATA>>() };

function updateView(state: Writable<typeof DEFAULT_DOCUMENT_STATE>, viewId: DocumentViewId, updater: Updater<typeof DEFAULT_VIEW_DATA>) {
	let run = false;
	state.subscribe((state) => {
		const view = state.documentViews.get(viewId);
		if (view) {
			run = true;
			view.update(updater);
		}
	})();
	if (!run)
		state.update((state) => {
			const view = writable(DEFAULT_VIEW_DATA);
			view.update(updater);
			state.documentViews.set(viewId, view);
			return state;
		});
}

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createDocumentState(editor: Editor) {
	const state = writable(DEFAULT_DOCUMENT_STATE);

	// Update layouts
	updateLayouts(editor, state);

	// Show or hide the graph view overlay
	editor.subscriptions.subscribeJsMessage(TriggerGraphViewOverlay, (triggerGraphViewOverlay) => {
		updateView(state, view, (state) => {
			state.graphViewOverlayOpen = triggerGraphViewOverlay.open;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(TriggerDelayedZoomCanvasToFitAll, () => {
		setTimeout(() => editor.handle.zoomCanvasToFitAll(), 0);
	});

	// Update rendered SVGs
	editor.subscriptions.subscribeJsMessage(UpdateDocumentArtwork, async (data) => {
		updateView(state, view, (state) => {
			state.artwork = data.svg;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateEyedropperSamplingState, async (data) => {
		updateView(state, view, (state) => {
			state.eyedropperSamplingState = data;
			return state;
		});
	});

	// Update scrollbars and rulers
	editor.subscriptions.subscribeJsMessage(UpdateDocumentScrollbars, async (data) => {
		updateView(state, view, (state) => {
			state.scrollbar = data;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateDocumentRulers, async (data) => {
		updateView(state, view, (state) => {
			state.rulers = data;
			return state;
		});
	});

	// Update mouse cursor icon
	editor.subscriptions.subscribeJsMessage(UpdateMouseCursor, async (data) => {
		updateView(state, view, (state) => {
			state.cursor = data.cursor;
			return state;
		});
	});

	// Text entry
	editor.subscriptions.subscribeJsMessage(TriggerTextCommit, async () => {
		window.dispatchEvent(new CustomEvent("triggerTextCommit", { detail: view }));
	});
	editor.subscriptions.subscribeJsMessage(DisplayEditableTextbox, async (data) => {
		updateView(state, view, (state) => {
			state.textInput = data;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(DisplayEditableTextboxTransform, async (data) => {
		updateView(state, view, (state) => {
			if (state.textInput !== undefined) state.textInput.transform = data.transform;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(DisplayRemoveEditableTextbox, async () => {
		updateView(state, view, (state) => {
			state.textInput = undefined;
			return state;
		});
	});

	return {
		subscribe: state.subscribe,
		getView: (viewId: DocumentViewId) => {
			let view: Readable<typeof DEFAULT_VIEW_DATA> | undefined = undefined;
			state.subscribe((state) => {
				view = state.documentViews.get(viewId);
			})();
			if (view === undefined) {
				const newView = writable(DEFAULT_VIEW_DATA);
				state.update((state) => {
					state.documentViews.set(viewId, newView);
					return state;
				});
				view = newView;
			}
			return view;
		},
	};
}
export type DocumentState = ReturnType<typeof createDocumentState>;

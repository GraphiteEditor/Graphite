import {tick} from "svelte";
import {writable} from "svelte/store";

import { type Editor } from "@/wasm-communication/editor";
import {
	defaultWidgetLayout,
	patchWidgetLayout,
	UpdateDocumentBarLayout,
	UpdateDocumentModeLayout,
	UpdateToolOptionsLayout,
	UpdateToolShelfLayout,
	UpdateWorkingColorsLayout,
} from "@/wasm-communication/messages";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createDocumentState(editor: Editor) {
	const state = writable({
		// Layouts
		documentModeLayout: defaultWidgetLayout(),
		toolOptionsLayout: defaultWidgetLayout(),
		documentBarLayout: defaultWidgetLayout(),
		toolShelfLayout: defaultWidgetLayout(),
		workingColorsLayout: defaultWidgetLayout(),
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

	return {
		subscribe,
	};
}
export type DocumentState = ReturnType<typeof createDocumentState>;

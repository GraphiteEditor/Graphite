import { nextTick, reactive, readonly } from "vue";

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
	const state = reactive({
		// Layouts
		documentModeLayout: defaultWidgetLayout(),
		toolOptionsLayout: defaultWidgetLayout(),
		documentBarLayout: defaultWidgetLayout(),
		toolShelfLayout: defaultWidgetLayout(),
		workingColorsLayout: defaultWidgetLayout(),
	});

	// Update layouts
	editor.subscriptions.subscribeJsMessage(UpdateDocumentModeLayout, async (updateDocumentModeLayout) => {
		await nextTick();
		patchWidgetLayout(state.documentModeLayout, updateDocumentModeLayout);
	});
	editor.subscriptions.subscribeJsMessage(UpdateToolOptionsLayout, async (updateToolOptionsLayout) => {
		await nextTick();
		patchWidgetLayout(state.toolOptionsLayout, updateToolOptionsLayout);
	});
	editor.subscriptions.subscribeJsMessage(UpdateDocumentBarLayout, async (updateDocumentBarLayout) => {
		await nextTick();
		patchWidgetLayout(state.documentBarLayout, updateDocumentBarLayout);
	});
	editor.subscriptions.subscribeJsMessage(UpdateToolShelfLayout, async (updateToolShelfLayout) => {
		await nextTick();
		patchWidgetLayout(state.toolShelfLayout, updateToolShelfLayout);
	});
	editor.subscriptions.subscribeJsMessage(UpdateWorkingColorsLayout, async (updateWorkingColorsLayout) => {
		await nextTick();
		patchWidgetLayout(state.workingColorsLayout, updateWorkingColorsLayout);
	});

	return {
		state: readonly(state) as typeof state,
	};
}
export type DocumentState = ReturnType<typeof createDocumentState>;

import { nextTick, reactive, readonly } from "vue";

import { type Editor } from "@/wasm-communication/editor";
import {
	DisplayEditableTextbox,
	DisplayRemoveEditableTextbox,
	TriggerRefreshBoundsOfViewports,
	TriggerTextCommit,
	TriggerViewportResize,
	UpdateDocumentArtboards,
	UpdateDocumentArtwork,
	UpdateDocumentBarLayout,
	UpdateDocumentModeLayout,
	UpdateDocumentOverlays,
	UpdateDocumentRulers,
	UpdateDocumentScrollbars,
	UpdateMouseCursor,
	UpdateToolOptionsLayout,
	UpdateToolShelfLayout,
	UpdateWorkingColorsLayout,
} from "@/wasm-communication/messages";

import DocumentComponent from "@/components/panels/Document.vue";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createPanelsState(editor: Editor) {
	const state = reactive({
		documentPanel: DocumentComponent,
	});

	// We use `any` instead of `typeof DocumentComponent` as a workaround for the fact that calling this function with the `this` argument from within `Document.vue` isn't a compatible type
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	function registerPanel(type: string, panelComponent: any): void {
		state.documentPanel = panelComponent;
	}

	function subscribeDocumentPanel(): void {
		// Update rendered SVGs
		editor.subscriptions.subscribeJsMessage(UpdateDocumentArtwork, async (updateDocumentArtwork) => {
			await nextTick();
			state.documentPanel.updateDocumentArtwork(updateDocumentArtwork.svg);
		});
		editor.subscriptions.subscribeJsMessage(UpdateDocumentOverlays, async (updateDocumentOverlays) => {
			await nextTick();
			state.documentPanel.updateDocumentOverlays(updateDocumentOverlays.svg);
		});
		editor.subscriptions.subscribeJsMessage(UpdateDocumentArtboards, async (updateDocumentArtboards) => {
			await nextTick();
			state.documentPanel.updateDocumentArtboards(updateDocumentArtboards.svg);
		});

		// Update scrollbars and rulers
		editor.subscriptions.subscribeJsMessage(UpdateDocumentScrollbars, async (updateDocumentScrollbars) => {
			await nextTick();
			const { position, size, multiplier } = updateDocumentScrollbars;
			state.documentPanel.updateDocumentScrollbars(position, size, multiplier);
		});
		editor.subscriptions.subscribeJsMessage(UpdateDocumentRulers, async (updateDocumentRulers) => {
			await nextTick();
			const { origin, spacing, interval } = updateDocumentRulers;
			state.documentPanel.updateDocumentRulers(origin, spacing, interval);
		});

		// Update mouse cursor icon
		editor.subscriptions.subscribeJsMessage(UpdateMouseCursor, async (updateMouseCursor) => {
			await nextTick();
			const { cursor } = updateMouseCursor;
			state.documentPanel.updateMouseCursor(cursor);
		});

		// Text entry
		editor.subscriptions.subscribeJsMessage(TriggerTextCommit, async () => {
			await nextTick();
			state.documentPanel.triggerTextCommit();
		});
		editor.subscriptions.subscribeJsMessage(DisplayEditableTextbox, async (displayEditableTextbox) => {
			await nextTick();
			state.documentPanel.displayEditableTextbox(displayEditableTextbox);
		});
		editor.subscriptions.subscribeJsMessage(DisplayRemoveEditableTextbox, async () => {
			await nextTick();
			state.documentPanel.displayRemoveEditableTextbox();
		});

		// Update layouts
		editor.subscriptions.subscribeJsMessage(UpdateDocumentModeLayout, async (updateDocumentModeLayout) => {
			await nextTick();
			state.documentPanel.updateDocumentModeLayout(updateDocumentModeLayout);
		});
		editor.subscriptions.subscribeJsMessage(UpdateToolOptionsLayout, async (updateToolOptionsLayout) => {
			await nextTick();
			state.documentPanel.updateToolOptionsLayout(updateToolOptionsLayout);
		});
		editor.subscriptions.subscribeJsMessage(UpdateDocumentBarLayout, async (updateDocumentBarLayout) => {
			await nextTick();
			state.documentPanel.updateDocumentBarLayout(updateDocumentBarLayout);
		});
		editor.subscriptions.subscribeJsMessage(UpdateToolShelfLayout, async (updateToolShelfLayout) => {
			await nextTick();
			state.documentPanel.updateToolShelfLayout(updateToolShelfLayout);
		});
		editor.subscriptions.subscribeJsMessage(UpdateWorkingColorsLayout, async (updateWorkingColorsLayout) => {
			await nextTick();
			state.documentPanel.updateWorkingColorsLayout(updateWorkingColorsLayout);
		});

		// Resize elements to render the new viewport size
		editor.subscriptions.subscribeJsMessage(TriggerViewportResize, async () => {
			await nextTick();
			state.documentPanel.viewportResize();
		});
		editor.subscriptions.subscribeJsMessage(TriggerRefreshBoundsOfViewports, async () => {
			// Wait to display the unpopulated document panel (missing: tools, options bar content, scrollbar positioning, and canvas)
			await nextTick();
			// Wait to display the populated document panel
			await nextTick();

			// Request a resize event so the viewport gets measured now that the canvas is populated and positioned correctly
			window.dispatchEvent(new CustomEvent("resize"));
		});
	}

	subscribeDocumentPanel();

	return {
		state: readonly(state) as typeof state,
		registerPanel,
	};
}
export type PanelsState = ReturnType<typeof createPanelsState>;

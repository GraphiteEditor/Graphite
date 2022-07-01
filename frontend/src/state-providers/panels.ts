import { reactive, readonly } from "vue";

import { Editor } from "@/wasm-communication/editor";
import {
	DisplayEditableTextbox,
	DisplayRemoveEditableTextbox,
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
} from "@/wasm-communication/messages";

import DocumentComponent from "@/components/panels/Document.vue";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createPanelsState(editor: Editor) {
	const state = reactive({
		documentPanel: DocumentComponent,
	});

	function registerPanel(type: string, panelComponent: typeof DocumentComponent): void {
		state.documentPanel = panelComponent;
	}

	function subscribeDocumentPanel(): void {
		// Update rendered SVGs
		editor.subscriptions.subscribeJsMessage(UpdateDocumentArtwork, (updateDocumentArtwork) => {
			state.documentPanel.updateDocumentArtwork(updateDocumentArtwork.svg);
		});
		editor.subscriptions.subscribeJsMessage(UpdateDocumentOverlays, (updateDocumentOverlays) => {
			state.documentPanel.updateDocumentOverlays(updateDocumentOverlays.svg);
		});
		editor.subscriptions.subscribeJsMessage(UpdateDocumentArtboards, (updateDocumentArtboards) => {
			state.documentPanel.updateDocumentArtboards(updateDocumentArtboards.svg);
		});

		// Update scrollbars and rulers
		editor.subscriptions.subscribeJsMessage(UpdateDocumentScrollbars, (updateDocumentScrollbars) => {
			const { position, size, multiplier } = updateDocumentScrollbars;
			state.documentPanel.updateDocumentScrollbars(position, size, multiplier);
		});
		editor.subscriptions.subscribeJsMessage(UpdateDocumentRulers, (updateDocumentRulers) => {
			const { origin, spacing, interval } = updateDocumentRulers;
			state.documentPanel.updateDocumentRulers(origin, spacing, interval);
		});

		// Update mouse cursor icon
		editor.subscriptions.subscribeJsMessage(UpdateMouseCursor, (updateMouseCursor) => {
			const { cursor } = updateMouseCursor;
			state.documentPanel.updateMouseCursor(cursor);
		});

		// Text entry
		editor.subscriptions.subscribeJsMessage(TriggerTextCommit, () => {
			state.documentPanel.triggerTextCommit();
		});
		editor.subscriptions.subscribeJsMessage(DisplayEditableTextbox, (displayEditableTextbox) => {
			state.documentPanel.displayEditableTextbox(displayEditableTextbox);
		});
		editor.subscriptions.subscribeJsMessage(DisplayRemoveEditableTextbox, () => {
			state.documentPanel.displayRemoveEditableTextbox();
		});

		// Update layouts
		editor.subscriptions.subscribeJsMessage(UpdateDocumentModeLayout, (updateDocumentModeLayout) => {
			state.documentPanel.updateDocumentModeLayout(updateDocumentModeLayout);
		});
		editor.subscriptions.subscribeJsMessage(UpdateToolOptionsLayout, (updateToolOptionsLayout) => {
			state.documentPanel.updateToolOptionsLayout(updateToolOptionsLayout);
		});
		editor.subscriptions.subscribeJsMessage(UpdateDocumentBarLayout, (updateDocumentBarLayout) => {
			state.documentPanel.updateDocumentBarLayout(updateDocumentBarLayout);
		});
		editor.subscriptions.subscribeJsMessage(UpdateToolShelfLayout, (updateToolShelfLayout) => {
			state.documentPanel.updateToolShelfLayout(updateToolShelfLayout);
		});

		// Resize elements to render the new viewport size
		editor.subscriptions.subscribeJsMessage(TriggerViewportResize, () => {
			state.documentPanel.viewportResize();
		});
	}

	subscribeDocumentPanel();

	return {
		state: readonly(state) as typeof state,
		registerPanel,
	};
}
export type PanelsState = ReturnType<typeof createPanelsState>;

import {tick} from "svelte";
import {writable} from "svelte/store";

import { type Editor } from "@/wasm-communication/editor";
import {
	DisplayEditableTextbox,
	DisplayRemoveEditableTextbox,
	TriggerRefreshBoundsOfViewports,
	TriggerTextCommit,
	TriggerViewportResize,
	UpdateDocumentArtboards,
	UpdateDocumentArtwork,
	UpdateDocumentOverlays,
	UpdateDocumentRulers,
	UpdateDocumentScrollbars,
	UpdateEyedropperSamplingState,
	UpdateMouseCursor,
} from "@/wasm-communication/messages";

import DocumentComponent from "@/components/panels/Document.svelte";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createPanelsState(editor: Editor) {
	const { subscribe, update } = writable({
		documentPanel: DocumentComponent,
	});

	// We use `any` instead of `typeof DocumentComponent` as a workaround for the fact that calling this function with the `this` argument from within `Document.svelte` isn't a compatible type
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	function registerPanel(type: string, panelComponent: any): void {
		update((state) => {
			state.documentPanel = panelComponent;
			return state;
		});
	}

	function subscribeDocumentPanel(): void {
		// Update rendered SVGs
		editor.subscriptions.subscribeJsMessage(UpdateDocumentArtwork, async (updateDocumentArtwork) => {
			await tick();

			update((state) => {
				state.documentPanel.updateDocumentArtwork(updateDocumentArtwork.svg);
				return state;
			});
		});
		editor.subscriptions.subscribeJsMessage(UpdateDocumentOverlays, async (updateDocumentOverlays) => {
			await tick();

			update((state) => {
				state.documentPanel.updateDocumentOverlays(updateDocumentOverlays.svg);
				return state;
			});
		});
		editor.subscriptions.subscribeJsMessage(UpdateDocumentArtboards, async (updateDocumentArtboards) => {
			await tick();

			update((state) => {
				state.documentPanel.updateDocumentArtboards(updateDocumentArtboards.svg);
				return state;
			});
		});
		editor.subscriptions.subscribeJsMessage(UpdateEyedropperSamplingState, async (updateEyedropperSamplingState) => {
			await tick();

			update((state) => {
				const { mousePosition, primaryColor, secondaryColor, setColorChoice } = updateEyedropperSamplingState;
				const rgb = (await state.documentPanel.updateEyedropperSamplingState(mousePosition, primaryColor, secondaryColor));

				if (setColorChoice && rgb) {
					if (setColorChoice === "Primary") editor.instance.updatePrimaryColor(...rgb, 1);
					if (setColorChoice === "Secondary") editor.instance.updateSecondaryColor(...rgb, 1);
				}
				return state;
			});
		});

		// Update scrollbars and rulers
		editor.subscriptions.subscribeJsMessage(UpdateDocumentScrollbars, async (updateDocumentScrollbars) => {
			await tick();

			update((state) => {
				const { position, size, multiplier } = updateDocumentScrollbars;
				state.documentPanel.updateDocumentScrollbars(position, size, multiplier);
				return state;
			});
		});
		editor.subscriptions.subscribeJsMessage(UpdateDocumentRulers, async (updateDocumentRulers) => {
			await tick();

			update((state) => {
				const { origin, spacing, interval } = updateDocumentRulers;
				state.documentPanel.updateDocumentRulers(origin, spacing, interval);
				return state;
			});
		});

		// Update mouse cursor icon
		editor.subscriptions.subscribeJsMessage(UpdateMouseCursor, async (updateMouseCursor) => {
			await tick();

			update((state) => {
				const { cursor } = updateMouseCursor;
				state.documentPanel.updateMouseCursor(cursor);
				return state;
			});
		});

		// Text entry
		editor.subscriptions.subscribeJsMessage(TriggerTextCommit, async () => {
			await tick();

			update((state) => {
				state.documentPanel.triggerTextCommit();
				return state;
			});
		});
		editor.subscriptions.subscribeJsMessage(DisplayEditableTextbox, async (displayEditableTextbox) => {
			await tick();

			update((state) => {
				state.documentPanel.displayEditableTextbox(displayEditableTextbox);
				return state;
			});
		});
		editor.subscriptions.subscribeJsMessage(DisplayRemoveEditableTextbox, async () => {
			await tick();

			update((state) => {
				state.documentPanel.displayRemoveEditableTextbox();
				return state;
			});
		});

		// Resize elements to render the new viewport size
		editor.subscriptions.subscribeJsMessage(TriggerViewportResize, async () => {
			await tick();

			update((state) => {
				state.documentPanel.viewportResize();
				return state;
			});
		});
		editor.subscriptions.subscribeJsMessage(TriggerRefreshBoundsOfViewports, async () => {
			// Wait to display the unpopulated document panel (missing: tools, options bar content, scrollbar positioning, and canvas)
			await tick();
			// Wait to display the populated document panel
			await tick();

			// Request a resize event so the viewport gets measured now that the canvas is populated and positioned correctly
			window.dispatchEvent(new CustomEvent("resize"));
		});
	}

	subscribeDocumentPanel();

	return {
		subscribe,
		registerPanel,
	};
}
export type PanelsState = ReturnType<typeof createPanelsState>;

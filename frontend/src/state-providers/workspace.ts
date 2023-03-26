/* eslint-disable max-classes-per-file */

import {tick} from "svelte";
import {writable} from "svelte/store";

import { type Editor } from "@graphite/wasm-communication/editor";
import { UpdateNodeGraphVisibility } from "@graphite/wasm-communication/messages";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createWorkspaceState(editor: Editor) {
	const { subscribe, update } = writable({
		nodeGraphVisible: false,
	});

	// Set up message subscriptions on creation
	editor.subscriptions.subscribeJsMessage(UpdateNodeGraphVisibility, async (updateNodeGraphVisibility) => {
		update((state) => {
			state.nodeGraphVisible = updateNodeGraphVisibility.visible;
			return state;
		});
		
		// Update the viewport bounds
		await tick();
		window.dispatchEvent(new Event("resize"));
	});

	return {
		subscribe,
	};
}
export type WorkspaceState = ReturnType<typeof createWorkspaceState>;

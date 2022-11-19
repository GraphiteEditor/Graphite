/* eslint-disable max-classes-per-file */
import { nextTick, reactive, readonly } from "vue";

import { type Editor } from "@/wasm-communication/editor";
import { UpdateNodeGraphVisibility } from "@/wasm-communication/messages";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createWorkspaceState(editor: Editor) {
	const state = reactive({
		nodeGraphVisible: false,
	});

	// Set up message subscriptions on creation
	editor.subscriptions.subscribeJsMessage(UpdateNodeGraphVisibility, async (updateNodeGraphVisibility) => {
		state.nodeGraphVisible = updateNodeGraphVisibility.visible;
		// Update the viewport bounds
		await nextTick();
		window.dispatchEvent(new Event("resize"));
	});

	return {
		state: readonly(state) as typeof state,
	};
}
export type WorkspaceState = ReturnType<typeof createWorkspaceState>;

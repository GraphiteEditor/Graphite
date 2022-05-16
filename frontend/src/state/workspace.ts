/* eslint-disable max-classes-per-file */
import { reactive, readonly } from "vue";

import { UpdateNodeGraphVisibility } from "@/dispatcher/js-messages";
import { EditorState } from "@/state/wasm-loader";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createWorkspaceState(editor: EditorState) {
	const state = reactive({
		nodeGraphVisible: false,
	});

	// Set up message subscriptions on creation
	editor.dispatcher.subscribeJsMessage(UpdateNodeGraphVisibility, (updateNodeGraphVisibility) => {
		state.nodeGraphVisible = updateNodeGraphVisibility.visible;
	});

	return {
		state: readonly(state),
	};
}
export type WorkspaceState = ReturnType<typeof createWorkspaceState>;

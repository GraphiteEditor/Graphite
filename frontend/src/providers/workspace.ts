/* eslint-disable max-classes-per-file */
import { reactive, readonly } from "vue";

import { Editor } from "@/interop/editor";
import { UpdateNodeGraphVisibility } from "@/interop/js-messages";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createWorkspaceState(editor: Editor) {
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

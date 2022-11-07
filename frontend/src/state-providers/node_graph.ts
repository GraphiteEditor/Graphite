/* eslint-disable max-classes-per-file */
import { reactive, readonly } from "vue";

import { type Editor } from "@/wasm-communication/editor";
import type { FrontendNode } from "@/wasm-communication/messages";
import { UpdateNodeGraph } from "@/wasm-communication/messages";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createNodeGraphState(editor: Editor) {
	const state = reactive({
		nodes: [] as FrontendNode[],
	});

	// Set up message subscriptions on creation
	editor.subscriptions.subscribeJsMessage(UpdateNodeGraph, (updateNodeGraph) => {
		state.nodes = updateNodeGraph.nodes;
	});

	return {
		state: readonly(state) as typeof state,
	};
}
export type NodeGraphState = ReturnType<typeof createNodeGraphState>;

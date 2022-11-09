import { reactive, readonly } from "vue";

import { type Editor } from "@/wasm-communication/editor";
import type { FrontendNodeLink } from "@/wasm-communication/messages";
import { type FrontendNode, UpdateNodeGraph } from "@/wasm-communication/messages";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createNodeGraphState(editor: Editor) {
	const state = reactive({
		nodes: [] as FrontendNode[],
		links: [] as FrontendNodeLink[],
	});

	// Set up message subscriptions on creation
	editor.subscriptions.subscribeJsMessage(UpdateNodeGraph, (updateNodeGraph) => {
		state.nodes = updateNodeGraph.nodes;
		state.links = updateNodeGraph.links;
	});

	return {
		state: readonly(state) as typeof state,
	};
}
export type NodeGraphState = ReturnType<typeof createNodeGraphState>;

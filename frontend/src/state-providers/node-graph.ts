import {writable} from "svelte/store";

import { type Editor } from "@graphite/wasm-communication/editor";
import {
	type FrontendNode,
	type FrontendNodeLink,
	type FrontendNodeType,
	UpdateNodeGraph,
	UpdateNodeTypes,
	UpdateZoomWithScroll,
} from "@graphite/wasm-communication/messages";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createNodeGraphState(editor: Editor) {
	const { subscribe, update } = writable({
		nodes: [] as FrontendNode[],
		links: [] as FrontendNodeLink[],
		nodeTypes: [] as FrontendNodeType[],
		zoomWithScroll: false as boolean,
	});

	// Set up message subscriptions on creation
	editor.subscriptions.subscribeJsMessage(UpdateNodeGraph, (updateNodeGraph) => {
		update((state) => {
			state.nodes = updateNodeGraph.nodes;
			state.links = updateNodeGraph.links;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateNodeTypes, (updateNodeTypes) => {
		update((state) => {
			state.nodeTypes = updateNodeTypes.nodeTypes;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateZoomWithScroll, (updateZoomWithScroll) => {
		update((state) => {
			state.zoomWithScroll = updateZoomWithScroll.zoomWithScroll;
			return state;
		});
	});

	return {
		subscribe,
	};
}
export type NodeGraphState = ReturnType<typeof createNodeGraphState>;

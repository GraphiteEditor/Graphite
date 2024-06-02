import { writable } from "svelte/store";

import { type Editor } from "@graphite/wasm-communication/editor";
import {
	type FrontendNode,
	type FrontendNodeWire as FrontendNodeWire,
	type FrontendNodeType,
	UpdateNodeGraph,
	UpdateNodeGraphSelection,
	UpdateNodeTypes,
	UpdateNodeThumbnail,
	UpdateSubgraphPath,
	UpdateZoomWithScroll,
} from "@graphite/wasm-communication/messages";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createNodeGraphState(editor: Editor) {
	const { subscribe, update } = writable({
		nodes: [] as FrontendNode[],
		wires: [] as FrontendNodeWire[],
		nodeTypes: [] as FrontendNodeType[],
		zoomWithScroll: false as boolean,
		thumbnails: new Map<bigint, string>(),
		selected: [] as bigint[],
		subgraphPath: [] as string[],
	});

	// Set up message subscriptions on creation
	editor.subscriptions.subscribeJsMessage(UpdateNodeGraph, (updateNodeGraph) => {
		update((state) => {
			state.nodes = updateNodeGraph.nodes;
			state.wires = updateNodeGraph.wires;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateNodeGraphSelection, (updateNodeGraphSelection) => {
		update((state) => {
			state.selected = updateNodeGraphSelection.selected;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateNodeTypes, (updateNodeTypes) => {
		update((state) => {
			state.nodeTypes = updateNodeTypes.nodeTypes;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateNodeThumbnail, (updateNodeThumbnail) => {
		update((state) => {
			state.thumbnails.set(updateNodeThumbnail.id, updateNodeThumbnail.value);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateSubgraphPath, (UpdateSubgraphPath) => {
		update((state) => {
			state.subgraphPath = UpdateSubgraphPath.subgraphPath;
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

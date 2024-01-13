import { writable } from "svelte/store";

import { type Editor } from "@graphite/wasm-communication/editor";
import {
	type FrontendNode,
	type FrontendNodeLink,
	type FrontendNodeType,
	UpdateNodeGraph,
	UpdateNodeTypes,
	UpdateNodeThumbnail,
	UpdateZoomWithScroll,
	UpdateNodeGraphSelection,
} from "@graphite/wasm-communication/messages";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createNodeGraphState(editor: Editor) {
	const { subscribe, update } = writable({
		nodes: [] as FrontendNode[],
		links: [] as FrontendNodeLink[],
		nodeTypes: [] as FrontendNodeType[],
		zoomWithScroll: false as boolean,
		thumbnails: new Map<bigint, string>(),
		selected: [] as bigint[],
	});

	// Set up message subscriptions on creation
	editor.subscriptions.subscribeJsMessage(UpdateNodeGraph, (updateNodeGraph) => {
		update((state) => {
			state.nodes = updateNodeGraph.nodes;
			state.links = updateNodeGraph.links;
			const newThumbnails = new Map<bigint, string>();
			// Transfer over any preexisting thumbnails from itself
			state.nodes.forEach((node) => {
				const thumbnail = state.thumbnails.get(node.id);
				if (thumbnail) newThumbnails.set(node.id, thumbnail);
			});
			state.thumbnails = newThumbnails;
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
	editor.subscriptions.subscribeJsMessage(UpdateZoomWithScroll, (updateZoomWithScroll) => {
		update((state) => {
			state.zoomWithScroll = updateZoomWithScroll.zoomWithScroll;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateNodeGraphSelection, (updateNodeGraphSelection) => {
		update((state) => {
			state.selected = updateNodeGraphSelection.selected;
			return state;
		});
	});

	return {
		subscribe,
	};
}
export type NodeGraphState = ReturnType<typeof createNodeGraphState>;

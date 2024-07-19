import { writable } from "svelte/store";

import { type Editor } from "@graphite/wasm-communication/editor";
import {
	type Box,
	type ContextMenuInformation,
	type FrontendNode,
	type FrontendNodeWire as FrontendNodeWire,
	type FrontendNodeType,
	type WirePath,
	UpdateBox,
	UpdateContextMenuInformation,
	UpdateLayerWidths,
	UpdateNodeGraph,
	UpdateNodeGraphSelection,
	UpdateNodeGraphTransform,
	UpdateNodeTypes,
	UpdateNodeThumbnail,
	UpdateWirePathInProgress,
	UpdateZoomWithScroll,
} from "@graphite/wasm-communication/messages";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createNodeGraphState(editor: Editor) {
	const { subscribe, update } = writable({
		box: undefined as Box | undefined,
		contextMenuInformation: undefined as ContextMenuInformation | undefined,
		layerWidths: new Map<bigint, number>(),
		nodes: [] as FrontendNode[],
		wires: [] as FrontendNodeWire[],
		wirePathInProgress: undefined as WirePath | undefined,
		nodeTypes: [] as FrontendNodeType[],
		zoomWithScroll: false as boolean,
		thumbnails: new Map<bigint, string>(),
		selected: [] as bigint[],
		transform: { scale: 1, x: 0, y: 0 },
	});

	// Set up message subscriptions on creation
	editor.subscriptions.subscribeJsMessage(UpdateBox, (updateBox) => {
		update((state) => {
			state.box = updateBox.box;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateContextMenuInformation, (updateContextMenuInformation) => {
		update((state) => {
			state.contextMenuInformation = updateContextMenuInformation.contextMenuInformation;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateLayerWidths, (updateLayerWidths) => {
		update((state) => {
			state.layerWidths = updateLayerWidths.layerWidths;
			return state;
		});
	});
	// TODO: Add a way to only update the nodes that have changed
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
	editor.subscriptions.subscribeJsMessage(UpdateNodeGraphTransform, (updateNodeGraphTransform) => {
		update((state) => {
			state.transform = updateNodeGraphTransform.transform;
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
	editor.subscriptions.subscribeJsMessage(UpdateWirePathInProgress, (updateWirePathInProgress) => {
		update((state) => {
			state.wirePathInProgress = updateWirePathInProgress.wirePath;
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

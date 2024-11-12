import { writable } from "svelte/store";

import { type Editor } from "@graphite/wasm-communication/editor";
import type { FrontendGraphOutput, FrontendGraphInput } from "@graphite/wasm-communication/messages";
import {
	type Box,
	type FrontendClickTargets,
	type ContextMenuInformation,
	type FrontendNode,
	type FrontendNodeWire as FrontendNodeWire,
	type FrontendNodeType,
	type WirePath,
	SendUIMetadata,
	UpdateBox,
	UpdateClickTargets,
	UpdateContextMenuInformation,
	UpdateInSelectedNetwork,
	UpdateImportsExports,
	UpdateLayerWidths,
	UpdateNodeGraph,
	UpdateNodeGraphSelection,
	UpdateNodeGraphTransform,
	UpdateNodeThumbnail,
	UpdateWirePathInProgress,
	UpdateZoomWithScroll,
} from "@graphite/wasm-communication/messages";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createNodeGraphState(editor: Editor) {
	const { subscribe, update } = writable({
		box: undefined as Box | undefined,
		clickTargets: undefined as FrontendClickTargets | undefined,
		contextMenuInformation: undefined as ContextMenuInformation | undefined,
		layerWidths: new Map<bigint, number>(),
		chainWidths: new Map<bigint, number>(),
		hasLeftInputWire: new Map<bigint, boolean>(),
		imports: [] as { outputMetadata: FrontendGraphOutput; position: { x: number; y: number } }[],
		exports: [] as { inputMetadata: FrontendGraphInput; position: { x: number; y: number } }[],
		addImport: undefined as { x: number; y: number } | undefined,
		addExport: undefined as { x: number; y: number } | undefined,
		nodes: new Map<bigint, FrontendNode>(),
		wires: [] as FrontendNodeWire[],
		wirePathInProgress: undefined as WirePath | undefined,
		inputTypeDescriptions: new Map<string, string>(),
		nodeDescriptions: new Map<string, string>(),
		nodeTypes: [] as FrontendNodeType[],
		zoomWithScroll: false as boolean,
		thumbnails: new Map<bigint, string>(),
		selected: [] as bigint[],
		transform: { scale: 1, x: 0, y: 0 },
		inSelectedNetwork: true,
	});

	// Set up message subscriptions on creation
	editor.subscriptions.subscribeJsMessage(SendUIMetadata, (UIMetadata) => {
		update((state) => {
			state.inputTypeDescriptions = UIMetadata.inputTypeDescriptions;
			state.nodeDescriptions = UIMetadata.nodeDescriptions;
			state.nodeTypes = UIMetadata.nodeTypes;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateBox, (updateBox) => {
		update((state) => {
			state.box = updateBox.box;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateClickTargets, (UpdateClickTargets) => {
		update((state) => {
			state.clickTargets = UpdateClickTargets.clickTargets;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateContextMenuInformation, (updateContextMenuInformation) => {
		update((state) => {
			state.contextMenuInformation = updateContextMenuInformation.contextMenuInformation;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateImportsExports, (updateImportsExports) => {
		update((state) => {
			state.imports = updateImportsExports.imports;
			state.exports = updateImportsExports.exports;
			state.addImport = updateImportsExports.addImport;
			state.addExport = updateImportsExports.addExport;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateInSelectedNetwork, (updateInSelectedNetwork) => {
		update((state) => {
			state.inSelectedNetwork = updateInSelectedNetwork.inSelectedNetwork;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateLayerWidths, (updateLayerWidths) => {
		update((state) => {
			state.layerWidths = updateLayerWidths.layerWidths;
			state.chainWidths = updateLayerWidths.chainWidths;
			state.hasLeftInputWire = updateLayerWidths.hasLeftInputWire;
			return state;
		});
	});
	// TODO: Add a way to only update the nodes that have changed
	editor.subscriptions.subscribeJsMessage(UpdateNodeGraph, (updateNodeGraph) => {
		update((state) => {
			state.nodes.clear();
			updateNodeGraph.nodes.forEach((node) => {
				state.nodes.set(node.id, node);
			});
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

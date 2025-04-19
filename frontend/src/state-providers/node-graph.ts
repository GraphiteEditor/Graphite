import { writable } from "svelte/store";

import { type Editor } from "@graphite/editor";
import type { FrontendGraphOutput, FrontendGraphInput } from "@graphite/messages";
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
	UpdateImportReorderIndex,
	UpdateExportReorderIndex,
	UpdateImportsExports,
	UpdateLayerWidths,
	UpdateNodeGraph,
	UpdateNodeGraphSelection,
	UpdateNodeGraphTransform,
	UpdateNodeThumbnail,
	UpdateWirePathInProgress,
} from "@graphite/messages";

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
		wiresDirectNotGridAligned: false,
		wirePathInProgress: undefined as WirePath | undefined,
		nodeDescriptions: new Map<string, string>(),
		nodeTypes: [] as FrontendNodeType[],
		thumbnails: new Map<bigint, string>(),
		selected: [] as bigint[],
		transform: { scale: 1, x: 0, y: 0 },
		inSelectedNetwork: true,
		reorderImportIndex: undefined as number | undefined,
		reorderExportIndex: undefined as number | undefined,
	});

	// Set up message subscriptions on creation
	editor.subscriptions.subscribeJsMessage(SendUIMetadata, (uiMetadata) => {
		update((state) => {
			state.nodeDescriptions = uiMetadata.nodeDescriptions;
			state.nodeTypes = uiMetadata.nodeTypes;
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
	editor.subscriptions.subscribeJsMessage(UpdateImportReorderIndex, (updateImportReorderIndex) => {
		update((state) => {
			state.reorderImportIndex = updateImportReorderIndex.importIndex;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateExportReorderIndex, (updateExportReorderIndex) => {
		update((state) => {
			state.reorderExportIndex = updateExportReorderIndex.exportIndex;
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
			state.wiresDirectNotGridAligned = updateNodeGraph.wiresDirectNotGridAligned;
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

	return {
		subscribe,
	};
}
export type NodeGraphState = ReturnType<typeof createNodeGraphState>;

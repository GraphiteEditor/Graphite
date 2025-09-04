import { writable } from "svelte/store";

import { type Editor } from "@graphite/editor";
import {
	type Box,
	type FrontendClickTargets,
	type ContextMenuInformation,
	type FrontendNode,
	type FrontendNodeType,
	type WirePath,
	ClearAllNodeGraphWires,
	SendUIMetadata,
	UpdateBox,
	UpdateClickTargets,
	UpdateContextMenuInformation,
	UpdateInSelectedNetwork,
	UpdateImportReorderIndex,
	UpdateExportReorderIndex,
	UpdateImportsExports,
	UpdateLayerWidths,
	UpdateNodeGraphNodes,
	UpdateVisibleNodes,
	UpdateNodeGraphWires,
	UpdateNodeGraphSelection,
	UpdateNodeGraphTransform,
	UpdateNodeThumbnail,
	UpdateWirePathInProgress,
} from "@graphite/messages";

export function createNodeGraphState(editor: Editor) {
	const { subscribe, update } = writable({
		box: undefined as Box | undefined,
		clickTargets: undefined as FrontendClickTargets | undefined,
		contextMenuInformation: undefined as ContextMenuInformation | undefined,
		layerWidths: new Map<bigint, number>(),
		chainWidths: new Map<bigint, number>(),
		hasLeftInputWire: new Map<bigint, boolean>(),
		updateImportsExports: undefined as UpdateImportsExports | undefined,
		nodes: new Map<bigint, FrontendNode>(),
		visibleNodes: new Set<bigint>(),
		/// The index is the exposed input index. The exports have a first key value of u32::MAX.
		wires: new Map<bigint, Map<number, WirePath>>(),
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
			state.updateImportsExports = updateImportsExports;
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
	editor.subscriptions.subscribeJsMessage(UpdateNodeGraphNodes, (updateNodeGraphNodes) => {
		update((state) => {
			state.nodes.clear();
			updateNodeGraphNodes.nodes.forEach((node) => {
				state.nodes.set(node.id, node);
			});
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateVisibleNodes, (updateVisibleNodes) => {
		update((state) => {
			state.visibleNodes = new Set<bigint>(updateVisibleNodes.nodes);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateNodeGraphWires, (updateNodeWires) => {
		update((state) => {
			updateNodeWires.wires.forEach((wireUpdate) => {
				let inputMap = state.wires.get(wireUpdate.id);
				// If it doesn't exist, create it and set it in the outer map
				if (!inputMap) {
					inputMap = new Map();
					state.wires.set(wireUpdate.id, inputMap);
				}
				if (wireUpdate.wirePathUpdate !== undefined) {
					inputMap.set(wireUpdate.inputIndex, wireUpdate.wirePathUpdate);
				} else {
					inputMap.delete(wireUpdate.inputIndex);
				}
			});
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(ClearAllNodeGraphWires, (_) => {
		update((state) => {
			state.wires.clear();
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

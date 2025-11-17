import { writable } from "svelte/store";

import { type Editor } from "@graphite/editor";
import type { NodeGraphError } from "@graphite/messages";
import {
	type FrontendSelectionBox,
	type FrontendClickTargets,
	type ContextMenuInformation,
	type FrontendNodeOld,
	type FrontendNodeType,
	type WirePathInProgress,
	type WirePathOld,
	ClearAllNodeGraphWiresOld,
	SendUIMetadata,
	UpdateNodeGraphSelectionBox,
	UpdateClickTargets,
	UpdateContextMenuInformation,
	UpdateInSelectedNetworkOld,
	UpdateImportReorderIndex,
	UpdateExportReorderIndex,
	UpdateImportsExports,
	UpdateLayerWidthsOld,
	UpdateNodeGraphNodesOld,
	UpdateVisibleNodesOld,
	UpdateNodeGraphWiresOld,
	UpdateNodeGraphSelectionOld,
	UpdateNodeGraphTransform,
	UpdateNodeThumbnail,
	UpdateWirePathInProgress,
	UpdateNodeGraphError,
	UpdateRenderNativeNodeGraph,
	UpdateGraphFadeArtworkOld,
} from "@graphite/messages";

export function createNodeGraphState(editor: Editor) {
	const { subscribe, update } = writable({
		// Data that will continue to be rendered in Svelte
		selectionBox: undefined as FrontendSelectionBox | undefined,
		clickTargets: undefined as FrontendClickTargets | undefined,
		error: undefined as NodeGraphError | undefined,
		wirePathInProgress: undefined as WirePathInProgress | undefined,
		importsExports: undefined as UpdateImportsExports | undefined,
		reorderImportIndex: undefined as number | undefined,
		reorderExportIndex: undefined as number | undefined,
		contextMenuInformation: undefined as ContextMenuInformation | undefined,
		nodeTypes: [] as FrontendNodeType[],
		nodeDescriptions: new Map<string, string>(),
		thumbnails: new Map<bigint, string>(),
		transform: { scale: 1, x: 0, y: 0 },

		// Data that will be removed when the node graph is rendered natively
		nodesOld: new Map<bigint, FrontendNodeOld>(),
		selectedOld: [] as bigint[],
		wiresOld: new Map<bigint, Map<number, WirePathOld>>(),
		opacityOld: 80,
		inSelectedNetworkOld: true,
		previewedNodeOld: undefined as bigint | undefined,
		visibleNodesOld: new Set<bigint>(),
		layerWidthsOld: new Map<bigint, number>(),
		chainWidthsOld: new Map<bigint, number>(),
		hasLeftInputWireOld: new Map<bigint, boolean>(),
		renderNativeNodeGraphOld: false,
	});

	// Set up message subscriptions on creation
	editor.subscriptions.subscribeJsMessage(SendUIMetadata, (uiMetadata) => {
		update((state) => {
			state.nodeDescriptions = uiMetadata.nodeDescriptions;
			state.nodeTypes = uiMetadata.nodeTypes;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateNodeGraphSelectionBox, (updateBox) => {
		update((state) => {
			state.selectionBox = updateBox.box;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateClickTargets, (updateClickTargets) => {
		update((state) => {
			state.clickTargets = updateClickTargets.clickTargets;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateWirePathInProgress, (updateWirePathInProgress) => {
		update((state) => {
			state.wirePathInProgress = updateWirePathInProgress.wirePathInProgress;
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
			state.importsExports = updateImportsExports;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateInSelectedNetworkOld, (updateInSelectedNetwork) => {
		update((state) => {
			state.inSelectedNetworkOld = updateInSelectedNetwork.inSelectedNetwork;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateLayerWidthsOld, (updateLayerWidths) => {
		update((state) => {
			state.layerWidthsOld = updateLayerWidths.layerWidths;
			state.chainWidthsOld = updateLayerWidths.chainWidths;
			state.hasLeftInputWireOld = updateLayerWidths.hasLeftInputWire;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateNodeGraphNodesOld, (updateNodeGraphNodesOld) => {
		update((state) => {
			state.nodesOld.clear();
			updateNodeGraphNodesOld.nodes.forEach((node) => {
				state.nodesOld.set(node.id, node);
			});
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateNodeGraphError, (updateNodeGraphError) => {
		update((state) => {
			state.error = updateNodeGraphError.error;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateVisibleNodesOld, (updateVisibleNodesOld) => {
		update((state) => {
			state.visibleNodesOld = new Set<bigint>(updateVisibleNodesOld.nodes);
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateNodeGraphWiresOld, (updateNodeWires) => {
		update((state) => {
			updateNodeWires.wires.forEach((wireUpdate) => {
				let inputMap = state.wiresOld.get(wireUpdate.id);
				// If it doesn't exist, create it and set it in the outer map
				if (!inputMap) {
					inputMap = new Map();
					state.wiresOld.set(wireUpdate.id, inputMap);
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
	editor.subscriptions.subscribeJsMessage(ClearAllNodeGraphWiresOld, (_) => {
		update((state) => {
			state.wiresOld.clear();
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateNodeGraphSelectionOld, (updateNodeGraphSelection) => {
		update((state) => {
			state.selectedOld = updateNodeGraphSelection.selected;
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
			state.wirePathInProgress = updateWirePathInProgress.wirePathInProgress;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateRenderNativeNodeGraph, (updateRenderNativeNodeGraph) => {
		update((state) => {
			state.renderNativeNodeGraphOld = updateRenderNativeNodeGraph.renderNativeNodeGraph;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateGraphFadeArtworkOld, (updateGraphFadeArtwork) => {
		update((state) => {
			state.opacityOld = updateGraphFadeArtwork.percentage;
			return state;
		});
	});
	return {
		subscribe,
	};
}
export type NodeGraphState = ReturnType<typeof createNodeGraphState>;

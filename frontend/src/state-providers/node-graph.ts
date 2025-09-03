import { writable } from "svelte/store";

import { type Editor } from "@graphite/editor";
import {
	type FrontendSelectionBox,
	type FrontendClickTargets,
	type ContextMenuInformation,
	type FrontendNodeToRender,
	type FrontendNodeType,
	type WirePathInProgress,
	SendUIMetadata,
	UpdateClickTargets,
	UpdateContextMenuInformation,
	UpdateImportReorderIndex,
	UpdateExportReorderIndex,
	UpdateImportsExports,
	UpdateLayerWidths,
	UpdateNodeGraphRender,
	UpdateNativeNodeGraphRender,
	UpdateVisibleNodes,
	UpdateNodeGraphTransform,
	UpdateNodeThumbnail,
	UpdateWirePathInProgress,
	UpdateNodeGraphSelectionBox,
} from "@graphite/messages";

export function createNodeGraphState(editor: Editor) {
	const { subscribe, update } = writable({
		// Data that will continue to be rendered in Svelte for now
		selectionBox: undefined as FrontendSelectionBox | undefined,
		clickTargets: undefined as FrontendClickTargets | undefined,
		wirePathInProgress: undefined as WirePathInProgress | undefined,
		updateImportsExports: undefined as UpdateImportsExports | undefined,
		reorderImportIndex: undefined as number | undefined,
		reorderExportIndex: undefined as number | undefined,

		contextMenuInformation: undefined as ContextMenuInformation | undefined,
		nodeTypes: [] as FrontendNodeType[],
		nodeDescriptions: new Map<string, string>(),

		// Data that will be moved into the node graph to be rendered natively
		nodesToRender: new Map<bigint, FrontendNodeToRender>(),
		opacity: 0.8,
		inSelectedNetwork: true,
		previewedNode: undefined as bigint | undefined,

		// TODO: Remove these fields
		visibleNodes: new Set<bigint>(),
		layerWidths: new Map<bigint, number>(),

		// Data that will be passed in the context
		thumbnails: new Map<bigint, string>(),
		transform: { scale: 1, x: 0, y: 0 },
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
			state.updateImportsExports = updateImportsExports;
			return state;
		});
	});

	editor.subscriptions.subscribeJsMessage(UpdateLayerWidths, (updateLayerWidths) => {
		update((state) => {
			state.layerWidths = updateLayerWidths.layerWidths;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateNodeGraphRender, (updateNodeGraphRender) => {
		update((state) => {
			state.nodesToRender.clear();
			updateNodeGraphRender.nodesToRender.forEach((node) => {
				state.nodesToRender.set(node.metadata.nodeId, node);
			});
			state.opacity = updateNodeGraphRender.opacity;
			state.inSelectedNetwork = updateNodeGraphRender.inSelectedNetwork;
			state.previewedNode = updateNodeGraphRender.previewedNode;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateNativeNodeGraphRender, (updateNativeNodeGraphRender) => {
		update((state) => {
			state.nativeNodeGraphRender = updateNativeNodeGraphRender.nativeNodeGraphRender;
			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(UpdateVisibleNodes, (updateVisibleNodes) => {
		update((state) => {
			state.visibleNodes = new Set<bigint>(updateVisibleNodes.nodes);
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

	return {
		subscribe,
	};
}
export type NodeGraphState = ReturnType<typeof createNodeGraphState>;

import { writable } from "svelte/store";

import type { Editor } from "@graphite/wasm-communication/editor";
import type { LayerPanelEntry } from "@graphite/wasm-communication/messages";
import {
	defaultWidgetLayout,
	patchWidgetLayout,
	UpdateDocumentLayerDetails,
	UpdateDocumentLayerStructureJs,
	UpdateLayersPanelOptionsLayout,
	type DataBuffer,
} from "@graphite/wasm-communication/messages";

type DocumentLayerStructure = {
	layerId: bigint;
	children: DocumentLayerStructure[];
};

export type LayerListingInfo = {
	folderIndex: number;
	bottomLayer: boolean;
	editingName: boolean;
	entry: LayerPanelEntry;
};

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createLayerState(editor: Editor) {
	const { subscribe, update } = writable({
		layersPanelOptionsLayout: defaultWidgetLayout(),
		// Layer data
		layerCache: new Map<string, LayerPanelEntry>(), // TODO: replace with BigUint64Array as index
		layers: [] as LayerListingInfo[],
	});

	function newUpdateDocumentLayerStructure(dataBuffer: DataBuffer): DocumentLayerStructure {
		const pointerNum = Number(dataBuffer.pointer);
		const lengthNum = Number(dataBuffer.length);

		const wasmMemoryBuffer = editor.raw.buffer;

		// Decode the folder structure encoding
		const encoding = new DataView(wasmMemoryBuffer, pointerNum, lengthNum);

		// The structure section indicates how to read through the upcoming layer list and assign depths to each layer
		const structureSectionLength = Number(encoding.getBigUint64(0, true));
		const structureSectionMsbSigned = new DataView(wasmMemoryBuffer, pointerNum + 8, structureSectionLength * 8);

		// The layer IDs section lists each layer ID sequentially in the tree, as it will show up in the panel
		const layerIdsSection = new DataView(wasmMemoryBuffer, pointerNum + 8 + structureSectionLength * 8);

		let layersEncountered = 0;
		let currentFolder: DocumentLayerStructure = { layerId: BigInt(-1), children: [] };
		const currentFolderStack = [currentFolder];

		for (let i = 0; i < structureSectionLength; i += 1) {
			const msbSigned = structureSectionMsbSigned.getBigUint64(i * 8, true);
			const msbMask = BigInt(1) << BigInt(64 - 1);

			// Set the MSB to 0 to clear the sign and then read the number as usual
			const numberOfLayersAtThisDepth = msbSigned & ~msbMask;

			// Store child folders in the current folder (until we are interrupted by an indent)
			for (let j = 0; j < numberOfLayersAtThisDepth; j += 1) {
				const layerId = layerIdsSection.getBigUint64(layersEncountered * 8, true);
				layersEncountered += 1;

				const childLayer: DocumentLayerStructure = { layerId, children: [] };
				currentFolder.children.push(childLayer);
			}

			// Check the sign of the MSB, where a 1 is a negative (outward) indent
			const subsequentDirectionOfDepthChange = (msbSigned & msbMask) === BigInt(0);
			// Inward
			if (subsequentDirectionOfDepthChange) {
				currentFolderStack.push(currentFolder);
				currentFolder = currentFolder.children[currentFolder.children.length - 1];
			}
			// Outward
			else {
				const popped = currentFolderStack.pop();
				if (!popped) throw Error("Too many negative indents in the folder structure");
				if (popped) currentFolder = popped;
			}
		}

		return currentFolder;
	}

	function rebuildLayerHierarchy(updateDocumentLayerStructure: DocumentLayerStructure) {
		update((state) => {
			const layerWithNameBeingEdited = state.layers.find((layer: LayerListingInfo) => layer.editingName);
			const layerIdWithNameBeingEdited = layerWithNameBeingEdited?.entry.id;

			// Clear the layer hierarchy before rebuilding it
			state.layers = [];

			// Build the new layer hierarchy
			const recurse = (folder: DocumentLayerStructure) => {
				folder.children.forEach((item, index) => {
					const mapping = state.layerCache.get(String(item.layerId));
					if (mapping) {
						mapping.id = item.layerId;
						state.layers.push({
							folderIndex: index,
							bottomLayer: index === folder.children.length - 1,
							entry: mapping,
							editingName: layerIdWithNameBeingEdited === item.layerId,
						});
					}

					// Call self recursively if there are any children
					if (item.children.length >= 1) recurse(item);
				});
			};
			recurse(updateDocumentLayerStructure);
			return state;
		});
	}

	function updateLayerInTree(targetId: bigint, targetLayer: LayerPanelEntry) {
		update((state) => {
			state.layerCache.set(String(targetId), targetLayer);

			const layer = state.layers.find((layer: LayerListingInfo) => layer.entry.id === targetId);
			if (layer) {
				layer.entry = targetLayer;
			}
			return state;
		});
	}

	editor.subscriptions.subscribeJsMessage(UpdateLayersPanelOptionsLayout, (updateLayersPanelOptionsLayout) => {
		update((state) => {
			patchWidgetLayout(state.layersPanelOptionsLayout, updateLayersPanelOptionsLayout);
			return state;
		});
	});

	editor.subscriptions.subscribeJsMessage(UpdateDocumentLayerStructureJs, (updateDocumentLayerStructure) => {
		const structure = newUpdateDocumentLayerStructure(updateDocumentLayerStructure.dataBuffer);
		rebuildLayerHierarchy(structure);
	});

	editor.subscriptions.subscribeJsMessage(UpdateDocumentLayerDetails, (updateDocumentLayerDetails) => {
		const targetLayer = updateDocumentLayerDetails.data;
		const targetId = targetLayer.id;

		updateLayerInTree(targetId, targetLayer);
	});

	return {
		subscribe,
	};
}
export type LayersState = ReturnType<typeof createLayerState>;

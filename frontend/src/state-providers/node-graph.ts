import { writable } from "svelte/store";

import { type Editor } from "@graphite/editor";
import type { FrontendGraphOutput, FrontendGraphInput } from "@graphite/messages";
import {
	type Box,
	type FrontendClickTargets,
	type ContextMenuInformation,
	type FrontendNode,
	type FrontendNodeType,
	type WirePath,
	ClearAllNodeGraphWirePaths,
	SendUIMetadata,
	UpdateBox,
	UpdateGraphBreadcrumbPath,
	UpdateClickTargets,
	UpdateContextMenuInformation,
	UpdateContextDuringEvaluation,
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
	UpdateThumbnails,
	UpdateWirePathInProgress,
} from "@graphite/messages";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createNodeGraphState(editor: Editor) {
	const { subscribe, update } = writable({
		box: undefined as Box | undefined,
		breadcrumbPath: [] as bigint[],
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
		visibleNodes: new Set<bigint>(),
		/// The first key is the document node id. The index is the actual input index. The exports have a first key value of u32::MAX.
		wires: new Map<bigint, Map<number, WirePath>>(),
		/// The first key is the caller stable node id
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
	editor.subscriptions.subscribeJsMessage(UpdateGraphBreadcrumbPath, (updateGraphBreadcrumbPath) => {
		update((state) => {
			state.breadcrumbPath = updateGraphBreadcrumbPath.breadcrumbPath;
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
	// editor.subscriptions.subscribeJsMessage(UpdateContextDuringEvaluation, (updateContextDuringEvaluation) => {
	// 	update((state) => {
	// 		return state;
	// 	});
	// });
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
					const existing = inputMap.get(wireUpdate.inputIndex);
					if (existing) {
						inputMap.set(wireUpdate.inputIndex, {
							...wireUpdate.wirePathUpdate,
							sni: existing.sni,
						});
					} else {
						inputMap.set(wireUpdate.inputIndex, wireUpdate.wirePathUpdate);
					}
				} else {
					const existing = inputMap.get(wireUpdate.inputIndex);
					if (existing) {
						existing.pathString = "";
					}
				}
			});

			return state;
		});
	});
	editor.subscriptions.subscribeJsMessage(ClearAllNodeGraphWirePaths, (_) => {
		update((state) => {
			for (const [, innerMap] of state.wires) {
				for (const [, wirePath] of innerMap) {
					wirePath.pathString = "";
				}
			}
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
	editor.subscriptions.subscribeJsMessage(UpdateThumbnails, (updateThumbnails) => {
		// console.log("thumbnail update: ", updateThumbnails);
		update((state) => {
			for (const [id, value] of updateThumbnails.add) {
				state.thumbnails.set(id, value);
			}
			for (const id of updateThumbnails.clear) {
				state.thumbnails.set(id, "");
			}
			updateThumbnails.wireSNIUpdates.forEach((wireUpdate) => {
				const inputMap = state.wires.get(wireUpdate.id);
				if (inputMap) {
					const wire = inputMap.get(wireUpdate.inputIndex);
					if (wire) {
						wire.sni = wireUpdate.sni;
					}
				}
			});
			updateThumbnails.layerSNIUpdates.forEach((wireUpdate) => {
				const node = state.nodes.get(wireUpdate.id);
				if (node) {
					node.layerThumbnailSNI = wireUpdate.sni;
				}
			});

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

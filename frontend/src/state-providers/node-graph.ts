import { writable } from "svelte/store";

import type { Editor } from "@graphite/editor";
import type { NodeGraphErrorDiagnostic, BoxSelection, FrontendClickTargets, ContextMenuInformation, FrontendNode, FrontendNodeType, WirePath } from "@graphite/messages";
import type { MessageBody } from "@graphite/subscription-router";

export function createNodeGraphState(editor: Editor) {
	const { subscribe, update } = writable({
		box: undefined as BoxSelection | undefined,
		clickTargets: undefined as FrontendClickTargets | undefined,
		contextMenuInformation: undefined as ContextMenuInformation | undefined,
		error: undefined as NodeGraphErrorDiagnostic | undefined,
		layerWidths: new Map<bigint, number>(),
		chainWidths: new Map<bigint, number>(),
		hasLeftInputWire: new Map<bigint, boolean>(),
		updateImportsExports: undefined as MessageBody<"UpdateImportsExports"> | undefined,
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

	function closeContextMenu() {
		update((state) => {
			state.contextMenuInformation = undefined;
			return state;
		});
	}

	// Set up message subscriptions on creation
	editor.subscriptions.subscribeFrontendMessage("SendUIMetadata", (data) => {
		update((state) => {
			state.nodeDescriptions = new Map(data.nodeDescriptions);
			state.nodeTypes = data.nodeTypes;
			return state;
		});
	});
	editor.subscriptions.subscribeFrontendMessage("UpdateBox", (data) => {
		update((state) => {
			state.box = data.box;
			return state;
		});
	});
	editor.subscriptions.subscribeFrontendMessage("UpdateClickTargets", (data) => {
		update((state) => {
			state.clickTargets = data.clickTargets;
			return state;
		});
	});
	editor.subscriptions.subscribeFrontendMessage("UpdateContextMenuInformation", (data) => {
		update((state) => {
			state.contextMenuInformation = data.contextMenuInformation;
			return state;
		});
	});
	editor.subscriptions.subscribeFrontendMessage("UpdateImportReorderIndex", (data) => {
		update((state) => {
			state.reorderImportIndex = data.importIndex;
			return state;
		});
	});
	editor.subscriptions.subscribeFrontendMessage("UpdateExportReorderIndex", (data) => {
		update((state) => {
			state.reorderExportIndex = data.exportIndex;
			return state;
		});
	});
	editor.subscriptions.subscribeFrontendMessage("UpdateImportsExports", (data) => {
		update((state) => {
			state.updateImportsExports = data;
			return state;
		});
	});
	editor.subscriptions.subscribeFrontendMessage("UpdateInSelectedNetwork", (data) => {
		update((state) => {
			state.inSelectedNetwork = data.inSelectedNetwork;
			return state;
		});
	});
	editor.subscriptions.subscribeFrontendMessage("UpdateLayerWidths", (data) => {
		update((state) => {
			state.layerWidths = data.layerWidths;
			state.chainWidths = data.chainWidths;
			state.hasLeftInputWire = data.hasLeftInputWire;
			return state;
		});
	});
	editor.subscriptions.subscribeFrontendMessage("UpdateNodeGraphNodes", (data) => {
		update((state) => {
			state.nodes.clear();
			data.nodes.forEach((node) => {
				state.nodes.set(node.id, node);
			});
			return state;
		});
	});
	editor.subscriptions.subscribeFrontendMessage("UpdateNodeGraphErrorDiagnostic", (data) => {
		update((state) => {
			state.error = data.error;
			return state;
		});
	});
	editor.subscriptions.subscribeFrontendMessage("UpdateVisibleNodes", (data) => {
		update((state) => {
			state.visibleNodes = new Set<bigint>(data.nodes);
			return state;
		});
	});
	editor.subscriptions.subscribeFrontendMessage("UpdateNodeGraphWires", (data) => {
		update((state) => {
			data.wires.forEach((wireUpdate) => {
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
	editor.subscriptions.subscribeFrontendMessage("ClearAllNodeGraphWires", () => {
		update((state) => {
			state.wires.clear();
			return state;
		});
	});
	editor.subscriptions.subscribeFrontendMessage("UpdateNodeGraphSelection", (data) => {
		update((state) => {
			state.selected = data.selected;
			return state;
		});
	});
	editor.subscriptions.subscribeFrontendMessage("UpdateNodeGraphTransform", (data) => {
		update((state) => {
			state.transform = { scale: data.scale, x: data.translation[0], y: data.translation[1] };
			return state;
		});
	});
	editor.subscriptions.subscribeFrontendMessage("UpdateNodeThumbnail", (data) => {
		update((state) => {
			state.thumbnails.set(data.id, data.value);
			return state;
		});
	});
	editor.subscriptions.subscribeFrontendMessage("UpdateWirePathInProgress", (data) => {
		update((state) => {
			state.wirePathInProgress = data.wirePath;
			return state;
		});
	});

	return {
		subscribe,
		closeContextMenu,
	};
}
export type NodeGraphState = ReturnType<typeof createNodeGraphState>;

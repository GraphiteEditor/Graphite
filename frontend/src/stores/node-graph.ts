import { writable } from "svelte/store";
import type { Writable } from "svelte/store";
import type { SubscriptionsRouter } from "/src/subscriptions-router";
import type { MessageBody } from "/src/subscriptions-router";
import type { NodeGraphErrorDiagnostic, BoxSelection, FrontendClickTargets, ContextMenuInformation, FrontendNode, FrontendNodeType, WirePath } from "/wrapper/pkg/graphite_wasm_wrapper";

export type NodeGraphStore = ReturnType<typeof createNodeGraphStore>;

type NodeGraphStoreState = {
	box: BoxSelection | undefined;
	clickTargets: FrontendClickTargets | undefined;
	contextMenuInformation: ContextMenuInformation | undefined;
	error: NodeGraphErrorDiagnostic | undefined;
	layerWidths: Map<bigint, number>;
	chainWidths: Map<bigint, number>;
	hasLeftInputWire: Map<bigint, boolean>;
	updateImportsExports: MessageBody<"UpdateImportsExports"> | undefined;
	nodes: Map<bigint, FrontendNode>;
	visibleNodes: Set<bigint>;
	/// The index is the exposed input index. The exports have a first key value of u32::MAX.
	wires: Map<bigint, Map<number, WirePath>>;
	wirePathInProgress: WirePath | undefined;
	nodeDescriptions: Map<string, string>;
	nodeTypes: FrontendNodeType[];
	thumbnails: Map<bigint, string>;
	selected: bigint[];
	transform: { scale: number; x: number; y: number };
	inSelectedNetwork: boolean;
	reorderImportIndex: number | undefined;
	reorderExportIndex: number | undefined;
};
const initialState: NodeGraphStoreState = {
	box: undefined,
	clickTargets: undefined,
	contextMenuInformation: undefined,
	error: undefined,
	layerWidths: new Map(),
	chainWidths: new Map(),
	hasLeftInputWire: new Map(),
	updateImportsExports: undefined,
	nodes: new Map(),
	visibleNodes: new Set(),
	wires: new Map(),
	wirePathInProgress: undefined,
	nodeDescriptions: new Map(),
	nodeTypes: [],
	thumbnails: new Map(),
	selected: [],
	transform: { scale: 1, x: 0, y: 0 },
	inSelectedNetwork: true,
	reorderImportIndex: undefined,
	reorderExportIndex: undefined,
};

let subscriptionsRouter: SubscriptionsRouter | undefined = undefined;

// Store state persisted across HMR to maintain reactive subscriptions in the component tree
const store: Writable<NodeGraphStoreState> = import.meta.hot?.data?.store || writable<NodeGraphStoreState>(initialState);
if (import.meta.hot) import.meta.hot.data.store = store;
const { subscribe, update } = store;

export function createNodeGraphStore(subscriptions: SubscriptionsRouter) {
	destroyNodeGraphStore();

	subscriptionsRouter = subscriptions;

	subscriptions.subscribeFrontendMessage("SendUIMetadata", (data) => {
		update((state) => {
			state.nodeDescriptions = new Map(data.nodeDescriptions);
			state.nodeTypes = data.nodeTypes;
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateBox", (data) => {
		update((state) => {
			state.box = data.box;
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateClickTargets", (data) => {
		update((state) => {
			state.clickTargets = data.clickTargets;
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateContextMenuInformation", (data) => {
		update((state) => {
			state.contextMenuInformation = data.contextMenuInformation;
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateImportReorderIndex", (data) => {
		update((state) => {
			state.reorderImportIndex = data.importIndex;
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateExportReorderIndex", (data) => {
		update((state) => {
			state.reorderExportIndex = data.exportIndex;
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateImportsExports", (data) => {
		update((state) => {
			state.updateImportsExports = data;
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateInSelectedNetwork", (data) => {
		update((state) => {
			state.inSelectedNetwork = data.inSelectedNetwork;
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateLayerWidths", (data) => {
		update((state) => {
			state.layerWidths = data.layerWidths;
			state.chainWidths = data.chainWidths;
			state.hasLeftInputWire = data.hasLeftInputWire;
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateNodeGraphNodes", (data) => {
		update((state) => {
			state.nodes.clear();
			data.nodes.forEach((node) => {
				state.nodes.set(node.id, node);
			});
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateNodeGraphErrorDiagnostic", (data) => {
		update((state) => {
			state.error = data.error;
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateVisibleNodes", (data) => {
		update((state) => {
			state.visibleNodes = new Set<bigint>(data.nodes);
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateNodeGraphWires", (data) => {
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

	subscriptions.subscribeFrontendMessage("ClearAllNodeGraphWires", () => {
		update((state) => {
			state.wires.clear();
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateNodeGraphSelection", (data) => {
		update((state) => {
			state.selected = data.selected;
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateNodeGraphTransform", (data) => {
		update((state) => {
			state.transform = { scale: data.scale, x: data.translation[0], y: data.translation[1] };
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateNodeThumbnail", (data) => {
		update((state) => {
			state.thumbnails.set(data.id, data.value);
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateWirePathInProgress", (data) => {
		update((state) => {
			state.wirePathInProgress = data.wirePath;
			return state;
		});
	});

	return { subscribe };
}

export function destroyNodeGraphStore() {
	const subscriptions = subscriptionsRouter;
	if (!subscriptions) return;

	subscriptions.unsubscribeFrontendMessage("SendUIMetadata");
	subscriptions.unsubscribeFrontendMessage("UpdateBox");
	subscriptions.unsubscribeFrontendMessage("UpdateClickTargets");
	subscriptions.unsubscribeFrontendMessage("UpdateContextMenuInformation");
	subscriptions.unsubscribeFrontendMessage("UpdateImportReorderIndex");
	subscriptions.unsubscribeFrontendMessage("UpdateExportReorderIndex");
	subscriptions.unsubscribeFrontendMessage("UpdateImportsExports");
	subscriptions.unsubscribeFrontendMessage("UpdateInSelectedNetwork");
	subscriptions.unsubscribeFrontendMessage("UpdateLayerWidths");
	subscriptions.unsubscribeFrontendMessage("UpdateNodeGraphNodes");
	subscriptions.unsubscribeFrontendMessage("UpdateNodeGraphErrorDiagnostic");
	subscriptions.unsubscribeFrontendMessage("UpdateVisibleNodes");
	subscriptions.unsubscribeFrontendMessage("UpdateNodeGraphWires");
	subscriptions.unsubscribeFrontendMessage("ClearAllNodeGraphWires");
	subscriptions.unsubscribeFrontendMessage("UpdateNodeGraphSelection");
	subscriptions.unsubscribeFrontendMessage("UpdateNodeGraphTransform");
	subscriptions.unsubscribeFrontendMessage("UpdateNodeThumbnail");
	subscriptions.unsubscribeFrontendMessage("UpdateWirePathInProgress");
}

export function closeContextMenu() {
	update((state) => {
		state.contextMenuInformation = undefined;
		return state;
	});
}

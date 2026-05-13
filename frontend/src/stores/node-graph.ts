import { writable } from "svelte/store";
import type { Writable } from "svelte/store";
import type { SubscriptionsRouter } from "/src/subscriptions-router";
import type { MessageBody } from "/src/subscriptions-router";
import type { NodeGraphErrorDiagnostic, BoxSelection, FrontendClickTargets, ContextMenuInformation, FrontendNode, FrontendNodeType, WirePath } from "/wrapper/pkg/graphite_wasm_wrapper";

export type NodeGraphStore = ReturnType<typeof createNodeGraphStore>;

export type NodeGraphTransform = { scale: number; x: number; y: number };

type NodeGraphStoreState = {
	box: BoxSelection | undefined;
	clickTargets: FrontendClickTargets | undefined;
	contextMenuInformation: ContextMenuInformation | undefined;
	error: NodeGraphErrorDiagnostic | undefined;
	layerWidths: Map<bigint, number>;
	chainWidths: Map<bigint, number>;
	hasLeftInputWire: Map<bigint, boolean>;
	nodes: Map<bigint, FrontendNode>;
	wirePathInProgress: WirePath | undefined;
	nodeDescriptions: Map<string, string>;
	nodeTypes: FrontendNodeType[];
	thumbnails: Map<bigint, string>;
	selected: bigint[];
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
	nodes: new Map(),
	wirePathInProgress: undefined,
	nodeDescriptions: new Map(),
	nodeTypes: [],
	thumbnails: new Map(),
	selected: [],
	inSelectedNetwork: true,
	reorderImportIndex: undefined,
	reorderExportIndex: undefined,
};

let subscriptionsRouter: SubscriptionsRouter | undefined = undefined;

// Store state persisted across HMR to maintain reactive subscriptions in the component tree
const store: Writable<NodeGraphStoreState> = import.meta.hot?.data?.store || writable<NodeGraphStoreState>(initialState);
if (import.meta.hot) import.meta.hot.data.store = store;
const { subscribe, update } = store;

// Separate transform store so pan/zoom updates don't trigger re-rendering the entire node graph
const transformStore: Writable<NodeGraphTransform> = import.meta.hot?.data?.transformStore || writable<NodeGraphTransform>({ scale: 1, x: 0, y: 0 });
if (import.meta.hot) import.meta.hot.data.transformStore = transformStore;

// Separate imports/exports store so viewport-anchored position updates don't trigger node re-renders
const importsExportsStore: Writable<MessageBody<"UpdateImportsExports"> | undefined> = import.meta.hot?.data?.importsExportsStore || writable(undefined);
if (import.meta.hot) import.meta.hot.data.importsExportsStore = importsExportsStore;

// Separate visible nodes store so viewport culling changes don't trigger full node re-renders
const visibleNodesStore: Writable<Set<bigint>> = import.meta.hot?.data?.visibleNodesStore || writable(new Set());
if (import.meta.hot) import.meta.hot.data.visibleNodesStore = visibleNodesStore;

// Separate wires store so wire path updates (e.g. export connector movement during pan) don't trigger node re-renders
const wiresStore: Writable<Map<bigint, Map<number, WirePath>>> = import.meta.hot?.data?.wiresStore || writable(new Map());
if (import.meta.hot) import.meta.hot.data.wiresStore = wiresStore;

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
		importsExportsStore.set(data);
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
		const newNodes = new Set<bigint>(data.nodes);

		// Short-circuit when the visible set hasn't changed to avoid unnecessary re-renders
		let changed = false;
		const unsubscribe = visibleNodesStore.subscribe((current) => {
			if (current.size !== newNodes.size) {
				changed = true;
			} else {
				newNodes.forEach((node) => {
					if (!current.has(node)) changed = true;
				});
			}
		});
		unsubscribe();

		if (!changed) return;

		visibleNodesStore.set(newNodes);
	});

	subscriptions.subscribeFrontendMessage("UpdateNodeGraphWires", (data) => {
		if (data.wires.length === 0) return;

		wiresStore.update((wires) => {
			data.wires.forEach((wireUpdate) => {
				let inputMap = wires.get(wireUpdate.id);
				if (!inputMap) {
					inputMap = new Map();
					wires.set(wireUpdate.id, inputMap);
				}
				if (wireUpdate.wirePathUpdate !== undefined) {
					inputMap.set(wireUpdate.inputIndex, wireUpdate.wirePathUpdate);
				} else {
					inputMap.delete(wireUpdate.inputIndex);
				}
			});
			return wires;
		});
	});

	subscriptions.subscribeFrontendMessage("ClearAllNodeGraphWires", () => {
		wiresStore.set(new Map());
	});

	subscriptions.subscribeFrontendMessage("UpdateNodeGraphSelection", (data) => {
		update((state) => {
			state.selected = data.selected;
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateNodeGraphTransform", (data) => {
		transformStore.set({ scale: data.scale, x: data.translation[0], y: data.translation[1] });
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

	return { subscribe, transformStore, importsExportsStore, visibleNodesStore, wiresStore };
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

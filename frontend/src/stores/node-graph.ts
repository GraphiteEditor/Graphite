import { get, writable } from "svelte/store";
import type { Writable } from "svelte/store";
import type { IconName } from "/src/icons";
import type { SubscriptionsRouter } from "/src/subscriptions-router";
import type { MessageBody } from "/src/subscriptions-router";
import type {
	NodeGraphErrorDiagnostic,
	BoxSelection,
	FrontendClickTargets,
	ContextMenuInformation,
	FrontendNode,
	FrontendNodeType,
	FrontendRemoteCursor,
	WirePath,
} from "/wrapper/pkg/graphite_wasm_wrapper";

export type NodeGraphStore = ReturnType<typeof createNodeGraphStore>;

export type NodeGraphTransform = { scale: number; x: number; y: number };

export type ThumbnailCanvases = { layersPanelCanvasId: number; nodeGraphCanvasId: number };

type NodeGraphStoreState = {
	box: BoxSelection | undefined;
	clickTargets: FrontendClickTargets | undefined;
	contextMenuInformation: ContextMenuInformation | undefined;
	errors: NodeGraphErrorDiagnostic[];
	layerWidths: Map<bigint, number>;
	chainWidths: Map<bigint, number>;
	hasLeftInputWire: Map<bigint, boolean>;
	nodes: Map<bigint, FrontendNode>;
	wirePathInProgress: WirePath | undefined;
	wireInProgressDraggedFrom: [number, number] | undefined;
	wireInProgressSnappedTo: [number, number] | undefined;
	hoveredConnector: [number, number] | undefined;
	nodeDescriptions: Map<string, string>;
	nodeIcons: Map<string, IconName>;
	nodeTypes: FrontendNodeType[];
	thumbnailCanvases: Map<bigint, ThumbnailCanvases>;
	selected: bigint[];
	inSelectedNetwork: boolean;
	reorderImportIndex: number | undefined;
	reorderExportIndex: number | undefined;
};
const initialState: NodeGraphStoreState = {
	box: undefined,
	clickTargets: undefined,
	contextMenuInformation: undefined,
	errors: [],
	layerWidths: new Map(),
	chainWidths: new Map(),
	hasLeftInputWire: new Map(),
	nodes: new Map(),
	wirePathInProgress: undefined,
	wireInProgressDraggedFrom: undefined,
	wireInProgressSnappedTo: undefined,
	hoveredConnector: undefined,
	nodeDescriptions: new Map(),
	nodeIcons: new Map(),
	nodeTypes: [],
	thumbnailCanvases: new Map(),
	selected: [],
	inSelectedNetwork: true,
	reorderImportIndex: undefined,
	reorderExportIndex: undefined,
};

let subscriptionsRouter: SubscriptionsRouter | undefined = undefined;

// Undoes the graph runtime's encoding of thumbnail pixels: pairs of little-endian u32 counts of skipped transparent pixels and kept pixels, each followed by the kept pixels' RGBA bytes
function decodeTransparentRuns(encoded: Uint8Array, width: number, height: number): ImageData {
	const pixels = new Uint8ClampedArray(width * height * 4);
	const view = new DataView(encoded.buffer, encoded.byteOffset, encoded.byteLength);

	let read = 0;
	let pixel = 0;
	while (read + 8 <= encoded.byteLength) {
		pixel += view.getUint32(read, true);
		const kept = view.getUint32(read + 4, true);
		read += 8;

		pixels.set(encoded.subarray(read, read + kept * 4), pixel * 4);
		read += kept * 4;
		pixel += kept;
	}

	return new ImageData(pixels, width, height);
}

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

// Kept apart from the node graph state so a thumbnail update doesn't re-render every node
const thumbnailImagesStore: Writable<Map<bigint, ImageData>> = import.meta.hot?.data?.thumbnailImagesStore || writable(new Map());
if (import.meta.hot) import.meta.hot.data.thumbnailImagesStore = thumbnailImagesStore;

// Separate visible nodes store so viewport culling changes don't trigger full node re-renders
const visibleNodesStore: Writable<Set<bigint>> = import.meta.hot?.data?.visibleNodesStore || writable(new Set());
if (import.meta.hot) import.meta.hot.data.visibleNodesStore = visibleNodesStore;

// Separate wires store so wire path updates (e.g. export connector movement during pan) don't trigger node re-renders.
// It only changes when wires are added or removed, since each wire's path is kept in its own store so updating it only re-renders that wire.
const wiresStore: Writable<Map<bigint, Map<number, Writable<WirePath>>>> = import.meta.hot?.data?.wiresStore || writable(new Map());
if (import.meta.hot) import.meta.hot.data.wiresStore = wiresStore;

// Separate wire insertion preview store so the wires don't re-render with every node graph update
const wireInsertionPreviewStore: Writable<[bigint, number] | undefined> = import.meta.hot?.data?.wireInsertionPreviewStore || writable(undefined);
if (import.meta.hot) import.meta.hot.data.wireInsertionPreviewStore = wireInsertionPreviewStore;

// Separate glide offsets store so the per-frame offsets of whatever is gliding into the place it moved to don't trigger full node re-renders
export type GlideOffsets = { nodes: Map<bigint, [number, number]>; imports: [number, number]; exports: [number, number] };
const glideOffsetsStore: Writable<GlideOffsets> = import.meta.hot?.data?.glideOffsetsStore || writable<GlideOffsets>({ nodes: new Map(), imports: [0, 0], exports: [0, 0] });
if (import.meta.hot) import.meta.hot.data.glideOffsetsStore = glideOffsetsStore;

// Kept apart from the main store since cursors move on every frame, and only the overlay draws them
const remoteCursorsStore: Writable<FrontendRemoteCursor[]> = import.meta.hot?.data?.remoteCursorsStore || writable<FrontendRemoteCursor[]>([]);
if (import.meta.hot) import.meta.hot.data.remoteCursorsStore = remoteCursorsStore;

export function createNodeGraphStore(subscriptions: SubscriptionsRouter) {
	destroyNodeGraphStore();

	subscriptionsRouter = subscriptions;

	subscriptions.subscribeFrontendMessage("SendUIMetadata", (data) => {
		update((state) => {
			state.nodeDescriptions = new Map(data.nodeDescriptions);
			state.nodeIcons = new Map(data.nodeIcons);
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

	subscriptions.subscribeFrontendMessage("UpdateNodeGraphCursors", (data) => {
		remoteCursorsStore.set(data.cursors);
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

	subscriptions.subscribeFrontendMessage("UpdateWireInsertionPreview", (data) => {
		wireInsertionPreviewStore.set(data.wire);
	});

	subscriptions.subscribeFrontendMessage("UpdateHoveredConnector", (data) => {
		update((state) => {
			state.hoveredConnector = data.center;
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

	subscriptions.subscribeFrontendMessage("UpdateNodeGraphErrorDiagnostics", (data) => {
		update((state) => {
			state.errors = data.errors;
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

	subscriptions.subscribeFrontendMessage("UpdateGlideOffsets", (data) => {
		glideOffsetsStore.set({ nodes: new Map(data.nodes), imports: data.imports, exports: data.exports });
	});

	subscriptions.subscribeFrontendMessage("UpdateNodeGraphWires", (data) => {
		const wires = get(wiresStore);
		let wiresChanged = false;

		data.wires.forEach(({ id, inputIndex, wirePathUpdate }) => {
			let inputMap = wires.get(id);
			const wire = inputMap?.get(inputIndex);

			if (wirePathUpdate === undefined) {
				if (inputMap?.delete(inputIndex)) wiresChanged = true;
			} else if (wire) {
				wire.set(wirePathUpdate);
			} else {
				if (!inputMap) {
					inputMap = new Map();
					wires.set(id, inputMap);
				}
				inputMap.set(inputIndex, writable(wirePathUpdate));
				wiresChanged = true;
			}
		});

		if (wiresChanged) wiresStore.set(wires);
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

	subscriptions.subscribeFrontendMessage("UpdateNodeThumbnailCanvases", (data) => {
		const { id, ...canvases } = data;

		const current = get(store).thumbnailCanvases.get(id);
		if (current?.layersPanelCanvasId === canvases.layersPanelCanvasId && current?.nodeGraphCanvasId === canvases.nodeGraphCanvasId) return;

		update((state) => {
			state.thumbnailCanvases.set(id, canvases);
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateNodeThumbnailPixels", (data) => {
		const image = decodeTransparentRuns(data.pixels, data.width, data.height);
		thumbnailImagesStore.update((images) => images.set(data.id, image));
	});

	subscriptions.subscribeFrontendMessage("ClearNodeThumbnails", (data) => {
		update((state) => {
			data.ids.forEach((id) => state.thumbnailCanvases.delete(id));
			return state;
		});
		thumbnailImagesStore.update((images) => {
			data.ids.forEach((id) => images.delete(id));
			return images;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateWirePathInProgress", (data) => {
		update((state) => {
			state.wirePathInProgress = data.wirePath;
			state.wireInProgressDraggedFrom = data.draggedFrom;
			state.wireInProgressSnappedTo = data.snappedTo;
			return state;
		});
	});

	return { subscribe, transformStore, importsExportsStore, visibleNodesStore, thumbnailImagesStore, wiresStore, wireInsertionPreviewStore, glideOffsetsStore, remoteCursorsStore };
}

export function destroyNodeGraphStore() {
	const subscriptions = subscriptionsRouter;
	if (!subscriptions) return;

	subscriptions.unsubscribeFrontendMessage("SendUIMetadata");
	subscriptions.unsubscribeFrontendMessage("UpdateBox");
	subscriptions.unsubscribeFrontendMessage("UpdateNodeGraphCursors");
	subscriptions.unsubscribeFrontendMessage("UpdateClickTargets");
	subscriptions.unsubscribeFrontendMessage("UpdateContextMenuInformation");
	subscriptions.unsubscribeFrontendMessage("UpdateImportReorderIndex");
	subscriptions.unsubscribeFrontendMessage("UpdateExportReorderIndex");
	subscriptions.unsubscribeFrontendMessage("UpdateWireInsertionPreview");
	subscriptions.unsubscribeFrontendMessage("UpdateHoveredConnector");
	subscriptions.unsubscribeFrontendMessage("UpdateImportsExports");
	subscriptions.unsubscribeFrontendMessage("UpdateInSelectedNetwork");
	subscriptions.unsubscribeFrontendMessage("UpdateLayerWidths");
	subscriptions.unsubscribeFrontendMessage("UpdateNodeGraphNodes");
	subscriptions.unsubscribeFrontendMessage("UpdateNodeGraphErrorDiagnostics");
	subscriptions.unsubscribeFrontendMessage("UpdateVisibleNodes");
	subscriptions.unsubscribeFrontendMessage("UpdateGlideOffsets");
	subscriptions.unsubscribeFrontendMessage("UpdateNodeGraphWires");
	subscriptions.unsubscribeFrontendMessage("ClearAllNodeGraphWires");
	subscriptions.unsubscribeFrontendMessage("UpdateNodeGraphSelection");
	subscriptions.unsubscribeFrontendMessage("UpdateNodeGraphTransform");
	subscriptions.unsubscribeFrontendMessage("UpdateNodeThumbnailCanvases");
	subscriptions.unsubscribeFrontendMessage("UpdateNodeThumbnailPixels");
	subscriptions.unsubscribeFrontendMessage("ClearNodeThumbnails");
	subscriptions.unsubscribeFrontendMessage("UpdateWirePathInProgress");
}

export function closeContextMenu() {
	update((state) => {
		state.contextMenuInformation = undefined;
		return state;
	});
}

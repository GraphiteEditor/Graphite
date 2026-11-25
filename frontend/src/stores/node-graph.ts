import { get, writable } from "svelte/store";
import type { Writable } from "svelte/store";
import type { SubscriptionsRouter } from "/src/subscriptions-router";
import type { ContextMenuInformation, FrontendNodeType } from "/wrapper/pkg/graphite_wasm_wrapper";

export type NodeGraphStore = ReturnType<typeof createNodeGraphStore>;

export type NodeGraphTransform = { scale: number; x: number; y: number };

type NodeGraphStoreState = {
	contextMenuInformation: ContextMenuInformation | undefined;
	nodeDescriptions: Map<string, string>;
	nodeTypes: FrontendNodeType[];
	thumbnailCanvases: Map<bigint, number>;
	selected: bigint[];
	readOnly: boolean;
};
const initialState: NodeGraphStoreState = {
	contextMenuInformation: undefined,
	nodeDescriptions: new Map(),
	nodeTypes: [],
	thumbnailCanvases: new Map(),
	selected: [],
	readOnly: false,
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

// Separate transform store so a pan or zoom re-renders only the menus and fields placed in graph space
const transformStore: Writable<NodeGraphTransform> = import.meta.hot?.data?.transformStore || writable<NodeGraphTransform>({ scale: 1, x: 0, y: 0 });
if (import.meta.hot) import.meta.hot.data.transformStore = transformStore;

// Kept apart from the node graph state so a thumbnail update doesn't re-render every row of the Layers panel
const thumbnailImagesStore: Writable<Map<bigint, ImageData>> = import.meta.hot?.data?.thumbnailImagesStore || writable(new Map());
if (import.meta.hot) import.meta.hot.data.thumbnailImagesStore = thumbnailImagesStore;

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

	subscriptions.subscribeFrontendMessage("UpdateContextMenuInformation", (data) => {
		update((state) => {
			state.contextMenuInformation = data.contextMenuInformation;
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateNetworkReadOnly", (data) => {
		update((state) => {
			state.readOnly = data.readOnly;
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
		transformStore.set({ scale: data.scale, x: data.translation[0], y: data.translation[1] });
	});

	subscriptions.subscribeFrontendMessage("UpdateNodeThumbnailCanvas", (data) => {
		if (get(store).thumbnailCanvases.get(data.id) === data.canvasId) return;

		update((state) => {
			state.thumbnailCanvases.set(data.id, data.canvasId);
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

	return { subscribe, transformStore, thumbnailImagesStore };
}

export function destroyNodeGraphStore() {
	const subscriptions = subscriptionsRouter;
	if (!subscriptions) return;

	subscriptions.unsubscribeFrontendMessage("SendUIMetadata");
	subscriptions.unsubscribeFrontendMessage("UpdateContextMenuInformation");
	subscriptions.unsubscribeFrontendMessage("UpdateNetworkReadOnly");
	subscriptions.unsubscribeFrontendMessage("UpdateNodeGraphSelection");
	subscriptions.unsubscribeFrontendMessage("UpdateNodeGraphTransform");
	subscriptions.unsubscribeFrontendMessage("UpdateNodeThumbnailCanvas");
	subscriptions.unsubscribeFrontendMessage("UpdateNodeThumbnailPixels");
	subscriptions.unsubscribeFrontendMessage("ClearNodeThumbnails");
}

export function closeContextMenu() {
	update((state) => {
		state.contextMenuInformation = undefined;
		return state;
	});
}

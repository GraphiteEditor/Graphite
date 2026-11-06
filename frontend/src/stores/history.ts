import { writable } from "svelte/store";
import type { Writable } from "svelte/store";
import type { SubscriptionsRouter } from "/src/subscriptions-router";
import type { HistoryPanelState } from "/wrapper/pkg/graphite_wasm_wrapper";

export type HistoryStore = ReturnType<typeof createHistoryStore>;

const initialState: HistoryPanelState = { rows: [], progress: [], more: false, following: undefined, previewing: undefined, collaborating: false };

// Store state persisted across HMR to maintain reactive subscriptions in the component tree
const store: Writable<HistoryPanelState> = import.meta.hot?.data?.store || writable<HistoryPanelState>(initialState);
// The captured images by the ID rows name them with, sent once each
const thumbnails: Writable<Record<string, string>> = import.meta.hot?.data?.thumbnails || writable<Record<string, string>>({});
if (import.meta.hot) {
	import.meta.hot.data.store = store;
	import.meta.hot.data.thumbnails = thumbnails;
}

const { subscribe, set } = store;

// The most recent captures kept, oldest dropped first
const MAX_THUMBNAILS = 300;
// Twice the size of the comparison view, which is the largest a capture is shown
const RASTER_WIDTH = 240;
const RASTER_HEIGHT = 160;

// A capture's SVG can embed whole images, so it is kept as a small PNG, or as the SVG when the browser can't draw it
async function rasterize(svg: string): Promise<string> {
	// An SVG drawn as an image needs an intrinsic size, which the capture's only gives as a view box
	const sized = svg.replace("<svg ", `<svg width="${RASTER_WIDTH}" height="${RASTER_HEIGHT}" `);
	const url = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(sized)}`;
	try {
		const image = new Image();
		image.src = url;
		await image.decode();
		const canvas = document.createElement("canvas");
		canvas.width = RASTER_WIDTH;
		canvas.height = RASTER_HEIGHT;
		canvas.getContext("2d")?.drawImage(image, 0, 0, RASTER_WIDTH, RASTER_HEIGHT);
		return canvas.toDataURL("image/png");
	} catch {
		return url;
	}
}

let subscriptionsRouter: SubscriptionsRouter | undefined = undefined;

export function createHistoryStore(subscriptions: SubscriptionsRouter) {
	destroyHistoryStore();
	subscriptionsRouter = subscriptions;

	subscriptions.subscribeFrontendMessage("UpdateHistoryPanel", (data) => {
		set(data.state);
	});

	subscriptions.subscribeFrontendMessage("UpdateHistoryThumbnail", async (data) => {
		const url = await rasterize(data.svg);
		thumbnails.update((images) => {
			const ids = Object.keys(images);
			const kept = ids.length < MAX_THUMBNAILS ? images : Object.fromEntries(ids.slice(ids.length - MAX_THUMBNAILS + 1).map((id) => [id, images[id]]));
			return { ...kept, [data.id]: url };
		});
	});

	return { subscribe, thumbnails: { subscribe: thumbnails.subscribe } };
}

export function destroyHistoryStore() {
	const subscriptions = subscriptionsRouter;
	if (!subscriptions) return;

	subscriptions.unsubscribeFrontendMessage("UpdateHistoryPanel");
	subscriptions.unsubscribeFrontendMessage("UpdateHistoryThumbnail");
	subscriptionsRouter = undefined;
}

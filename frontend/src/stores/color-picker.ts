import { writable } from "svelte/store";
import type { Writable } from "svelte/store";
import type { SubscriptionsRouter } from "/src/subscriptions-router";
import { patchLayout } from "/src/utility-functions/widgets";
import type { FillChoice, Layout } from "/wrapper/pkg/graphite_wasm_wrapper";

export type ColorPickerCallbacks = {
	onColorChanged?: (value: FillChoice) => void;
	onStartTransaction?: () => void;
	onCommitTransaction?: () => void;
};

export type ColorPickerStoreState = {
	pickersAndGradient: Layout;
	details: Layout;
	callbacks: ColorPickerCallbacks;
	// True while the user is actively dragging one of the visual H/S/V/A pickers, so the popover knows to suppress its stray-pointer-close behavior until the drag ends.
	isDragging: boolean;
};

const initialState: ColorPickerStoreState = {
	pickersAndGradient: [],
	details: [],
	callbacks: {},
	isDragging: false,
};

let subscriptionsRouter: SubscriptionsRouter | undefined = undefined;

// Persist the store across HMR so subscriptions stay live.
const store: Writable<ColorPickerStoreState> = import.meta.hot?.data?.store || writable<ColorPickerStoreState>(initialState);
if (import.meta.hot) import.meta.hot.data.store = store;
const { subscribe, update } = store;

export type ColorPickerStore = {
	subscribe: typeof subscribe;
	setCallbacks: (callbacks: ColorPickerCallbacks) => void;
	clearCallbacks: () => void;
	setDragging: (dragging: boolean) => void;
};

// The Rust handler keeps a single shared layout per target, but multiple `<ColorPicker>` Svelte instances may be mounted across
// the app (one per `ColorInput`/`WorkingColorsInput`/etc.). Subscribing to the layout target from each instance is destructive,
// only the last-registered callback wins. So we maintain a single global subscription here and let each `<ColorPicker>` instance
// read from the resulting store and register its own per-open callbacks for color/transaction events.
export function createColorPickerStore(subscriptions: SubscriptionsRouter): ColorPickerStore {
	destroyColorPickerStore();

	subscriptionsRouter = subscriptions;

	subscriptions.subscribeFrontendMessage("ColorPickerColorChanged", (data) => {
		update((state) => {
			state.callbacks.onColorChanged?.(data.value);
			return state;
		});
	});
	subscriptions.subscribeFrontendMessage("ColorPickerStartHistoryTransaction", () => {
		update((state) => {
			state.callbacks.onStartTransaction?.();
			return state;
		});
	});
	subscriptions.subscribeFrontendMessage("ColorPickerCommitHistoryTransaction", () => {
		update((state) => {
			state.callbacks.onCommitTransaction?.();
			return state;
		});
	});

	subscriptions.subscribeLayoutUpdate("ColorPickerPickersAndGradient", (diffs) => {
		update((state) => {
			patchLayout(state.pickersAndGradient, diffs);
			return state;
		});
	});
	subscriptions.subscribeLayoutUpdate("ColorPickerDetails", (diffs) => {
		update((state) => {
			patchLayout(state.details, diffs);
			return state;
		});
	});

	return {
		subscribe,
		setCallbacks: (callbacks: ColorPickerCallbacks) => {
			update((state) => {
				state.callbacks = callbacks;
				return state;
			});
		},
		clearCallbacks: () => {
			update((state) => {
				state.callbacks = {};
				return state;
			});
		},
		setDragging: (dragging: boolean) => {
			update((state) => {
				state.isDragging = dragging;
				return state;
			});
		},
	};
}

export function destroyColorPickerStore() {
	const subscriptions = subscriptionsRouter;
	if (!subscriptions) return;

	subscriptions.unsubscribeFrontendMessage("ColorPickerColorChanged");
	subscriptions.unsubscribeFrontendMessage("ColorPickerStartHistoryTransaction");
	subscriptions.unsubscribeFrontendMessage("ColorPickerCommitHistoryTransaction");
	subscriptions.unsubscribeLayoutUpdate("ColorPickerPickersAndGradient");
	subscriptions.unsubscribeLayoutUpdate("ColorPickerDetails");
}

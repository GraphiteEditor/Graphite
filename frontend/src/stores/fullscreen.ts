import { get, writable } from "svelte/store";
import type { Writable } from "svelte/store";
import type { SubscriptionsRouter } from "/src/subscriptions-router";

export type FullscreenStore = ReturnType<typeof createFullscreenStore>;

type FullscreenStoreState = {
	windowFullscreen: boolean;
	keyboardLocked: boolean;
};
const initialState: FullscreenStoreState = {
	windowFullscreen: false,
	keyboardLocked: false,
};

let subscriptionsRouter: SubscriptionsRouter | undefined = undefined;

// Store state persisted across HMR to maintain reactive subscriptions in the component tree
const store: Writable<FullscreenStoreState> = import.meta.hot?.data?.store || writable<FullscreenStoreState>(initialState);
if (import.meta.hot) import.meta.hot.data.store = store;
const { subscribe, update } = store;

export function createFullscreenStore(subscriptions: SubscriptionsRouter) {
	destroyFullscreenStore();

	subscriptionsRouter = subscriptions;

	subscriptions.subscribeFrontendMessage("WindowFullscreen", () => {
		toggleFullscreen();
	});

	return { subscribe };
}

export function destroyFullscreenStore() {
	const subscriptions = subscriptionsRouter;
	if (!subscriptions) return;

	subscriptions.unsubscribeFrontendMessage("WindowFullscreen");
}

export function fullscreenModeChanged() {
	update((state) => {
		state.windowFullscreen = Boolean(document.fullscreenElement);
		if (!state.windowFullscreen) state.keyboardLocked = false;
		return state;
	});
}

export async function enterFullscreen() {
	await document.documentElement.requestFullscreen();

	const keyboardLockApiSupported = navigator.keyboard !== undefined && "lock" in navigator.keyboard;

	if (keyboardLockApiSupported && navigator.keyboard) {
		await navigator.keyboard.lock(["ControlLeft", "ControlRight"]);

		update((state) => {
			state.keyboardLocked = true;
			return state;
		});
	}
}

export async function exitFullscreen() {
	await document.exitFullscreen();
}

export async function toggleFullscreen() {
	const state = get(store);
	if (state.windowFullscreen) await exitFullscreen();
	else await enterFullscreen();
}

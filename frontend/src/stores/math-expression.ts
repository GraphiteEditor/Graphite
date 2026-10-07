import { writable } from "svelte/store";
import type { Writable } from "svelte/store";
import type { SubscriptionsRouter } from "/src/subscriptions-router";
import type { MathExpressionCompletions, MathExpressionToken, MathExpressionTooltip } from "/wrapper/pkg/graphite_wasm_wrapper";

// The backend's reply to the text a math expression widget last sent it, which the widget displays only once its tokens arrive
export type MathExpressionAnalysis = { source: string; tokens: MathExpressionToken[]; tooltips: MathExpressionTooltip[]; completions: MathExpressionCompletions | undefined };

// The backend's reply to math typed in a math expression widget's number popover, with its value written as a number where it has one
export type MathExpressionEvaluation = { source: string; result: string | undefined };

// Each widget's latest reply of each kind, by its widget ID
export type MathExpressionStoreState = {
	analyses: Map<string, MathExpressionAnalysis>;
	evaluations: Map<string, MathExpressionEvaluation>;
};

const initialState: MathExpressionStoreState = {
	analyses: new Map(),
	evaluations: new Map(),
};

let subscriptionsRouter: SubscriptionsRouter | undefined = undefined;

// Persist the store across HMR so subscriptions stay live.
const store: Writable<MathExpressionStoreState> = import.meta.hot?.data?.store || writable<MathExpressionStoreState>(initialState);
if (import.meta.hot) import.meta.hot.data.store = store;
const { subscribe, update } = store;

export type MathExpressionStore = {
	subscribe: typeof subscribe;
	// Drops a widget's replies once it's gone
	forget: (widgetId: string) => void;
};

export function createMathExpressionStore(subscriptions: SubscriptionsRouter): MathExpressionStore {
	destroyMathExpressionStore();

	subscriptionsRouter = subscriptions;

	subscriptions.subscribeFrontendMessage("UpdateMathExpressionAnalysis", (data) => {
		update((state) => {
			state.analyses.set(String(data.widgetId), { source: data.source, tokens: data.tokens, tooltips: data.tooltips, completions: data.completions });
			return state;
		});
	});

	subscriptions.subscribeFrontendMessage("UpdateMathExpressionEvaluation", (data) => {
		update((state) => {
			state.evaluations.set(String(data.widgetId), { source: data.source, result: data.result });
			return state;
		});
	});

	return {
		subscribe,
		forget: (widgetId: string) => {
			update((state) => {
				state.analyses.delete(widgetId);
				state.evaluations.delete(widgetId);
				return state;
			});
		},
	};
}

export function destroyMathExpressionStore() {
	const subscriptions = subscriptionsRouter;
	if (!subscriptions) return;

	subscriptions.unsubscribeFrontendMessage("UpdateMathExpressionAnalysis");
	subscriptions.unsubscribeFrontendMessage("UpdateMathExpressionEvaluation");
}

import type { FrontendMessage, LayoutTarget, WidgetDiff } from "@graphite/../wasm/pkg/graphite_wasm";

// Type convert a union of messages into a map of messages
export type ToMessageMap<T> = {
	[K in T extends string ? T : T extends object ? keyof T : never]: K extends T ? Record<string, never> : T extends Record<K, infer Payload> ? Payload : never;
};

export type MessageMap = ToMessageMap<FrontendMessage>;
export type MessageName = keyof MessageMap;
export type MessageBody<T extends MessageName> = Extract<FrontendMessage, Record<T, unknown>>[T];

export function createSubscriptionRouter() {
	// Callbacks are wrapped at subscription time to capture their type-specific data extraction in a closure,
	// so the stored function has a uniform signature and the map doesn't need per-key generic value types.
	const subscriptions: Partial<Record<MessageName, (taggedMessage: MessageMap) => void>> = {};
	const layoutCallbacks: Partial<Record<LayoutTarget, (diffs: WidgetDiff[]) => void>> = {};

	const subscribeFrontendMessage = <T extends MessageName>(messageType: T, callback: (data: MessageMap[T]) => void) => {
		subscriptions[messageType] = (taggedMessage: MessageMap) => callback(taggedMessage[messageType]);
	};

	const unsubscribeFrontendMessage = (messageType: MessageName) => {
		delete subscriptions[messageType];
	};

	const subscribeLayoutUpdate = (target: LayoutTarget, callback: (diffs: WidgetDiff[]) => void) => {
		layoutCallbacks[target] = callback;
	};

	const unsubscribeLayoutUpdate = (target: LayoutTarget) => {
		delete layoutCallbacks[target];
	};

	function normalizeMessage<T extends string | object>(message: T): ToMessageMap<T>;
	function normalizeMessage(message: string | Record<string, unknown>): Record<string, unknown> {
		// If it's a bare string, convert it to an object with an empty payload
		if (typeof message === "string") {
			const result: Record<string, Record<string, never>> = { [message]: {} };
			return result;
		}

		// If it's already an object, it matches the structure of our map
		return message;
	}

	const handleFrontendMessage = (messageType: MessageName, messageData: FrontendMessage) => {
		// Messages with non-empty data are provided by Serde JSON as an object with one key as the message name, like: { NameOfThisMessage: { ... } }
		// Messages with empty data are provided by Serde JSON as a string with the message name, like: "NameOfThisMessage"
		// Here we extract the payload object or create an empty payload object, as needed.
		const taggedMessage = normalizeMessage(messageData);

		// Resolve the dispatch thunk, depending on whether this is a layout update or a regular message.
		// UpdateLayout messages are dispatched to layout-specific callbacks based on the layout target.
		// The thunk is re-evaluated on each retry because the callback may not be registered yet.
		let getHandler: () => ((taggedMessage: MessageMap) => void) | undefined = () => subscriptions[messageType];

		// Handle layout updates specially to route them to layout-specific callbacks and extract the diffs as the data to pass
		let target: LayoutTarget | undefined;
		if ("UpdateLayout" in taggedMessage) {
			const { layoutTarget, diff } = taggedMessage["UpdateLayout"];
			target = layoutTarget;

			getHandler = () => {
				const layoutCallback = layoutCallbacks[layoutTarget];
				if (!layoutCallback) return undefined;
				return () => layoutCallback(diff);
			};
		}

		// Try to execute the callback. Due to message ordering, the callback may not be registered yet,
		// so we retry a few times on the next stack frame to give onMount a chance to run.
		let retries = 0;
		const callCallback = () => {
			const handler = getHandler();

			if (handler) {
				handler(taggedMessage);
			} else if (retries <= 3) {
				retries += 1;
				setTimeout(callCallback, 0);
			} else {
				// eslint-disable-next-line no-console
				console.error(`Received a frontend message of type ${messageType}${target ? ` (${target})` : ""} but no handler was registered for it from the client.`);
			}
		};

		callCallback();
	};

	return {
		subscribeFrontendMessage,
		unsubscribeFrontendMessage,
		subscribeLayoutUpdate,
		unsubscribeLayoutUpdate,
		handleFrontendMessage,
	};
}
export type SubscriptionRouter = ReturnType<typeof createSubscriptionRouter>;

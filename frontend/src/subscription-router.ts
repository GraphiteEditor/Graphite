import type { FrontendMessages, LayoutTarget, WidgetDiff } from "@graphite/messages";
import { parseWidgetDiffs } from "@graphite/utility-functions/widgets";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type FrontendMessageCallbacks = Record<string, ((messageData: any) => void) | undefined>;

export function createSubscriptionRouter() {
	const subscriptions: FrontendMessageCallbacks = {};
	const layoutCallbacks: Partial<Record<LayoutTarget, (diffs: WidgetDiff[]) => void>> = {};

	const subscribeFrontendMessage = <T extends keyof FrontendMessages>(messageType: T, callback: (data: FrontendMessages[T]) => void) => {
		subscriptions[messageType] = callback;
	};

	const unsubscribeFrontendMessage = (messageType: keyof FrontendMessages) => {
		delete subscriptions[messageType];
	};

	const subscribeLayoutUpdate = (target: LayoutTarget, callback: (diffs: WidgetDiff[]) => void) => {
		layoutCallbacks[target] = callback;
	};

	const unsubscribeLayoutUpdate = (target: LayoutTarget) => {
		delete layoutCallbacks[target];
	};

	const handleFrontendMessage = (messageType: keyof FrontendMessages, messageData: Record<string, unknown>) => {
		// Messages with non-empty data are provided by Serde JSON as an object with one key as the message name, like: { NameOfThisMessage: { ... } }
		// Messages with empty data are provided by Serde JSON as a string with the message name, like: "NameOfThisMessage"
		// Here we extract the payload object or use an empty object depending on the situation.
		const message = messageData[messageType] || {};

		// Resolve the callback lookup and the data to pass, depending on whether this is a layout update or a regular message.
		// UpdateLayout messages are dispatched to layout-specific callbacks based on the layout target.
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		let getCallback: () => ((data: any) => void) | undefined;
		let callbackData: unknown;
		let errorLabel: string;
		if (messageType === "UpdateLayout") {
			const { layoutTarget, diff } = message as FrontendMessages["UpdateLayout"];
			getCallback = () => layoutCallbacks[layoutTarget];
			callbackData = parseWidgetDiffs(diff);
			errorLabel = `UpdateLayout for layout target "${layoutTarget}"`;
		} else {
			getCallback = () => subscriptions[messageType];
			callbackData = message;
			errorLabel = messageType;
		}

		// Try to execute the callback. Due to message ordering, the callback may not be registered yet,
		// so we retry a few times on the next stack frame to give onMount a chance to run.
		let retries = 0;
		const callCallback = () => {
			const callback = getCallback();

			if (callback) {
				callback(callbackData);
			} else if (retries <= 3) {
				retries += 1;
				setTimeout(callCallback, 0);
			} else {
				// eslint-disable-next-line no-console
				console.error(`Received a frontend message of type "${errorLabel}" but no handler was registered for it from the client.`);
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

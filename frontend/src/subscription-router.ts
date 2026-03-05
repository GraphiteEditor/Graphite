import { type EditorHandle } from "@graphite/../wasm/pkg/graphite_wasm";
import { type JsMessageType, type JsMessageTypeMap, type LayoutTarget, type WidgetDiff, messageMakers, parseWidgetDiffs } from "@graphite/messages";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type JsMessageCallbackMap = Record<string, ((messageData: any) => void) | undefined>;

export function createSubscriptionRouter() {
	const subscriptions: JsMessageCallbackMap = {};
	const layoutCallbacks: Partial<Record<LayoutTarget, (diffs: WidgetDiff[]) => void>> = {};

	const subscribeJsMessage = <T extends JsMessageType>(messageType: T, callback: (data: JsMessageTypeMap[T]) => void) => {
		subscriptions[messageType] = callback;
	};

	const unsubscribeJsMessage = (messageType: JsMessageType) => {
		delete subscriptions[messageType];
	};

	const subscribeLayoutUpdate = (target: LayoutTarget, callback: (diffs: WidgetDiff[]) => void) => {
		layoutCallbacks[target] = callback;
	};

	const unsubscribeLayoutUpdate = (target: LayoutTarget) => {
		delete layoutCallbacks[target];
	};

	const handleJsMessage = (messageType: JsMessageType, messageData: Record<string, unknown>, wasm: WebAssembly.Memory, handle: EditorHandle) => {
		// Find the message maker for the message type, which can either be undefined (passthrough) or a factory function
		if (!(messageType in messageMakers)) {
			// eslint-disable-next-line no-console
			console.error(
				`Received a frontend message of type "${messageType}" but was not able to parse the data. ` +
					"(Perhaps this message parser isn't exported in `messageMakers` at the bottom of `messages.ts`.)",
			);
			return;
		}
		const messageMaker = messageMakers[messageType];

		// Messages with non-empty data are provided by Serde JSON as an object with one key as the message name, like: { NameOfThisMessage: { ... } }
		// Messages with empty data are provided by Serde JSON as a string with the message name, like: "NameOfThisMessage"
		// Here we extract the payload object or use an empty object depending on the situation.
		const unwrappedMessageData = messageData[messageType] || {};

		// If the maker is undefined, the raw data is passed through directly.
		// If the maker is a factory function, it transforms the raw data into the desired shape.
		const message = messageMaker !== undefined ? messageMaker(unwrappedMessageData, wasm, handle) : unwrappedMessageData;

		// Resolve the callback lookup and the data to pass, depending on whether this is a layout update or a regular message.
		// UpdateLayout messages are dispatched to layout-specific callbacks based on the layout target.
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		let getCallback: () => ((data: any) => void) | undefined;
		let callbackData: unknown;
		let errorLabel: string;
		if (messageType === "UpdateLayout") {
			const { layoutTarget, diff } = message as JsMessageTypeMap["UpdateLayout"];
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
		subscribeJsMessage,
		unsubscribeJsMessage,
		subscribeLayoutUpdate,
		unsubscribeLayoutUpdate,
		handleJsMessage,
	};
}
export type SubscriptionRouter = ReturnType<typeof createSubscriptionRouter>;

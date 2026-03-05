import { type EditorHandle } from "@graphite/../wasm/pkg/graphite_wasm";
import { type JsMessageType, type JsMessageTypeMap, messageMakers } from "@graphite/messages";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type JsMessageCallbackMap = Record<string, ((messageData: any) => void) | undefined>;

export function createSubscriptionRouter() {
	const subscriptions: JsMessageCallbackMap = {};

	const subscribeJsMessage = <T extends JsMessageType>(messageType: T, callback: (data: JsMessageTypeMap[T]) => void) => {
		subscriptions[messageType] = callback;
	};

	const unsubscribeJsMessage = (messageType: JsMessageType) => {
		delete subscriptions[messageType];
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

		// If we have constructed a valid message, then we try and execute the callback that the frontend has associated with this message.
		// The frontend should always have a callback for all messages, but due to message ordering, we might have to delay a few stack frames until we do.
		let retries = 0;
		const callCallback = () => {
			const callback = subscriptions[messageType];

			// Attempt to call the callback, but try again several times on the next stack frame if it is not yet registered due to message ordering.
			if (callback) {
				callback(message);
			} else if (retries <= 3) {
				retries += 1;
				setTimeout(callCallback, 0);
			} else {
				// eslint-disable-next-line no-console
				console.error(`Received a frontend message of type "${messageType}" but no handler was registered for it from the client.`);
			}
		};

		callCallback();
	};

	return {
		subscribeJsMessage,
		unsubscribeJsMessage,
		handleJsMessage,
	};
}
export type SubscriptionRouter = ReturnType<typeof createSubscriptionRouter>;

import { plainToInstance } from "class-transformer";

import { type WasmEditorInstance, type WasmRawInstance } from "@graphite/wasm-communication/editor";
import { type JsMessageType, messageMakers, type JsMessage } from "@graphite/wasm-communication/messages";

type JsMessageCallback<T extends JsMessage> = (messageData: T) => void;
// Don't know a better way of typing this since it can be any subclass of JsMessage
// The functions interacting with this map are strongly typed though around JsMessage
// eslint-disable-next-line @typescript-eslint/no-explicit-any
type JsMessageCallbackMap = Record<string, JsMessageCallback<any> | undefined>;

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createSubscriptionRouter() {
	const subscriptions: JsMessageCallbackMap = {};

	const subscribeJsMessage = <T extends JsMessage, Args extends unknown[]>(messageType: new (...args: Args) => T, callback: JsMessageCallback<T>) => {
		subscriptions[messageType.name] = callback;
	};

	const handleJsMessage = (messageType: JsMessageType, messageData: Record<string, unknown>, wasm: WasmRawInstance, instance: WasmEditorInstance) => {
		// Find the message maker for the message type, which can either be a JS class constructor or a function that returns an instance of the JS class
		const messageMaker = messageMakers[messageType];
		if (!messageMaker) {
			// eslint-disable-next-line no-console
			console.error(
				`Received a frontend message of type "${messageType}" but was not able to parse the data. ` +
					"(Perhaps this message parser isn't exported in `messageMakers` at the bottom of `messages.ts`.)",
			);
			return;
		}

		// Checks if the provided `messageMaker` is a class extending `JsMessage`. All classes inheriting from `JsMessage` will have a static readonly `jsMessageMarker` which is `true`.
		const isJsMessageMaker = (fn: typeof messageMaker): fn is typeof JsMessage => "jsMessageMarker" in fn;
		const messageIsClass = isJsMessageMaker(messageMaker);

		// Messages with non-empty data are provided by wasm-bindgen as an object with one key as the message name, like: { NameOfThisMessage: { ... } }
		// Messages with empty data are provided by wasm-bindgen as a string with the message name, like: "NameOfThisMessage"
		// Here we extract the payload object or use an empty object depending on the situation.
		const unwrappedMessageData = messageData[messageType] || {};

		// Converts to a `JsMessage` object by turning the JSON message data into an instance of the message class, either automatically or by calling the function that builds it.
		// If the `messageMaker` is a `JsMessage` class then we use the class-transformer library's `plainToInstance` function in order to convert the JSON data into the destination class.
		// If it is not a `JsMessage` then it should be a custom function that creates a JsMessage from a JSON, so we call the function itself with the raw JSON as an argument.
		// The resulting `message` is an instance of a class that extends `JsMessage`.
		const message = messageIsClass ? plainToInstance(messageMaker, unwrappedMessageData) : messageMaker(unwrappedMessageData, wasm, instance);

		// If we have constructed a valid message, then we try and execute the callback that the frontend has associated with this message.
		// The frontend should always have a callback for all messages, but due to message ordering, we might have to delay a few stack frames until we do.
		let retries = 0;
		const callCallback = () => {
			// It is ok to use constructor.name even with minification since it is used consistently with registerHandler
			const callback = subscriptions[message.constructor.name];

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
		handleJsMessage,
	};
}
export type SubscriptionRouter = ReturnType<typeof createSubscriptionRouter>;

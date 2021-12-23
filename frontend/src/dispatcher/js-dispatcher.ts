import { plainToInstance } from "class-transformer";
import { JsMessageType, messageConstructors, JsMessage } from "@/dispatcher/js-messages";
import type { RustEditorInstance, WasmInstance } from "@/state/wasm-loader";

type JsMessageCallback<T extends JsMessage> = (messageData: T) => void;
type JsMessageCallbackMap = {
	// Don't know a better way of typing this since it can be any subclass of JsMessage
	// The functions interacting with this map are strongly typed though around JsMessage
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	[message: string]: JsMessageCallback<any> | undefined;
};

export function createJsDispatcher() {
	const subscriptions: JsMessageCallbackMap = {};

	const subscribeJsMessage = <T extends JsMessage, Args extends unknown[]>(messageType: new (...args: Args) => T, callback: JsMessageCallback<T>) => {
		subscriptions[messageType.name] = callback;
	};

	const handleJsMessage = (messageType: JsMessageType, messageData: Record<string, unknown>, wasm: WasmInstance, instance: RustEditorInstance) => {
		const messageConstructor = messageConstructors[messageType];
		if (!messageConstructor) {
			// eslint-disable-next-line no-console
			console.error(
				`Received a frontend message of type "${messageType}" but was not able to parse the data. ` +
					"(Perhaps this message parser isn't exported in `messageConstructors` at the bottom of `js-messages.ts`.)"
			);
			return;
		}

		// Messages with non-empty data are provided by wasm-bindgen as an object with one key as the message name, like: { NameOfThisMessage: { ... } }
		// Messages with empty data are provided by wasm-bindgen as a string with the message name, like: "NameOfThisMessage"
		const unwrappedMessageData = messageData[messageType] || {};

		const isJsMessageConstructor = (fn: typeof messageConstructor): fn is typeof JsMessage => {
			return "jsMessageMarker" in fn;
		};
		let message: JsMessage;
		if (isJsMessageConstructor(messageConstructor)) {
			message = plainToInstance(messageConstructor, unwrappedMessageData);
		} else {
			message = messageConstructor(unwrappedMessageData, wasm, instance);
		}

		// It is ok to use constructor.name even with minification since it is used consistently with registerHandler
		const callback = subscriptions[message.constructor.name];

		if (callback && message) {
			callback(message);
		} else if (message) {
			// eslint-disable-next-line no-console
			console.error(`Received a frontend message of type "${messageType}" but no handler was registered for it from the client.`);
		}
	};

	return {
		subscribeJsMessage,
		handleJsMessage,
	};
}
export type JsDispatcher = ReturnType<typeof createJsDispatcher>;

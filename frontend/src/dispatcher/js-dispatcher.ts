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

type Constructs<T> = new (...args: never[]) => T;

type JSMessageFactory = (data: unknown, wasm: WasmInstance, instance: RustEditorInstance) => JsMessage;

type MessageMaker = typeof JsMessage | JSMessageFactory;

export class JsDispatcher {
	private subscriptions: JsMessageCallbackMap = {};

	subscribeJsMessage<T extends JsMessage>(messageType: Constructs<T>, callback: JsMessageCallback<T>) {
		this.subscriptions[messageType.name] = callback;
	}

	handleJsMessage(messageType: JsMessageType, messageData: Record<string, unknown>, wasm: WasmInstance, instance: RustEditorInstance) {
		const messageConstructor = messageConstructors[messageType] as MessageMaker;
		if (!messageConstructor) {
			// eslint-disable-next-line no-console
			console.error(`Received a frontend message of type "${messageType}" but but was not able to parse the data.`);
			return;
		}

		// Messages with non-empty data are provided by wasm-bindgen as an object with one key as the message name, like: { NameOfThisMessage: { ... } }
		// Messages with empty data are provided by wasm-bindgen as a string with the message name, like: "NameOfThisMessage"
		const unwrappedMessageData = messageData[messageType] || {};

		const isJsMessageConstructor = (fn: MessageMaker): fn is typeof JsMessage => {
			return (fn as typeof JsMessage).jsMessageMarker !== undefined;
		};
		let message: JsMessage;
		if (isJsMessageConstructor(messageConstructor)) {
			message = plainToInstance(messageConstructor, unwrappedMessageData);
		} else {
			message = messageConstructor(unwrappedMessageData, wasm, instance);
		}

		// It is ok to use constructor.name even with minification since it is used consistently with registerHandler
		const callback = this.subscriptions[message.constructor.name];

		if (callback && message) {
			callback(message);
		} else if (message) {
			// eslint-disable-next-line no-console
			console.error(`Received a frontend message of type "${messageType}" but no handler was registered for it from the client.`);
		}
	}
}

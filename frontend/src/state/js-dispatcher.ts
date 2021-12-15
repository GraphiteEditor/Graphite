import { plainToInstance } from "class-transformer";
import {
	DisplayConfirmationToCloseAllDocuments,
	DisplayConfirmationToCloseDocument,
	DisplayError,
	DisplayPanic,
	ExportDocument,
	newDisplayFolderTreeStructure,
	OpenDocumentBrowse,
	SaveDocument,
	SetActiveDocument,
	SetActiveTool,
	SetCanvasRotation,
	SetCanvasZoom,
	UpdateCanvas,
	UpdateOpenDocumentsList,
	UpdateRulers,
	UpdateScrollbars,
	UpdateWorkingColors,
	UpdateLayer,
	JsMessage,
	GlobalJsMessage,
} from "../utilities/js-messages";
import { globalEditorManager } from "./global-state";
import { RustEditorInstance, WasmInstance } from "./wasm-loader";

type JsMessageCallback<T extends JsMessage> = (responseData: T) => void;
type JsMessageCallbackMap = {
	// Don't know a better way of typing this since it can be any subclass of JsMessage
	// The functions interacting with this map are strongly typed though around JsMessage
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	[response: string]: JsMessageCallback<any> | undefined;
};

type Constructs<T> = new (...args: never[]) => T;

const messageConstructorMap = {
	UpdateCanvas,
	UpdateScrollbars,
	UpdateRulers,
	ExportDocument,
	SaveDocument,
	OpenDocumentBrowse,
	DisplayFolderTreeStructure: newDisplayFolderTreeStructure,
	UpdateLayer,
	SetActiveTool,
	SetActiveDocument,
	UpdateOpenDocumentsList,
	UpdateWorkingColors,
	SetCanvasZoom,
	SetCanvasRotation,
	DisplayError,
	DisplayPanic,
	DisplayConfirmationToCloseDocument,
	DisplayConfirmationToCloseAllDocuments,
} as const;

export type JsMessageType = keyof typeof messageConstructorMap;

type JSMessageFactory = (data: unknown, wasm: WasmInstance, instance: RustEditorInstance) => JsMessage;

type MessageMaker = typeof JsMessage | JSMessageFactory;

function isJsMessageConstructor(fn: MessageMaker): fn is typeof JsMessage {
	return (fn as typeof JsMessage).jsMessageMarker !== undefined;
}

export class JsDispatcher {
	private responseMap: JsMessageCallbackMap = {};

	handleJsMessage(messageType: JsMessageType, responseData: Record<string, unknown>, wasm: WasmInstance, instance: RustEditorInstance) {
		const messageMaker = messageConstructorMap[messageType] as MessageMaker;
		let message: JsMessage;

		if (!messageMaker) {
			// eslint-disable-next-line no-console
			console.error(`Received a Response of type "${messageType}" but but was not able to parse the data.`);
		}

		if (isJsMessageConstructor(messageMaker)) {
			message = plainToInstance(messageMaker, responseData[messageType]);
		} else {
			message = messageMaker(responseData[messageType], wasm, instance);
		}

		if (message instanceof GlobalJsMessage) {
			globalEditorManager.broadcastGlobalMessage(message);
		} else {
			this.dispatchJsMessage(message);
		}
	}

	dispatchJsMessage(message: JsMessage) {
		// It is ok to use constructor.name even with minification since it is used consistently with registerHandler
		const callback = this.responseMap[message.constructor.name];

		if (callback && message) {
			callback(message);
		} else if (message) {
			// eslint-disable-next-line no-console
			console.error(`Received a Response of type "${message.constructor.name}" but no handler was registered for it from the client.`);
		}
	}

	subscribeJsMessage<T extends JsMessage>(responseType: Constructs<T>, callback: JsMessageCallback<T>) {
		this.responseMap[responseType.name] = callback;
	}
}

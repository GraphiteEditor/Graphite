/* eslint-disable @typescript-eslint/no-explicit-any */
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
	WasmInstance,
} from "../utilities/js-messages";

type JsMessageCallback<T extends JsMessage> = (responseData: T) => void;
type JsMessageCallbackMap = {
	[response: string]: JsMessageCallback<any> | undefined;
};

type Constructs<T> = new (...args: any[]) => T;
type ConstructsJsMessage = typeof JsMessage;

const responseMap = {
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

export type JsMessageType = keyof typeof responseMap;

function isJsMessageConstructor(fn: ConstructsJsMessage | ((data: any, wasm: WasmInstance) => JsMessage)): fn is ConstructsJsMessage {
	return (fn as ConstructsJsMessage).jsMessageMarker !== undefined;
}

export class JsDispatcher {
	private responseMap: JsMessageCallbackMap = {};

	handleJsMessage(responseType: JsMessageType, responseData: any, wasm: WasmInstance) {
		const messageMaker = responseMap[responseType];
		let message: JsMessage;

		if (!messageMaker) {
			// eslint-disable-next-line no-console
			console.error(`Received a Response of type "${responseType}" but but was not able to parse the data.`);
		}

		if (isJsMessageConstructor(messageMaker)) {
			message = plainToInstance(messageMaker, responseData[responseType]);
		} else {
			message = messageMaker(responseData[responseType], wasm);
		}

		// It is ok to use constructor.name even with minification since it is used consistently with registerHandler
		const callback = this.responseMap[message.constructor.name];

		if (callback && message) {
			callback(message);
		} else if (message) {
			// eslint-disable-next-line no-console
			console.error(`Received a Response of type "${responseType}" but no handler was registered for it from the client.`);
		}
	}

	subscribeJsMessage<T extends JsMessage>(responseType: Constructs<T>, callback: JsMessageCallback<T>) {
		this.responseMap[responseType.name] = callback;
	}
}

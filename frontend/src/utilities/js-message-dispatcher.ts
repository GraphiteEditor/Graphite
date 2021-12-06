/* eslint-disable @typescript-eslint/no-explicit-any */

import { reactive } from "vue";
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
} from "./js-messages";

type JsMessageCallback<T extends JsMessage> = (responseData: T) => void;
type JsMessageCallbackMap = {
	[response: string]: JsMessageCallback<any> | undefined;
};

const state = reactive({
	responseMap: {} as JsMessageCallbackMap,
});

type Constructs<T> = new (...args: any[]) => T;
type ConstructsJsMessage = Constructs<JsMessage> & typeof JsMessage;

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

function isResponseConstructor(fn: ConstructsJsMessage | ((data: any) => JsMessage)): fn is ConstructsJsMessage {
	return (fn as ConstructsJsMessage).responseMarker !== undefined;
}

export function handleJsMessage(responseType: JsMessageType, responseData: any) {
	const MessageMaker = responseMap[responseType];
	let message: JsMessage;

	if (!MessageMaker) {
		// eslint-disable-next-line no-console
		console.error(`Received a Response of type "${responseType}" but but was not able to parse the data.`);
	}

	if (isResponseConstructor(MessageMaker)) {
		message = plainToInstance(MessageMaker, responseData[responseType]);
	} else {
		message = MessageMaker(responseData[responseType]);
	}

	// It is ok to use constructor.name even with minification since it is used consistently with registerHandler
	const callback = state.responseMap[message.constructor.name];

	if (callback && message) {
		callback(message);
	} else if (message) {
		// eslint-disable-next-line no-console
		console.error(`Received a Response of type "${responseType}" but no handler was registered for it from the client.`);
	}
}

export function subscribeJsMessage<T extends JsMessage>(responseType: Constructs<T>, callback: JsMessageCallback<T>) {
	state.responseMap[responseType.name] = callback;
}

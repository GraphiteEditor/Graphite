/* eslint-disable @typescript-eslint/no-explicit-any */

import { plainToInstance } from "class-transformer";
import {
	JsMessage,
	DisplayConfirmationToCloseAllDocuments,
	DisplayConfirmationToCloseDocument,
	DisplayError,
	DisplayPanic,
	ExportDocument,
	newDisplayFolderTreeStructure as DisplayFolderTreeStructure,
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
} from "@/utilities/js-messages";

const messageConstructors = {
	UpdateCanvas,
	UpdateScrollbars,
	UpdateRulers,
	ExportDocument,
	SaveDocument,
	OpenDocumentBrowse,
	DisplayFolderTreeStructure,
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
type JsMessageType = keyof typeof messageConstructors;

type JsMessageCallback<T extends JsMessage> = (messageData: T) => void;
type JsMessageCallbackMap = {
	[message: string]: JsMessageCallback<any> | undefined;
};

type Constructs<T> = new (...args: any[]) => T;
type ConstructsJsMessage = Constructs<JsMessage> & typeof JsMessage;

const subscriptions = {} as JsMessageCallbackMap;

export function subscribeJsMessage<T extends JsMessage>(messageType: Constructs<T>, callback: JsMessageCallback<T>) {
	subscriptions[messageType.name] = callback;
}

export function handleJsMessage(messageType: JsMessageType, messageData: any) {
	const messageConstructor = messageConstructors[messageType];
	if (!messageConstructor) {
		// eslint-disable-next-line no-console
		console.error(`Received a frontend message of type "${messageType}" but but was not able to parse the data.`);
		return;
	}

	// Messages with non-empty data are provided by wasm-bindgen as an object with one key as the message name, like: { NameOfThisMessage: { ... } }
	// Messages with empty data are provided by wasm-bindgen as a string with the message name, like: "NameOfThisMessage"
	const unwrappedMessageData = messageData[messageType] || {};

	const isJsMessageConstructor = (fn: ConstructsJsMessage | ((data: any) => JsMessage)): fn is ConstructsJsMessage => {
		return (fn as ConstructsJsMessage).jsMessageMarker !== undefined;
	};
	let message: JsMessage;
	if (isJsMessageConstructor(messageConstructor)) {
		message = plainToInstance(messageConstructor, unwrappedMessageData);
	} else {
		message = messageConstructor(unwrappedMessageData);
	}

	// It is ok to use constructor.name even with minification since it is used consistently with registerHandler
	const callback = subscriptions[message.constructor.name];

	if (callback && message) {
		callback(message);
	} else if (message) {
		// eslint-disable-next-line no-console
		console.error(`Received a frontend message of type "${messageType}" but no handler was registered for it from the client.`);
	}
}

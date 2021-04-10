type ResponseCallback = (responseData: string) => void;
type ResponseMap = {
	[response: string]: ResponseCallback | undefined;
};
declare global {
	interface Window { responseMap: ResponseMap }
}

export enum ResponseType {
	UpdateCanvas = "UpdateCanvas",
	SetActiveTool = "SetActiveTool",
}

export function attachResponseHandlerToPage() {
	window.responseMap = {};
}

export function registerResponseHandler(responseType: ResponseType, callback: ResponseCallback) {
	window.responseMap[responseType] = callback;
}

export function handleResponse(responseType: ResponseType, responseData: string) {
	const callback = window.responseMap[responseType];

	if (callback) {
		callback(responseData);
	}
	else {
		console.error(`Received a Response of type "${responseType}" but no handler was registered for it from the client.`);
	}
}

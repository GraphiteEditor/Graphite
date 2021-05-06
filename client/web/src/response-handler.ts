type ResponseCallback = (responseData: Response) => void;
type ResponseMap = {
	[response: string]: ResponseCallback | undefined;
};
declare global {
	interface Window {
		responseMap: ResponseMap;
	}
}

export enum ResponseType {
	UpdateCanvas = "UpdateCanvas",
	ExpandFolder = "ExpandFolder",
	CollapseFolder = "CollapseFolder",
	SetActiveTool = "SetActiveTool",
}

export function attachResponseHandlerToPage() {
	window.responseMap = {};
}

export function registerResponseHandler(responseType: ResponseType, callback: ResponseCallback) {
	window.responseMap[responseType] = callback;
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function parseResponse(origin: string, type: string, data: any): Response {
	switch (origin) {
		case "Document":
			switch (type) {
				case "DocumentChanged":
					return data.Document.DocumentChanged as Response;
				case "CollapseFolder":
					return data.Document.CollapseFolder as Response;
				case "ExpandFolder":
					return (data.Document.ExpandFolder as ExpandFolder) as Response;
			}
		case "Tool":
			switch (type) {
				case "SetActiveTool":
					return data.Tool.SetActiveTool as Response;
				case "UpdateCanvas":
					return data.Tool.UpdateCanvas as Response;
			}
		default:
			throw new Error("ResponseType not recognized");
	}
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function handleResponse(responseType: ResponseType, responseData: any) {
	const [origin, type] = responseType.split("::", 2);
	const callback = window.responseMap[type];
	const data = parseResponse(origin, type, responseData);

	if (callback) {
		callback(data);
	} else {
		console.error(`Received a Response of type "${responseType}" but no handler was registered for it from the client.`);
	}
}

export type Response = SetActiveTool | UpdateCanvas | DocumentChanged | CollapseFolder | ExpandFolder;

export interface SetActiveTool {
	tool_name: string;
}
export interface UpdateCanvas {
	document: string;
}
export interface DocumentChanged {}
export interface CollapseFolder {
	path: Array<number>;
}
export interface ExpandFolder {
	path: Array<number>;
	children: Array<LayerPanelEntry>;
}

export interface LayerPanelEntry {
	name: string;
	visible: boolean;
	layer_type: LayerType;
}

export enum LayerType {
	Folder,
	Shape,
}

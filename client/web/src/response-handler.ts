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
function parseResponse(responseType: string, data: any): Response {
	switch (responseType) {
		case "DocumentChanged":
			return (data.DocumentChanged as DocumentChanged) as Response;
		case "CollapseFolder":
			return (data.CollapseFolder as CollapseFolder) as Response;
		case "ExpandFolder":
			return (data.ExpandFolder as ExpandFolder) as Response;
		case "SetActiveTool":
			return (data.SetActiveTool as SetActiveTool) as Response;
		case "UpdateCanvas":
			return (data.UpdateCanvas as UpdateCanvas) as Response;
		default:
			throw new Error("ResponseType not recognized");
	}
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function handleResponse(responseType: string, responseData: any) {
	const callback = window.responseMap[responseType];
	const data = parseResponse(responseType, responseData);

	if (callback && data) {
		callback(data);
	} else if (data) {
		console.error(`Received a Response of type "${responseType}" but no handler was registered for it from the client.`);
	} else {
		console.error(`Received a Response of type "${responseType}" but but was not able to parse the data.`);
	}
}

export type Response = SetActiveTool | UpdateCanvas | DocumentChanged | CollapseFolder | ExpandFolder;

export interface SetActiveTool {
	tool_name: string;
}
export interface UpdateCanvas {
	document: string;
}
export type DocumentChanged = {};
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
	collapsed: boolean;
	path: Array<number>;
}

export enum LayerType {
	Folder,
	Shape,
}

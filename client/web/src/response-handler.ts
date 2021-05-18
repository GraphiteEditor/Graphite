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
function parseResponse(origin: string, responseType: string, data: any): Response {
	type OriginNames = "Document" | "Tool";

	const originHandlers = {
		Document: () => {
			switch (responseType) {
				case "DocumentChanged":
					return (data.Document.DocumentChanged as DocumentChanged) as Response;
				case "CollapseFolder":
					return (data.Document.CollapseFolder as CollapseFolder) as Response;
				case "ExpandFolder":
					return (data.Document.ExpandFolder as ExpandFolder) as Response;
				default:
					return undefined;
			}
		},
		Tool: () => {
			switch (responseType) {
				case "SetActiveTool":
					return (data.Tool.SetActiveTool as SetActiveTool) as Response;
				case "UpdateCanvas":
					return (data.Tool.UpdateCanvas as UpdateCanvas) as Response;
				default:
					return undefined;
			}
		},
	};

	// TODO: Optional chaining would be nice here when we can upgrade to Webpack 5: https://github.com/webpack/webpack/issues/10227
	// const response = originHandlers[origin as OriginNames]?.();
	const response = originHandlers[origin as OriginNames] && originHandlers[origin as OriginNames]();
	if (!response) throw new Error("ResponseType not recognized. Received: " + responseType );
	return response;
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function handleResponse(responseIdentifier: string, responseData: any) {
	console.error(responseIdentifier)
	const [origin, responseType] = responseIdentifier.split(".", 2);
	const callback = window.responseMap[responseType];
	const data = parseResponse(origin, responseType, responseData);

	if (callback && data) {
		callback(data);
	} else if (data) {
		console.error(`Received a Response of type "${responseIdentifier}" but no handler was registered for it from the client.`);
	} else {
		console.error(`Received a Response of type "${responseIdentifier}" but but was not able to parse the data.`);
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

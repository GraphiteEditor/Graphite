/* eslint-disable @typescript-eslint/no-explicit-any */

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
	UpdatePrimaryColor = "UpdatePrimaryColor",
	UpdateSecondaryColor = "UpdateSecondaryColor",
}

export function attachResponseHandlerToPage() {
	window.responseMap = {};
}

export function registerResponseHandler(responseType: ResponseType, callback: ResponseCallback) {
	window.responseMap[responseType] = callback;
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

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function parseResponse(responseType: string, data: any): Response {
	switch (responseType) {
		case "DocumentChanged":
			return newDocumentChanged(data.DocumentChanged);
		case "CollapseFolder":
			return newCollapseFolder(data.CollapseFolder);
		case "ExpandFolder":
			return newExpandFolder(data.ExpandFolder);
		case "SetActiveTool":
			return newSetActiveTool(data.SetActiveTool);
		case "UpdateCanvas":
			return newUpdateCanvas(data.UpdateCanvas);
		case "UpdatePrimaryColor":
			return data.UpdatePrimaryColor;
		case "UpdateSecondaryColor":
			return data.UpdateSecondaryColor;
		default:
			throw new Error(`Unrecognized origin/responseType pair: ${origin}, ${responseType}`);
	}
}

export type Response = SetActiveTool | UpdateCanvas | DocumentChanged | CollapseFolder | ExpandFolder | RGBColor;

export interface SetActiveTool {
	tool_name: string;
}
function newSetActiveTool(input: any): SetActiveTool {
	return {
		tool_name: input.tool_name,
	};
}

export interface UpdateCanvas {
	document: string;
}
function newUpdateCanvas(input: any): UpdateCanvas {
	return {
		document: input.document,
	};
}

export type DocumentChanged = {};
function newDocumentChanged(_: any): DocumentChanged {
	return {};
}

export interface CollapseFolder {
	path: BigUint64Array;
}
function newCollapseFolder(input: any): CollapseFolder {
	return {
		path: new BigUint64Array(input.path.map((n: number) => BigInt(n))),
	};
}

export interface ExpandFolder {
	path: BigUint64Array;
	children: Array<LayerPanelEntry>;
}
function newExpandFolder(input: any): ExpandFolder {
	return {
		path: new BigUint64Array(input.path.map((n: number) => BigInt(n))),
		children: input.children.map((child: any) => newLayerPanelEntry(child)),
	};
}

export interface LayerPanelEntry {
	name: string;
	visible: boolean;
	layer_type: LayerType;
	collapsed: boolean;
	path: BigUint64Array;
}
function newLayerPanelEntry(input: any): LayerPanelEntry {
	return {
		name: input.name,
		visible: input.visible,
		layer_type: newLayerType(input.layer_type),
		collapsed: input.collapsed,
		path: new BigUint64Array(input.path.map((n: number) => BigInt(n))),
	};
}

export enum LayerType {
	Folder = "Folder",
	Shape = "Shape",
	Circle = "Circle",
	Rect = "Rect",
	Line = "Line",
	PolyLine = "PolyLine",
	Ellipse = "Ellipse",
}
function newLayerType(input: any): LayerType {
	switch (input) {
		case "Folder":
			return LayerType.Folder;
		case "Shape":
			return LayerType.Shape;
		case "Circle":
			return LayerType.Circle;
		case "Rect":
			return LayerType.Rect;
		case "Line":
			return LayerType.Line;
		case "PolyLine":
			return LayerType.PolyLine;
		case "Ellipse":
			return LayerType.Ellipse;
		default:
			throw Error(`Received invalid input as an enum variant for LayerType: ${input}`);
	}
}

export interface RGBColor {
	r: number;
	g: number;
	b: number;
	a: number;
}

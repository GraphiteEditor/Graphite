/* eslint-disable @typescript-eslint/no-explicit-any */
/* eslint-disable camelcase */

import { reactive } from "vue";

type ResponseCallback = (responseData: Response) => void;
type ResponseMap = {
	[response: string]: ResponseCallback | undefined;
};

const state = reactive({
	responseMap: {} as ResponseMap,
});

export enum ResponseType {
	UpdateCanvas = "UpdateCanvas",
	UpdateScrollbars = "UpdateScrollbars",
	ExportDocument = "ExportDocument",
	SaveDocument = "SaveDocument",
	OpenDocumentBrowse = "OpenDocumentBrowse",
	ExpandFolder = "ExpandFolder",
	CollapseFolder = "CollapseFolder",
	UpdateLayer = "UpdateLayer",
	SetActiveTool = "SetActiveTool",
	SetActiveDocument = "SetActiveDocument",
	UpdateOpenDocumentsList = "UpdateOpenDocumentsList",
	UpdateWorkingColors = "UpdateWorkingColors",
	SetCanvasZoom = "SetCanvasZoom",
	SetCanvasRotation = "SetCanvasRotation",
	DisplayError = "DisplayError",
	DisplayPanic = "DisplayPanic",
	DisplayConfirmationToCloseDocument = "DisplayConfirmationToCloseDocument",
	DisplayConfirmationToCloseAllDocuments = "DisplayConfirmationToCloseAllDocuments",
}

export function registerResponseHandler(responseType: ResponseType, callback: ResponseCallback) {
	state.responseMap[responseType] = callback;
}

export function handleResponse(responseType: string, responseData: any) {
	const callback = state.responseMap[responseType];
	const data = parseResponse(responseType, responseData);

	if (callback && data) {
		callback(data);
	} else if (data) {
		// eslint-disable-next-line no-console
		console.error(`Received a Response of type "${responseType}" but no handler was registered for it from the client.`);
	} else {
		// eslint-disable-next-line no-console
		console.error(`Received a Response of type "${responseType}" but but was not able to parse the data.`);
	}
}

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
		case "SetActiveDocument":
			return newSetActiveDocument(data.SetActiveDocument);
		case "UpdateOpenDocumentsList":
			return newUpdateOpenDocumentsList(data.UpdateOpenDocumentsList);
		case "UpdateCanvas":
			return newUpdateCanvas(data.UpdateCanvas);
		case "UpdateScrollbars":
			return newUpdateScrollbars(data.UpdateScrollbars);
		case "UpdateLayer":
			return newUpdateLayer(data.UpdateLayer);
		case "SetCanvasZoom":
			return newSetCanvasZoom(data.SetCanvasZoom);
		case "SetCanvasRotation":
			return newSetCanvasRotation(data.SetCanvasRotation);
		case "ExportDocument":
			return newExportDocument(data.ExportDocument);
		case "SaveDocument":
			return newSaveDocument(data.SaveDocument);
		case "OpenDocumentBrowse":
			return newOpenDocumentBrowse(data.OpenDocumentBrowse);
		case "UpdateWorkingColors":
			return newUpdateWorkingColors(data.UpdateWorkingColors);
		case "DisplayError":
			return newDisplayError(data.DisplayError);
		case "DisplayPanic":
			return newDisplayPanic(data.DisplayPanic);
		case "DisplayConfirmationToCloseDocument":
			return newDisplayConfirmationToCloseDocument(data.DisplayConfirmationToCloseDocument);
		case "DisplayConfirmationToCloseAllDocuments":
			return newDisplayConfirmationToCloseAllDocuments(data.DisplayConfirmationToCloseAllDocuments);
		default:
			throw new Error(`Unrecognized origin/responseType pair: ${origin}, '${responseType}'`);
	}
}

export type Response = SetActiveTool | UpdateCanvas | UpdateScrollbars | DocumentChanged | CollapseFolder | ExpandFolder | UpdateWorkingColors | SetCanvasZoom | SetCanvasRotation;

export interface UpdateOpenDocumentsList {
	open_documents: Array<string>;
}
function newUpdateOpenDocumentsList(input: any): UpdateOpenDocumentsList {
	return { open_documents: input.open_documents };
}

export interface Color {
	red: number;
	green: number;
	blue: number;
	alpha: number;
}
function newColor(input: any): Color {
	// TODO: Possibly change this in the Rust side to avoid any pitfalls
	return { red: input.red * 255, green: input.green * 255, blue: input.blue * 255, alpha: input.alpha };
}

export interface UpdateWorkingColors {
	primary: Color;
	secondary: Color;
}
function newUpdateWorkingColors(input: any): UpdateWorkingColors {
	return {
		primary: newColor(input.primary),
		secondary: newColor(input.secondary),
	};
}

export interface SetActiveTool {
	tool_name: string;
	tool_options: object;
}
function newSetActiveTool(input: any): SetActiveTool {
	return {
		tool_name: input.tool_name,
		tool_options: input.tool_options,
	};
}

export interface SetActiveDocument {
	document_index: number;
}
function newSetActiveDocument(input: any): SetActiveDocument {
	return {
		document_index: input.document_index,
	};
}

export interface DisplayError {
	title: string;
	description: string;
}
function newDisplayError(input: any): DisplayError {
	return {
		title: input.title,
		description: input.description,
	};
}

export interface DisplayPanic {
	title: string;
	description: string;
}
function newDisplayPanic(input: any): DisplayPanic {
	return {
		title: input.title,
		description: input.description,
	};
}

export interface DisplayConfirmationToCloseDocument {
	document_index: number;
}
function newDisplayConfirmationToCloseDocument(input: any): DisplayConfirmationToCloseDocument {
	return {
		document_index: input.document_index,
	};
}

function newDisplayConfirmationToCloseAllDocuments(_input: any): {} {
	return {};
}

export interface UpdateCanvas {
	document: string;
}
function newUpdateCanvas(input: any): UpdateCanvas {
	return {
		document: input.document,
	};
}

export interface UpdateScrollbars {
	position: { x: number; y: number };
	size: { x: number; y: number };
	multiplier: { x: number; y: number };
}
function newUpdateScrollbars(input: any): UpdateScrollbars {
	return {
		position: { x: input.position[0], y: input.position[1] },
		size: { x: input.size[0], y: input.size[1] },
		multiplier: { x: input.multiplier[0], y: input.multiplier[1] },
	};
}

export interface ExportDocument {
	document: string;
	name: string;
}
function newExportDocument(input: any): ExportDocument {
	return {
		document: input.document,
		name: input.name,
	};
}

export interface SaveDocument {
	document: string;
	name: string;
}
function newSaveDocument(input: any): SaveDocument {
	return {
		document: input.document,
		name: input.name,
	};
}

export type OpenDocumentBrowse = {};
function newOpenDocumentBrowse(_: any): OpenDocumentBrowse {
	return {};
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
		path: newPath(input.path),
	};
}

export interface UpdateLayer {
	path: BigUint64Array;
	data: LayerPanelEntry;
}
function newUpdateLayer(input: any): UpdateLayer {
	return {
		path: newPath(input.data.path),
		data: newLayerPanelEntry(input.data),
	};
}

export interface ExpandFolder {
	path: BigUint64Array;
	children: Array<LayerPanelEntry>;
}
function newExpandFolder(input: any): ExpandFolder {
	return {
		path: newPath(input.path),
		children: input.children.map((child: any) => newLayerPanelEntry(child)),
	};
}

export interface SetCanvasZoom {
	new_zoom: number;
}
function newSetCanvasZoom(input: any): SetCanvasZoom {
	return {
		new_zoom: input.new_zoom,
	};
}

export interface SetCanvasRotation {
	new_radians: number;
}
function newSetCanvasRotation(input: any): SetCanvasRotation {
	return {
		new_radians: input.new_radians,
	};
}

function newPath(input: any): BigUint64Array {
	// eslint-disable-next-line
	const u32CombinedPairs = input.map((n: Array<number>) => BigInt((BigInt(n[0]) << BigInt(32)) | BigInt(n[1])));
	return new BigUint64Array(u32CombinedPairs);
}

export enum BlendMode {
	Normal = "normal",
	Multiply = "multiply",
	Darken = "darken",
	ColorBurn = "color-burn",
	Screen = "screen",
	Lighten = "lighten",
	ColorDodge = "color-dodge",
	Overlay = "overlay",
	SoftLight = "soft-light",
	HardLight = "hard-light",
	Difference = "difference",
	Exclusion = "exclusion",
	Hue = "hue",
	Saturation = "saturation",
	Color = "color",
	Luminosity = "luminosity",
}
function newBlendMode(input: string): BlendMode {
	const blendMode = {
		Normal: BlendMode.Normal,
		Multiply: BlendMode.Multiply,
		Darken: BlendMode.Darken,
		ColorBurn: BlendMode.ColorBurn,
		Screen: BlendMode.Screen,
		Lighten: BlendMode.Lighten,
		ColorDodge: BlendMode.ColorDodge,
		Overlay: BlendMode.Overlay,
		SoftLight: BlendMode.SoftLight,
		HardLight: BlendMode.HardLight,
		Difference: BlendMode.Difference,
		Exclusion: BlendMode.Exclusion,
		Hue: BlendMode.Hue,
		Saturation: BlendMode.Saturation,
		Color: BlendMode.Color,
		Luminosity: BlendMode.Luminosity,
	}[input];

	if (!blendMode) throw new Error(`Invalid blend mode "${blendMode}"`);

	return blendMode;
}

function newOpacity(input: number): number {
	return input * 100;
}

export interface LayerPanelEntry {
	name: string;
	visible: boolean;
	blend_mode: BlendMode;
	opacity: number;
	layer_type: LayerType;
	path: BigUint64Array;
	layer_data: LayerData;
	thumbnail: string;
}
function newLayerPanelEntry(input: any): LayerPanelEntry {
	return {
		name: input.name,
		visible: input.visible,
		blend_mode: newBlendMode(input.blend_mode),
		opacity: newOpacity(input.opacity),
		layer_type: newLayerType(input.layer_type),
		layer_data: newLayerData(input.layer_data),
		path: newPath(input.path),
		thumbnail: input.thumbnail,
	};
}

export interface LayerData {
	expanded: boolean;
	selected: boolean;
}
function newLayerData(input: any): LayerData {
	return {
		expanded: input.expanded,
		selected: input.selected,
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

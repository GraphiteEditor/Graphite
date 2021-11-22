/* eslint-disable @typescript-eslint/no-explicit-any */
/* eslint-disable camelcase */
/* eslint-disable max-classes-per-file */

import { reactive } from "vue";
import { plainToInstance, Transform, Type } from "class-transformer";

type ResponseCallback<T extends Response> = (responseData: T) => void;
type ResponseMap = {
	[response: string]: ResponseCallback<any> | undefined;
};

const state = reactive({
	responseMap: {} as ResponseMap,
});

export class Response {
	static responseMarker = true;
}

export class UpdateOpenDocumentsList extends Response {
	open_documents!: string[];
}

const To255Scale = Transform(({ value }) => value * 255);
export class Color {
	@To255Scale
	red!: number;

	@To255Scale
	green!: number;

	@To255Scale
	blue!: number;

	alpha!: number;

	toRgb() {
		return { r: this.red, g: this.green, b: this.blue, a: this.alpha };
	}

	toString() {
		const { r, g, b, a } = this.toRgb();
		return `rgba(${r}, ${g}, ${b}, ${a})`;
	}
}

export class UpdateWorkingColors extends Response {
	@Type(() => Color)
	primary!: Color;

	@Type(() => Color)
	secondary!: Color;
}

export class SetActiveTool extends Response {
	tool_name!: string;

	tool_options!: object;
}

export class SetActiveDocument extends Response {
	document_index!: number;
}

export class DisplayError extends Response {
	title!: string;

	description!: string;
}

export class DisplayPanic extends Response {
	panic_info!: string;

	title!: string;

	description!: string;
}

export class DisplayConfirmationToCloseDocument extends Response {
	document_index!: number;
}

export class DisplayConfirmationToCloseAllDocuments extends Response {}

export class UpdateCanvas extends Response {
	document!: string;
}

const TupleToVec2 = Transform(({ value }) => ({ x: value[0], y: value[1] }));

export class UpdateScrollbars extends Response {
	@TupleToVec2
	position!: { x: number; y: number };

	@TupleToVec2
	size!: { x: number; y: number };

	@TupleToVec2
	multiplier!: { x: number; y: number };
}

export class UpdateRulers extends Response {
	@TupleToVec2
	origin!: { x: number; y: number };

	spacing!: number;

	interval!: number;
}

export class ExportDocument extends Response {
	document!: string;

	name!: string;
}

export class SaveDocument extends Response {
	document!: string;

	name!: string;
}

export class OpenDocumentBrowse extends Response {}

export class DocumentChanged extends Response {}

export class DisplayFolderTreeStructure extends Response {
	constructor(public layerId: BigInt, public children: DisplayFolderTreeStructure[]) {
		super();
	}
}
function newDisplayFolderTreeStructure(input: any): DisplayFolderTreeStructure {
	const { ptr, len } = input.data_buffer;
	const wasmMemoryBuffer = (window as any).wasmMemory().buffer;

	// Decode the folder structure encoding
	const encoding = new DataView(wasmMemoryBuffer, ptr, len);

	// The structure section indicates how to read through the upcoming layer list and assign depths to each layer
	const structureSectionLength = Number(encoding.getBigUint64(0, true));
	const structureSectionMsbSigned = new DataView(wasmMemoryBuffer, ptr + 8, structureSectionLength * 8);

	// The layer IDs section lists each layer ID sequentially in the tree, as it will show up in the panel
	const layerIdsSection = new DataView(wasmMemoryBuffer, ptr + 8 + structureSectionLength * 8);

	let layersEncountered = 0;
	let currentFolder = new DisplayFolderTreeStructure(BigInt(-1), []);
	const currentFolderStack = [currentFolder];

	for (let i = 0; i < structureSectionLength; i += 1) {
		const msbSigned = structureSectionMsbSigned.getBigUint64(i * 8, true);
		const msbMask = BigInt(1) << BigInt(63);

		// Set the MSB to 0 to clear the sign and then read the number as usual
		const numberOfLayersAtThisDepth = msbSigned & ~msbMask;

		// Store child folders in the current folder (until we are interrupted by an indent)
		for (let j = 0; j < numberOfLayersAtThisDepth; j += 1) {
			const layerId = layerIdsSection.getBigUint64(layersEncountered * 8, true);
			layersEncountered += 1;

			const childLayer = new DisplayFolderTreeStructure(layerId, []);
			currentFolder.children.push(childLayer);
		}

		// Check the sign of the MSB, where a 1 is a negative (outward) indent
		const subsequentDirectionOfDepthChange = (msbSigned & msbMask) === BigInt(0);
		// debugger;
		// Inward
		if (subsequentDirectionOfDepthChange) {
			currentFolderStack.push(currentFolder);
			currentFolder = currentFolder.children[currentFolder.children.length - 1];
		}
		// Outward
		else {
			const popped = currentFolderStack.pop();
			if (!popped) throw Error("Too many negative indents in the folder structure");
			if (popped) currentFolder = popped;
		}
	}

	return currentFolder;
}

export class UpdateLayer extends Response {
	constructor(public path: BigUint64Array, public data: LayerPanelEntry) {
		super();
	}
}

function newUpdateLayer(input: any): UpdateLayer {
	return new UpdateLayer(newPath(input.data.path), newLayerPanelEntry(input.data));
}

export class SetCanvasZoom extends Response {
	new_zoom!: number;
}

export class SetCanvasRotation extends Response {
	new_radians!: number;
}

function newPath(input: any): BigUint64Array {
	// eslint-disable-next-line
	const u32CombinedPairs = input.map((n: Array<number>) => BigInt((BigInt(n[0]) << BigInt(32)) | BigInt(n[1])));
	return new BigUint64Array(u32CombinedPairs);
}

type Constructs<T> = new (...args: any[]) => T;

// From https://stackoverflow.com/questions/60496276/typescript-derive-union-type-from-array-of-objects
function createResponseMap<T extends Record<string, Constructs<Response> | ((data: any) => Response)>>(arg: T) {
	return arg;
}

const responseMap = createResponseMap({
	UpdateCanvas,
	UpdateScrollbars,
	UpdateRulers,
	ExportDocument,
	SaveDocument,
	OpenDocumentBrowse,
	DisplayFolderTreeStructure: newDisplayFolderTreeStructure,
	UpdateLayer: newUpdateLayer,
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
});

export type ResponseType = keyof typeof responseMap;

function isResponseConstructor(fn: Constructs<Response> | ((data: any) => Response)): fn is Constructs<Response> {
	return (fn as any).responseMarker !== undefined;
}

export function handleResponse(responseType: ResponseType, responseData: any) {
	const dataParser = responseMap[responseType];
	let data: Response;

	if (!dataParser) {
		// eslint-disable-next-line no-console
		console.error(`Received a Response of type "${responseType}" but but was not able to parse the data.`);
	}

	if (isResponseConstructor(dataParser)) {
		data = plainToInstance(dataParser, responseData[responseType]);
	} else {
		data = dataParser(responseData[responseType]);
	}

	// It is ok to use constructor.name even with minification since it is used consistently with registerHandler
	const callback = state.responseMap[data.constructor.name];

	if (callback && data) {
		callback(data);
	} else if (data) {
		// eslint-disable-next-line no-console
		console.error(`Received a Response of type "${responseType}" but no handler was registered for it from the client.`);
	}
}

export function registerResponseHandler<T extends Response>(responseType: Constructs<T>, callback: ResponseCallback<T>) {
	state.responseMap[responseType.name] = callback;
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
	"Color" = "color",
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

/* eslint-disable @typescript-eslint/no-explicit-any */
/* eslint-disable camelcase */
/* eslint-disable max-classes-per-file */

import { Transform, Type } from "class-transformer";

export class JsMessage {
	static responseMarker = true;
}

export class UpdateOpenDocumentsList extends JsMessage {
	@Transform(({ value }) => value.map((tuple: [string, boolean]) => ({ name: tuple[0], isSaved: tuple[1] })))
	open_documents!: { name: string; isSaved: boolean }[];
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

export class UpdateWorkingColors extends JsMessage {
	@Type(() => Color)
	primary!: Color;

	@Type(() => Color)
	secondary!: Color;
}

export class SetActiveTool extends JsMessage {
	tool_name!: string;

	tool_options!: object;
}

export class SetActiveDocument extends JsMessage {
	document_index!: number;
}

export class DisplayError extends JsMessage {
	title!: string;

	description!: string;
}

export class DisplayPanic extends JsMessage {
	panic_info!: string;

	title!: string;

	description!: string;
}

export class DisplayConfirmationToCloseDocument extends JsMessage {
	document_index!: number;
}

export class DisplayConfirmationToCloseAllDocuments extends JsMessage {}

export class UpdateCanvas extends JsMessage {
	document!: string;
}

const TupleToVec2 = Transform(({ value }) => ({ x: value[0], y: value[1] }));

export class UpdateScrollbars extends JsMessage {
	@TupleToVec2
	position!: { x: number; y: number };

	@TupleToVec2
	size!: { x: number; y: number };

	@TupleToVec2
	multiplier!: { x: number; y: number };
}

export class UpdateRulers extends JsMessage {
	@TupleToVec2
	origin!: { x: number; y: number };

	spacing!: number;

	interval!: number;
}

export class ExportDocument extends JsMessage {
	document!: string;

	name!: string;
}

export class SaveDocument extends JsMessage {
	document!: string;

	name!: string;
}

export class OpenDocumentBrowse extends JsMessage {}

export class DocumentChanged extends JsMessage {}

export class DisplayFolderTreeStructure extends JsMessage {
	constructor(public layerId: BigInt, public children: DisplayFolderTreeStructure[]) {
		super();
	}
}
export function newDisplayFolderTreeStructure(input: any): DisplayFolderTreeStructure {
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

export class UpdateLayer extends JsMessage {
	@Type(() => LayerPanelEntry)
	public data!: LayerPanelEntry;
}

export class SetCanvasZoom extends JsMessage {
	new_zoom!: number;
}

export class SetCanvasRotation extends JsMessage {
	new_radians!: number;
}

function newPath(input: any): BigUint64Array {
	// eslint-disable-next-line
	const u32CombinedPairs = input.map((n: number[]) => BigInt((BigInt(n[0]) << BigInt(32)) | BigInt(n[1])));
	return new BigUint64Array(u32CombinedPairs);
}

export type BlendMode =
	| "normal"
	| "multiply"
	| "darken"
	| "color-burn"
	| "screen"
	| "lighten"
	| "color-dodge"
	| "overlay"
	| "soft-light"
	| "hard-light"
	| "difference"
	| "exclusion"
	| "hue"
	| "saturation"
	| "color"
	| "luminosity";

function newOpacity(input: number): number {
	return input * 100;
}

export class LayerPanelEntry {
	name!: string;

	visible!: boolean;

	blend_mode!: BlendMode;

	@Transform(({ value }) => newOpacity(value))
	opacity!: number;

	// No need to check the backend editor. Assume that it is always correct
	layer_type!: LayerType;

	@Transform(({ value }) => newPath(value))
	path!: BigUint64Array;

	@Type(() => LayerData)
	layer_data!: LayerData;

	thumbnail!: string;
}

export class LayerData {
	expanded!: boolean;

	selected!: boolean;
}

export const LayerTypeOptions = {
	Folder: "Folder",
	Shape: "Shape",
	Circle: "Circle",
	Rect: "Rect",
	Line: "Line",
	PolyLine: "PolyLine",
	Ellipse: "Ellipse",
} as const;

export type LayerType = typeof LayerTypeOptions[keyof typeof LayerTypeOptions];

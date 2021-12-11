/* eslint-disable @typescript-eslint/no-explicit-any */
/* eslint-disable camelcase */
/* eslint-disable max-classes-per-file */

import { Transform, Type } from "class-transformer";

// Wasm types are here to remove dependency cycle
export type WasmInstance = typeof import("@/../wasm/pkg");

export class JsMessage {
	// The marker provides a way to check if an object is a sub-class constructor for a jsMessage.
	static readonly jsMessageMarker = true;
}

export class UpdateOpenDocumentsList extends JsMessage {
	@Transform(({ value }) => value.map((tuple: [string, boolean]) => ({ name: tuple[0], isSaved: tuple[1] })))
	readonly open_documents!: { name: string; isSaved: boolean }[];
}

const To255Scale = Transform(({ value }) => value * 255);
export class Color {
	@To255Scale
	readonly red!: number;

	@To255Scale
	readonly green!: number;

	@To255Scale
	readonly blue!: number;

	readonly alpha!: number;

	toRgba() {
		return { r: this.red, g: this.green, b: this.blue, a: this.alpha };
	}

	toRgbaCSS() {
		const { r, g, b, a } = this.toRgba();
		return `rgba(${r}, ${g}, ${b}, ${a})`;
	}
}

export class UpdateWorkingColors extends JsMessage {
	@Type(() => Color)
	readonly primary!: Color;

	@Type(() => Color)
	readonly secondary!: Color;
}

export class SetActiveTool extends JsMessage {
	readonly tool_name!: string;

	readonly tool_options!: object;
}

export class SetActiveDocument extends JsMessage {
	readonly document_index!: number;
}

export class DisplayError extends JsMessage {
	readonly title!: string;

	readonly description!: string;
}

export class DisplayPanic extends JsMessage {
	readonly panic_info!: string;

	readonly title!: string;

	readonly description!: string;
}

export class DisplayConfirmationToCloseDocument extends JsMessage {
	readonly document_index!: number;
}

export class DisplayConfirmationToCloseAllDocuments extends JsMessage {}

export class UpdateCanvas extends JsMessage {
	readonly document!: string;
}

const TupleToVec2 = Transform(({ value }) => ({ x: value[0], y: value[1] }));

export class UpdateScrollbars extends JsMessage {
	@TupleToVec2
	readonly position!: { x: number; y: number };

	@TupleToVec2
	readonly size!: { x: number; y: number };

	@TupleToVec2
	readonly multiplier!: { x: number; y: number };
}

export class UpdateRulers extends JsMessage {
	@TupleToVec2
	readonly origin!: { x: number; y: number };

	readonly spacing!: number;

	readonly interval!: number;
}

export class ExportDocument extends JsMessage {
	readonly document!: string;

	readonly name!: string;
}

export class SaveDocument extends JsMessage {
	readonly document!: string;

	readonly name!: string;
}

export class OpenDocumentBrowse extends JsMessage {}

export class DocumentChanged extends JsMessage {}

export class DisplayFolderTreeStructure extends JsMessage {
	constructor(readonly layerId: BigInt, readonly children: DisplayFolderTreeStructure[]) {
		super();
	}
}
export function newDisplayFolderTreeStructure(input: any, wasm: WasmInstance): DisplayFolderTreeStructure {
	const { ptr, len } = input.data_buffer;
	const wasmMemoryBuffer = wasm.wasm_memory().buffer;

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
	readonly data!: LayerPanelEntry;
}

export class SetCanvasZoom extends JsMessage {
	readonly new_zoom!: number;
}

export class SetCanvasRotation extends JsMessage {
	readonly new_radians!: number;
}

function newPath(input: any): BigUint64Array {
	// eslint-disable-next-line
	const u32CombinedPairs = input.map((n: number[]) => BigInt((BigInt(n[0]) << BigInt(32)) | BigInt(n[1])));
	return new BigUint64Array(u32CombinedPairs);
}

export type BlendMode =
	| "Normal"
	| "Multiply"
	| "Darken"
	| "ColorBurn"
	| "Screen"
	| "Lighten"
	| "ColorDodge"
	| "Overlay"
	| "SoftLight"
	| "HardLight"
	| "Difference"
	| "Exclusion"
	| "Hue"
	| "Saturation"
	| "Color"
	| "Luminosity";

export class LayerPanelEntry {
	name!: string;

	visible!: boolean;

	blend_mode!: BlendMode;

	// On the rust side opacity is out of 1 rather than 100
	@Transform(({ value }) => value * 100)
	opacity!: number;

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

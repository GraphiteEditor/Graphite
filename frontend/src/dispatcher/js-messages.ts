/* eslint-disable camelcase */
/* eslint-disable max-classes-per-file */

import { Transform, Type } from "class-transformer";

import type { RustEditorInstance, WasmInstance } from "@/state/wasm-loader";

export class JsMessage {
	// The marker provides a way to check if an object is a sub-class constructor for a jsMessage.
	static readonly jsMessageMarker = true;
}

// ============================================================================
// Add additional classes to replicate Rust's FrontendMessages and data structures below.
//
// Remember to add each message to the `messageConstructors` export at the bottom of the file.
//
// Read class-transformer docs at https://github.com/typestack/class-transformer#table-of-contents
// for details about how to transform the JSON from wasm-bindgen into classes.
// ============================================================================

export class FrontendDocumentDetails {
	readonly name!: string;

	readonly is_saved!: boolean;

	readonly id!: BigInt;

	get displayName() {
		return `${this.name}${this.is_saved ? "" : "*"}`;
	}
}

export class UpdateOpenDocumentsList extends JsMessage {
	@Type(() => FrontendDocumentDetails)
	readonly open_documents!: FrontendDocumentDetails[];
}

export class UpdateInputHints extends JsMessage {
	@Type(() => HintInfo)
	readonly hint_data!: HintData;
}

export class HintGroup extends Array<HintInfo> {}

export class HintData extends Array<HintGroup> {}

export class HintInfo {
	readonly keys!: string[];

	readonly mouse!: KeysGroup | null;

	readonly label!: string;

	readonly plus!: boolean;
}

export class KeysGroup extends Array<string> {}

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
	readonly document_id!: BigInt;
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
	readonly document_id!: BigInt;
}

export class DisplayConfirmationToCloseAllDocuments extends JsMessage {}

export class DisplayAboutGraphiteDialog extends JsMessage {}

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

interface DataBuffer {
	pointer: BigInt;
	length: BigInt;
}

export function newDisplayFolderTreeStructure(input: { data_buffer: DataBuffer }, wasm: WasmInstance): DisplayFolderTreeStructure {
	const { pointer, length } = input.data_buffer;
	const pointerNum = Number(pointer);
	const lengthNum = Number(length);
	const wasmMemoryBuffer = wasm.wasm_memory().buffer;

	// Decode the folder structure encoding
	const encoding = new DataView(wasmMemoryBuffer, pointerNum, lengthNum);

	// The structure section indicates how to read through the upcoming layer list and assign depths to each layer
	const structureSectionLength = Number(encoding.getBigUint64(0, true));
	const structureSectionMsbSigned = new DataView(wasmMemoryBuffer, pointerNum + 8, structureSectionLength * 8);

	// The layer IDs section lists each layer ID sequentially in the tree, as it will show up in the panel
	const layerIdsSection = new DataView(wasmMemoryBuffer, pointerNum + 8 + structureSectionLength * 8);

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

	@Transform(({ value }) => new BigUint64Array(value))
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

// Any is used since the type of the object should be known from the rust side
// eslint-disable-next-line @typescript-eslint/no-explicit-any
type JSMessageFactory = (data: any, wasm: WasmInstance, instance: RustEditorInstance) => JsMessage;
type MessageMaker = typeof JsMessage | JSMessageFactory;

export const messageConstructors: Record<string, MessageMaker> = {
	UpdateCanvas,
	UpdateScrollbars,
	UpdateRulers,
	ExportDocument,
	SaveDocument,
	OpenDocumentBrowse,
	DisplayFolderTreeStructure: newDisplayFolderTreeStructure,
	UpdateLayer,
	SetActiveTool,
	SetActiveDocument,
	UpdateOpenDocumentsList,
	UpdateInputHints,
	UpdateWorkingColors,
	SetCanvasZoom,
	SetCanvasRotation,
	DisplayError,
	DisplayPanic,
	DisplayConfirmationToCloseDocument,
	DisplayConfirmationToCloseAllDocuments,
	DisplayAboutGraphiteDialog,
} as const;
export type JsMessageType = keyof typeof messageConstructors;

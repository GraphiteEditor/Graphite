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

// Allows the auto save system to use a string for the id rather than a BigInt.
// IndexedDb does not allow for BigInts as primary keys. TypeScript does not allow
// subclasses to change the type of class variables in subclasses. It is an abstract
// class to point out that it should not be instantiated directly.
export abstract class DocumentDetails {
	readonly name!: string;

	readonly is_saved!: boolean;

	readonly id!: BigInt | string;

	get displayName(): string {
		return `${this.name}${this.is_saved ? "" : "*"}`;
	}
}

export class FrontendDocumentDetails extends DocumentDetails {
	readonly id!: BigInt;
}

export class UpdateOpenDocumentsList extends JsMessage {
	@Type(() => FrontendDocumentDetails)
	readonly open_documents!: FrontendDocumentDetails[];
}

export class UpdateInputHints extends JsMessage {
	@Type(() => HintInfo)
	readonly hint_data!: HintData;
}

export type HintData = HintGroup[];

export type HintGroup = HintInfo[];

export class HintInfo {
	readonly key_groups!: KeysGroup[];

	readonly mouse!: MouseMotion | null;

	readonly label!: string;

	readonly plus!: boolean;
}

export type KeysGroup = string[]; // Array of Rust enum `Key`

export type MouseMotion = string;

export type RGBA = {
	r: number;
	g: number;
	b: number;
	a: number;
};

export type HSVA = {
	h: number;
	s: number;
	v: number;
	a: number;
};

const To255Scale = Transform(({ value }) => value * 255);
export class Color {
	@To255Scale
	readonly red!: number;

	@To255Scale
	readonly green!: number;

	@To255Scale
	readonly blue!: number;

	readonly alpha!: number;

	toRgba(): RGBA {
		return { r: this.red, g: this.green, b: this.blue, a: this.alpha };
	}

	toRgbaCSS(): string {
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

export type ToolName =
	| "Select"
	| "Crop"
	| "Navigate"
	| "Eyedropper"
	| "Text"
	| "Fill"
	| "Gradient"
	| "Brush"
	| "Heal"
	| "Clone"
	| "Patch"
	| "Detail"
	| "Relight"
	| "Path"
	| "Pen"
	| "Freehand"
	| "Spline"
	| "Line"
	| "Rectangle"
	| "Ellipse"
	| "Shape";

export class UpdateActiveTool extends JsMessage {
	readonly tool_name!: ToolName;
}

export class UpdateActiveDocument extends JsMessage {
	readonly document_id!: BigInt;
}

export class DisplayDialogError extends JsMessage {
	readonly title!: string;

	readonly description!: string;
}

export class DisplayDialogPanic extends JsMessage {
	readonly panic_info!: string;

	readonly title!: string;

	readonly description!: string;
}

export class DisplayConfirmationToCloseDocument extends JsMessage {
	readonly document_id!: BigInt;
}

export class DisplayConfirmationToCloseAllDocuments extends JsMessage {}

export class DisplayDialogAboutGraphite extends JsMessage {}

export class UpdateDocumentArtwork extends JsMessage {
	readonly svg!: string;
}

export class UpdateDocumentOverlays extends JsMessage {
	readonly svg!: string;
}

export class UpdateDocumentArtboards extends JsMessage {
	readonly svg!: string;
}

const TupleToVec2 = Transform(({ value }) => ({ x: value[0], y: value[1] }));

export class UpdateDocumentScrollbars extends JsMessage {
	@TupleToVec2
	readonly position!: { x: number; y: number };

	@TupleToVec2
	readonly size!: { x: number; y: number };

	@TupleToVec2
	readonly multiplier!: { x: number; y: number };
}

export class UpdateDocumentRulers extends JsMessage {
	@TupleToVec2
	readonly origin!: { x: number; y: number };

	readonly spacing!: number;

	readonly interval!: number;
}

export type MouseCursorIcon = "default" | "zoom-in" | "zoom-out" | "grabbing" | "crosshair" | "text";

const ToCssCursorProperty = Transform(({ value }) => {
	const cssNames: Record<string, MouseCursorIcon> = {
		ZoomIn: "zoom-in",
		ZoomOut: "zoom-out",
		Grabbing: "grabbing",
		Crosshair: "crosshair",
		Text: "text",
	};

	return cssNames[value] || "default";
});

export class UpdateMouseCursor extends JsMessage {
	@ToCssCursorProperty
	readonly cursor!: MouseCursorIcon;
}

export class TriggerFileDownload extends JsMessage {
	readonly document!: string;

	readonly name!: string;
}

export class TriggerFileUpload extends JsMessage {}

export class DocumentChanged extends JsMessage {}

export class DisplayDocumentLayerTreeStructure extends JsMessage {
	constructor(readonly layerId: BigInt, readonly children: DisplayDocumentLayerTreeStructure[]) {
		super();
	}
}

interface DataBuffer {
	pointer: BigInt;
	length: BigInt;
}

export function newDisplayDocumentLayerTreeStructure(input: { data_buffer: DataBuffer }, wasm: WasmInstance): DisplayDocumentLayerTreeStructure {
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
	let currentFolder = new DisplayDocumentLayerTreeStructure(BigInt(-1), []);
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

			const childLayer = new DisplayDocumentLayerTreeStructure(layerId, []);
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

export class DisplayEditableTextbox extends JsMessage {
	readonly text!: string;

	readonly line_width!: undefined | number;

	readonly font_size!: number;
}

export class DisplayRemoveEditableTextbox extends JsMessage {}

export class UpdateDocumentLayer extends JsMessage {
	@Type(() => LayerPanelEntry)
	readonly data!: LayerPanelEntry;
}

export class UpdateCanvasZoom extends JsMessage {
	readonly factor!: number;
}

export class UpdateCanvasRotation extends JsMessage {
	readonly angle_radians!: number;
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

	@Type(() => LayerMetadata)
	layer_metadata!: LayerMetadata;

	thumbnail!: string;
}

export class LayerMetadata {
	expanded!: boolean;

	selected!: boolean;
}

export type LayerType = "Folder" | "Shape" | "Circle" | "Rect" | "Line" | "PolyLine" | "Ellipse";

export class IndexedDbDocumentDetails extends DocumentDetails {
	@Transform(({ value }: { value: BigInt }) => value.toString())
	id!: string;
}

export class TriggerIndexedDbWriteDocument extends JsMessage {
	document!: string;

	@Type(() => IndexedDbDocumentDetails)
	details!: IndexedDbDocumentDetails;

	version!: string;
}

export class TriggerIndexedDbRemoveDocument extends JsMessage {
	// Use a string since IndexedDB can not use BigInts for keys
	@Transform(({ value }: { value: BigInt }) => value.toString())
	document_id!: string;
}

export interface WidgetLayout {
	layout_target: unknown;
	layout: LayoutRow[];
}

export function defaultWidgetLayout(): WidgetLayout {
	return {
		layout: [],
		layout_target: null,
	};
}

export type LayoutRow = WidgetRow | WidgetSection;

export type WidgetRow = { name: string; widgets: Widget[] };
export function isWidgetRow(layoutRow: WidgetRow | WidgetSection): layoutRow is WidgetRow {
	return Boolean((layoutRow as WidgetRow).widgets);
}

export type WidgetSection = { name: string; layout: LayoutRow[] };
export function isWidgetSection(layoutRow: WidgetRow | WidgetSection): layoutRow is WidgetSection {
	return Boolean((layoutRow as WidgetSection).layout);
}

export type WidgetKind = "NumberInput" | "Separator" | "IconButton" | "PopoverButton" | "OptionalInput" | "RadioInput";

export interface Widget {
	kind: WidgetKind;
	widget_id: BigInt;
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	props: any;
}

export class UpdateToolOptionsLayout extends JsMessage implements WidgetLayout {
	layout_target!: unknown;

	@Transform(({ value }) => createWidgetLayout(value))
	layout!: LayoutRow[];
}

export class UpdateDocumentBarLayout extends JsMessage {
	layout_target!: unknown;

	@Transform(({ value }) => createWidgetLayout(value))
	layout!: LayoutRow[];
}

// Unpacking rust types to more usable type in the frontend
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function createWidgetLayout(widgetLayout: any[]): LayoutRow[] {
	return widgetLayout.map((rowOrSection) => {
		if (rowOrSection.Row) {
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			const widgets = rowOrSection.Row.widgets.map((widgetHolder: any) => {
				const { widget_id } = widgetHolder;
				const kind = Object.keys(widgetHolder.widget)[0];
				const props = widgetHolder.widget[kind];

				return { widget_id, kind, props };
			});

			return {
				name: rowOrSection.Row.name,
				widgets,
			};
		}
		if (rowOrSection.Section) {
			return {
				name: rowOrSection.Section.name,
				layout: createWidgetLayout(rowOrSection.Section),
			};
		}

		throw new Error("Layout row type does not exist");
	});
}

export class DisplayDialogComingSoon extends JsMessage {
	issue: number | undefined;
}

export class TriggerTextCommit extends JsMessage {}

// Any is used since the type of the object should be known from the rust side
// eslint-disable-next-line @typescript-eslint/no-explicit-any
type JSMessageFactory = (data: any, wasm: WasmInstance, instance: RustEditorInstance) => JsMessage;
type MessageMaker = typeof JsMessage | JSMessageFactory;

export const messageConstructors: Record<string, MessageMaker> = {
	UpdateDocumentArtwork,
	UpdateDocumentOverlays,
	UpdateDocumentScrollbars,
	UpdateDocumentRulers,
	TriggerFileDownload,
	TriggerFileUpload,
	DisplayDocumentLayerTreeStructure: newDisplayDocumentLayerTreeStructure,
	DisplayEditableTextbox,
	DisplayRemoveEditableTextbox,
	UpdateDocumentLayer,
	UpdateActiveTool,
	UpdateActiveDocument,
	UpdateOpenDocumentsList,
	UpdateInputHints,
	UpdateWorkingColors,
	UpdateCanvasZoom,
	UpdateCanvasRotation,
	UpdateMouseCursor,
	DisplayDialogError,
	DisplayDialogPanic,
	DisplayConfirmationToCloseDocument,
	DisplayConfirmationToCloseAllDocuments,
	DisplayDialogAboutGraphite,
	TriggerIndexedDbWriteDocument,
	TriggerIndexedDbRemoveDocument,
	TriggerTextCommit,
	UpdateDocumentArtboards,
	UpdateToolOptionsLayout,
	DisplayDialogComingSoon,
	UpdateDocumentBarLayout,
} as const;
export type JsMessageType = keyof typeof messageConstructors;

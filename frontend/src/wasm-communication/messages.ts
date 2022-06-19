/* eslint-disable camelcase */
/* eslint-disable max-classes-per-file */

import { Transform, Type } from "class-transformer";

import { IconName } from "@/utility-functions/icons";
import type { WasmEditorInstance, WasmRawInstance } from "@/wasm-communication/editor";

export class JsMessage {
	// The marker provides a way to check if an object is a sub-class constructor for a jsMessage.
	static readonly jsMessageMarker = true;
}

// ============================================================================
// Add additional classes to replicate Rust's `FrontendMessage`s and data structures below.
//
// Remember to add each message to the `messageConstructors` export at the bottom of the file.
//
// Read class-transformer docs at https://github.com/typestack/class-transformer#table-of-contents
// for details about how to transform the JSON from wasm-bindgen into classes.
// ============================================================================

// Allows the auto save system to use a string for the id rather than a BigInt.
// IndexedDb does not allow for BigInts as primary keys.
// TypeScript does not allow subclasses to change the type of class variables in subclasses.
// It is an abstract class to point out that it should not be instantiated directly.
export abstract class DocumentDetails {
	readonly name!: string;

	readonly is_saved!: boolean;

	readonly id!: bigint | string;

	get displayName(): string {
		return `${this.name}${this.is_saved ? "" : "*"}`;
	}
}

export class FrontendDocumentDetails extends DocumentDetails {
	readonly id!: bigint;
}

export class UpdateNodeGraphVisibility extends JsMessage {
	readonly visible!: boolean;
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

export class UpdateActiveDocument extends JsMessage {
	readonly document_id!: bigint;
}

export class DisplayDialogPanic extends JsMessage {
	readonly panic_info!: string;

	readonly title!: string;

	readonly description!: string;
}

export class DisplayDialog extends JsMessage {
	readonly icon!: IconName;
}

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

export type MouseCursorIcon = "default" | "zoom-in" | "zoom-out" | "grabbing" | "crosshair" | "text" | "ns-resize" | "ew-resize" | "nesw-resize" | "nwse-resize";

const ToCssCursorProperty = Transform(({ value }) => {
	const cssNames: Record<string, MouseCursorIcon> = {
		ZoomIn: "zoom-in",
		ZoomOut: "zoom-out",
		Grabbing: "grabbing",
		Crosshair: "crosshair",
		Text: "text",
		NSResize: "ns-resize",
		EWResize: "ew-resize",
		NESWResize: "nesw-resize",
		NWSEResize: "nwse-resize",
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

export class TriggerPaste extends JsMessage {}

export class TriggerRasterDownload extends JsMessage {
	readonly document!: string;

	readonly name!: string;

	readonly mime!: string;

	@TupleToVec2
	readonly size!: { x: number; y: number };
}

export class DocumentChanged extends JsMessage {}

export class UpdateDocumentLayerTreeStructure extends JsMessage {
	constructor(readonly layerId: bigint, readonly children: UpdateDocumentLayerTreeStructure[]) {
		super();
	}
}

interface DataBuffer {
	pointer: bigint;
	length: bigint;
}

export function newUpdateDocumentLayerTreeStructure(input: { data_buffer: DataBuffer }, wasm: WasmRawInstance): UpdateDocumentLayerTreeStructure {
	const pointerNum = Number(input.data_buffer.pointer);
	const lengthNum = Number(input.data_buffer.length);

	const wasmMemoryBuffer = wasm.wasm_memory().buffer;

	// Decode the folder structure encoding
	const encoding = new DataView(wasmMemoryBuffer, pointerNum, lengthNum);

	// The structure section indicates how to read through the upcoming layer list and assign depths to each layer
	const structureSectionLength = Number(encoding.getBigUint64(0, true));
	const structureSectionMsbSigned = new DataView(wasmMemoryBuffer, pointerNum + 8, structureSectionLength * 8);

	// The layer IDs section lists each layer ID sequentially in the tree, as it will show up in the panel
	const layerIdsSection = new DataView(wasmMemoryBuffer, pointerNum + 8 + structureSectionLength * 8);

	let layersEncountered = 0;
	let currentFolder = new UpdateDocumentLayerTreeStructure(BigInt(-1), []);
	const currentFolderStack = [currentFolder];

	for (let i = 0; i < structureSectionLength; i += 1) {
		const msbSigned = structureSectionMsbSigned.getBigUint64(i * 8, true);
		const msbMask = BigInt(1) << BigInt(64 - 1);

		// Set the MSB to 0 to clear the sign and then read the number as usual
		const numberOfLayersAtThisDepth = msbSigned & ~msbMask;

		// Store child folders in the current folder (until we are interrupted by an indent)
		for (let j = 0; j < numberOfLayersAtThisDepth; j += 1) {
			const layerId = layerIdsSection.getBigUint64(layersEncountered * 8, true);
			layersEncountered += 1;

			const childLayer = new UpdateDocumentLayerTreeStructure(layerId, []);
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

	@Type(() => Color)
	readonly color!: Color;
}

export class UpdateImageData extends JsMessage {
	readonly image_data!: ImageData[];
}

export class DisplayRemoveEditableTextbox extends JsMessage {}

export class UpdateDocumentLayerDetails extends JsMessage {
	@Type(() => LayerPanelEntry)
	readonly data!: LayerPanelEntry;
}

export class LayerPanelEntry {
	name!: string;

	visible!: boolean;

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

export type LayerType = "Folder" | "Image" | "Shape" | "Text";

export class ImageData {
	readonly path!: BigUint64Array;

	readonly mime!: string;

	readonly image_data!: Uint8Array;
}

export class IndexedDbDocumentDetails extends DocumentDetails {
	@Transform(({ value }: { value: bigint }) => value.toString())
	id!: string;
}

export class DisplayDialogDismiss extends JsMessage {}

export class TriggerIndexedDbWriteDocument extends JsMessage {
	document!: string;

	@Type(() => IndexedDbDocumentDetails)
	details!: IndexedDbDocumentDetails;

	version!: string;
}

export class TriggerIndexedDbRemoveDocument extends JsMessage {
	// Use a string since IndexedDB can not use BigInts for keys
	@Transform(({ value }: { value: bigint }) => value.toString())
	document_id!: string;
}

export class Font {
	font_family!: string;

	font_style!: string;
}

export class TriggerFontLoad extends JsMessage {
	@Type(() => Font)
	font!: Font;

	is_default!: boolean;
}

export class TriggerVisitLink extends JsMessage {
	url!: string;
}

export interface WidgetLayout {
	layout: LayoutGroup[];
	layout_target: unknown;
}

export function defaultWidgetLayout(): WidgetLayout {
	return {
		layout: [],
		layout_target: null,
	};
}

export type LayoutGroup = WidgetRow | WidgetColumn | WidgetSection;

export type WidgetColumn = { columnWidgets: Widget[] };
export function isWidgetColumn(layoutColumn: LayoutGroup): layoutColumn is WidgetColumn {
	return Boolean((layoutColumn as WidgetColumn).columnWidgets);
}

export type WidgetRow = { rowWidgets: Widget[] };
export function isWidgetRow(layoutRow: LayoutGroup): layoutRow is WidgetRow {
	return Boolean((layoutRow as WidgetRow).rowWidgets);
}

export type WidgetSection = { name: string; layout: LayoutGroup[] };
export function isWidgetSection(layoutRow: LayoutGroup): layoutRow is WidgetSection {
	return Boolean((layoutRow as WidgetSection).layout);
}

export type WidgetKind =
	| "CheckboxInput"
	| "ColorInput"
	| "DropdownInput"
	| "FontInput"
	| "IconButton"
	| "IconLabel"
	| "NumberInput"
	| "OptionalInput"
	| "PopoverButton"
	| "RadioInput"
	| "Separator"
	| "TextAreaInput"
	| "TextButton"
	| "TextInput"
	| "TextLabel";

export interface Widget {
	kind: WidgetKind;
	widget_id: bigint;
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	props: any;
}

export class UpdateDialogDetails extends JsMessage implements WidgetLayout {
	layout_target!: unknown;

	@Transform(({ value }) => createWidgetLayout(value))
	layout!: LayoutGroup[];
}

export class UpdateDocumentModeLayout extends JsMessage implements WidgetLayout {
	layout_target!: unknown;

	@Transform(({ value }) => createWidgetLayout(value))
	layout!: LayoutGroup[];
}

export class UpdateToolOptionsLayout extends JsMessage implements WidgetLayout {
	layout_target!: unknown;

	@Transform(({ value }) => createWidgetLayout(value))
	layout!: LayoutGroup[];
}

export class UpdateDocumentBarLayout extends JsMessage implements WidgetLayout {
	layout_target!: unknown;

	@Transform(({ value }) => createWidgetLayout(value))
	layout!: LayoutGroup[];
}

export class UpdateToolShelfLayout extends JsMessage implements WidgetLayout {
	layout_target!: unknown;

	@Transform(({ value }) => createWidgetLayout(value))
	layout!: LayoutGroup[];
}

export class UpdatePropertyPanelOptionsLayout extends JsMessage implements WidgetLayout {
	layout_target!: unknown;

	@Transform(({ value }) => createWidgetLayout(value))
	layout!: LayoutGroup[];
}

export class UpdatePropertyPanelSectionsLayout extends JsMessage implements WidgetLayout {
	layout_target!: unknown;

	@Transform(({ value }) => createWidgetLayout(value))
	layout!: LayoutGroup[];
}

export class UpdateLayerTreeOptionsLayout extends JsMessage implements WidgetLayout {
	layout_target!: unknown;

	@Transform(({ value }) => createWidgetLayout(value))
	layout!: LayoutGroup[];
}

// Unpacking rust types to more usable type in the frontend
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function createWidgetLayout(widgetLayout: any[]): LayoutGroup[] {
	return widgetLayout.map((layoutType): LayoutGroup => {
		if (layoutType.Column) {
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			const columnWidgets = hoistWidgetHolders(layoutType.Column.columnWidgets);
			const result: WidgetColumn = { columnWidgets };
			return result;
		}

		if (layoutType.Row) {
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			const rowWidgets = hoistWidgetHolders(layoutType.Row.rowWidgets);
			const result: WidgetRow = { rowWidgets };
			return result;
		}

		if (layoutType.Section) {
			const { name } = layoutType.Section;
			const layout = createWidgetLayout(layoutType.Section.layout);

			const result: WidgetSection = { name, layout };
			return result;
		}

		throw new Error("Layout row type does not exist");
	});
}
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function hoistWidgetHolders(widgetHolders: any[]): Widget[] {
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	return widgetHolders.map((widgetHolder: any) => {
		const { widget_id } = widgetHolder;
		const kind = Object.keys(widgetHolder.widget)[0];
		const props = widgetHolder.widget[kind];

		return { widget_id, kind, props } as Widget;
	});
}

export class UpdateMenuBarLayout extends JsMessage {
	layout_target!: unknown;

	@Transform(({ value }) => createMenuLayout(value))
	layout!: MenuColumn[];
}

export type MenuEntry = {
	shortcut: string[] | undefined;
	action: Widget;
	label: string;
	icon: string | undefined;
	children: undefined | MenuEntry[][];
};

export type MenuColumn = {
	label: string;
	children: MenuEntry[][];
};

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function createMenuLayout(menuLayout: any[]): MenuColumn[] {
	return menuLayout.map((column) => ({ ...column, children: createMenuLayoutRecursive(column.children) }));
}
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function createMenuLayoutRecursive(subLayout: any[][]): MenuEntry[][] {
	return subLayout.map((groups) =>
		groups.map((entry) => ({
			...entry,
			action: hoistWidgetHolders([entry.action])[0],
			children: entry.children ? createMenuLayoutRecursive(entry.children) : undefined,
		}))
	);
}

export class TriggerTextCommit extends JsMessage {}

export class TriggerTextCopy extends JsMessage {
	readonly copy_text!: string;
}

export class TriggerAboutGraphiteLocalizedCommitDate extends JsMessage {
	readonly commit_date!: string;
}

export class TriggerViewportResize extends JsMessage {}

// `any` is used since the type of the object should be known from the Rust side
// eslint-disable-next-line @typescript-eslint/no-explicit-any
type JSMessageFactory = (data: any, wasm: WasmRawInstance, instance: WasmEditorInstance) => JsMessage;
type MessageMaker = typeof JsMessage | JSMessageFactory;

export const messageMakers: Record<string, MessageMaker> = {
	DisplayDialog,
	DisplayDialogPanic,
	UpdateDocumentLayerTreeStructure: newUpdateDocumentLayerTreeStructure,
	DisplayEditableTextbox,
	UpdateImageData,
	DisplayRemoveEditableTextbox,
	DisplayDialogDismiss,
	TriggerFileDownload,
	TriggerFileUpload,
	TriggerIndexedDbRemoveDocument,
	TriggerFontLoad,
	TriggerIndexedDbWriteDocument,
	TriggerPaste,
	TriggerRasterDownload,
	TriggerTextCommit,
	TriggerTextCopy,
	TriggerAboutGraphiteLocalizedCommitDate,
	TriggerViewportResize,
	TriggerVisitLink,
	UpdateActiveDocument,
	UpdateDialogDetails,
	UpdateDocumentArtboards,
	UpdateDocumentArtwork,
	UpdateDocumentBarLayout,
	UpdateToolShelfLayout,
	UpdateDocumentLayerDetails,
	UpdateDocumentOverlays,
	UpdateDocumentRulers,
	UpdateDocumentScrollbars,
	UpdateInputHints,
	UpdateMouseCursor,
	UpdateNodeGraphVisibility,
	UpdateOpenDocumentsList,
	UpdatePropertyPanelOptionsLayout,
	UpdatePropertyPanelSectionsLayout,
	UpdateLayerTreeOptionsLayout,
	UpdateDocumentModeLayout,
	UpdateToolOptionsLayout,
	UpdateWorkingColors,
	UpdateMenuBarLayout,
} as const;
export type JsMessageType = keyof typeof messageMakers;

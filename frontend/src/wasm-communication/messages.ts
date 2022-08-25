/* eslint-disable max-classes-per-file */

import { Transform, Type, plainToClass } from "class-transformer";

import type { IconName, IconSize, IconStyle } from "@/utility-functions/icons";
import type { WasmEditorInstance, WasmRawInstance } from "@/wasm-communication/editor";

import type MenuList from "@/components/floating-menus/MenuList.vue";

export class JsMessage {
	// The marker provides a way to check if an object is a sub-class constructor for a jsMessage.
	static readonly jsMessageMarker = true;
}

// ============================================================================
// Add additional classes below to replicate Rust's `FrontendMessage`s and data structures.
//
// Remember to add each message to the `messageConstructors` export at the bottom of the file.
//
// Read class-transformer docs at https://github.com/typestack/class-transformer#table-of-contents
// for details about how to transform the JSON from wasm-bindgen into classes.
// ============================================================================

export class UpdateNodeGraphVisibility extends JsMessage {
	readonly visible!: boolean;
}

export class UpdateOpenDocumentsList extends JsMessage {
	@Type(() => FrontendDocumentDetails)
	readonly openDocuments!: FrontendDocumentDetails[];
}

// Allows the auto save system to use a string for the id rather than a BigInt.
// IndexedDb does not allow for BigInts as primary keys.
// TypeScript does not allow subclasses to change the type of class variables in subclasses.
// It is an abstract class to point out that it should not be instantiated directly.
export abstract class DocumentDetails {
	readonly name!: string;

	readonly isSaved!: boolean;

	readonly id!: bigint | string;

	get displayName(): string {
		return `${this.name}${this.isSaved ? "" : "*"}`;
	}
}

export class FrontendDocumentDetails extends DocumentDetails {
	readonly id!: bigint;
}

export class TriggerIndexedDbWriteDocument extends JsMessage {
	document!: string;

	@Type(() => IndexedDbDocumentDetails)
	details!: IndexedDbDocumentDetails;

	version!: string;
}

export class IndexedDbDocumentDetails extends DocumentDetails {
	@Transform(({ value }: { value: bigint }) => value.toString())
	id!: string;
}

export class TriggerIndexedDbRemoveDocument extends JsMessage {
	// Use a string since IndexedDB can not use BigInts for keys
	@Transform(({ value }: { value: bigint }) => value.toString())
	documentId!: string;
}

export class UpdateInputHints extends JsMessage {
	@Type(() => HintInfo)
	readonly hintData!: HintData;
}

export type HintData = HintGroup[];

export type HintGroup = HintInfo[];

export class HintInfo {
	readonly keyGroups!: KeysGroup[];

	readonly keyGroupsMac!: KeysGroup[] | null;

	readonly mouse!: MouseMotion | null;

	readonly label!: string;

	readonly plus!: boolean;
}

// Rust enum `Key`
export type KeyRaw = string;
// Serde converts a Rust `Key` enum variant into this format (via a custom serializer) with both the `Key` variant name (called `RawKey` in TS) and the localized `label` for the key
export type Key = { key: KeyRaw; label: string };
export type KeysGroup = Key[];
export type ActionKeys = { keys: KeysGroup };

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

const To255Scale = Transform(({ value }: { value: number }) => value * 255);
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

export class UpdateActiveDocument extends JsMessage {
	readonly documentId!: bigint;
}

export class DisplayDialogPanic extends JsMessage {
	readonly panicInfo!: string;

	readonly header!: string;

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

const TupleToVec2 = Transform(({ value }: { value: [number, number] }) => ({ x: value[0], y: value[1] }));

export type XY = { x: number; y: number };

export class UpdateDocumentScrollbars extends JsMessage {
	@TupleToVec2
	readonly position!: XY;

	@TupleToVec2
	readonly size!: XY;

	@TupleToVec2
	readonly multiplier!: XY;
}

export class UpdateDocumentRulers extends JsMessage {
	@TupleToVec2
	readonly origin!: XY;

	readonly spacing!: number;

	readonly interval!: number;
}

const mouseCursorIconCSSNames = {
	ZoomIn: "zoom-in",
	ZoomOut: "zoom-out",
	Grabbing: "grabbing",
	Crosshair: "crosshair",
	Text: "text",
	NSResize: "ns-resize",
	EWResize: "ew-resize",
	NESWResize: "nesw-resize",
	NWSEResize: "nwse-resize",
} as const;
export type MouseCursor = keyof typeof mouseCursorIconCSSNames;
export type MouseCursorIcon = typeof mouseCursorIconCSSNames[MouseCursor];

export class UpdateMouseCursor extends JsMessage {
	@Transform(({ value }: { value: MouseCursor }) => mouseCursorIconCSSNames[value] || "default")
	readonly cursor!: MouseCursorIcon;
}

export class TriggerFileDownload extends JsMessage {
	readonly document!: string;

	readonly name!: string;
}

export class TriggerOpenDocument extends JsMessage {}

export class TriggerImport extends JsMessage {}

export class TriggerPaste extends JsMessage {}

export class TriggerRasterDownload extends JsMessage {
	readonly document!: string;

	readonly name!: string;

	readonly mime!: string;

	@TupleToVec2
	readonly size!: XY;
}

export class TriggerRefreshBoundsOfViewports extends JsMessage {}

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

export function newUpdateDocumentLayerTreeStructure(input: { dataBuffer: DataBuffer }, wasm: WasmRawInstance): UpdateDocumentLayerTreeStructure {
	const pointerNum = Number(input.dataBuffer.pointer);
	const lengthNum = Number(input.dataBuffer.length);

	const wasmMemoryBuffer = wasm.wasmMemory().buffer;

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

	readonly lineWidth!: undefined | number;

	readonly fontSize!: number;

	@Type(() => Color)
	readonly color!: Color;
}

export class UpdateImageData extends JsMessage {
	@Type(() => ImageData)
	readonly imageData!: ImageData[];
}

export class DisplayRemoveEditableTextbox extends JsMessage {}

export class UpdateDocumentLayerDetails extends JsMessage {
	@Type(() => LayerPanelEntry)
	readonly data!: LayerPanelEntry;
}

export class LayerPanelEntry {
	name!: string;

	tooltip!: string;

	visible!: boolean;

	layerType!: LayerType;

	@Transform(({ value }: { value: bigint[] }) => new BigUint64Array(value))
	path!: BigUint64Array;

	@Type(() => LayerMetadata)
	layerMetadata!: LayerMetadata;

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

	readonly imageData!: Uint8Array;
}

export class DisplayDialogDismiss extends JsMessage {}

export class Font {
	fontFamily!: string;

	fontStyle!: string;
}

export class TriggerFontLoad extends JsMessage {
	@Type(() => Font)
	font!: Font;

	isDefault!: boolean;
}

export class TriggerVisitLink extends JsMessage {
	url!: string;
}

export class TriggerTextCommit extends JsMessage {}

export class TriggerTextCopy extends JsMessage {
	readonly copyText!: string;
}

export class TriggerAboutGraphiteLocalizedCommitDate extends JsMessage {
	readonly commitDate!: string;
}

export class TriggerViewportResize extends JsMessage {}

// WIDGET PROPS

export abstract class WidgetProps {
	kind!: string;
}

export class CheckboxInput extends WidgetProps {
	checked!: boolean;

	icon!: IconName;

	tooltip!: string;
}

export class ColorInput extends WidgetProps {
	value!: string | undefined;

	label!: string | undefined;

	noTransparency!: boolean;

	disabled!: boolean;

	tooltip!: string;
}

type MenuEntryCommon = {
	label: string;
	icon?: IconName;
	shortcut?: ActionKeys;
};

// The entry in the expanded menu or a sub-menu as received from the Rust backend
export type MenuBarEntry = MenuEntryCommon & {
	action: Widget;
	children?: MenuBarEntry[][];
};

// An entry in the all-encompassing MenuList component which defines all types of menus ranging from `MenuBarInput` to `DropdownInput` widgets
export type MenuListEntry = MenuEntryCommon & {
	action?: () => void;
	children?: MenuListEntry[][];

	shortcutRequiresLock?: boolean;
	value?: string;
	disabled?: boolean;
	font?: URL;
	ref?: InstanceType<typeof MenuList>;
};

export class DropdownInput extends WidgetProps {
	entries!: MenuListEntry[][];

	selectedIndex!: number | undefined;

	drawIcon!: boolean;

	interactive!: boolean;

	disabled!: boolean;
}

export class FontInput extends WidgetProps {
	fontFamily!: string;

	fontStyle!: string;

	isStyle!: boolean;

	disabled!: boolean;
}

export class IconButton extends WidgetProps {
	icon!: IconName;

	size!: IconSize;

	active!: boolean;

	tooltip!: string;
}

export class IconLabel extends WidgetProps {
	icon!: IconName;

	iconStyle!: IconStyle | undefined;
}

export type IncrementBehavior = "Add" | "Multiply" | "Callback" | "None";

export class NumberInput extends WidgetProps {
	label!: string | undefined;

	value!: number | undefined;

	min!: number | undefined;

	max!: number | undefined;

	isInteger!: boolean;

	displayDecimalPlaces!: number;

	unit!: string;

	unitIsHiddenWhenEditing!: boolean;

	incrementBehavior!: IncrementBehavior;

	incrementFactor!: number;

	disabled!: boolean;
}

export class OptionalInput extends WidgetProps {
	checked!: boolean;

	icon!: IconName;

	tooltip!: string;
}

export class PopoverButton extends WidgetProps {
	icon!: string | undefined;

	// Body
	header!: string;

	text!: string;
}

export interface RadioEntryData {
	value?: string;
	label?: string;
	icon?: IconName;
	tooltip?: string;

	// Callbacks
	action?: () => void;
}
export type RadioEntries = RadioEntryData[];

export class RadioInput extends WidgetProps {
	entries!: RadioEntries;

	selectedIndex!: number;
}

export type SeparatorDirection = "Horizontal" | "Vertical";
export type SeparatorType = "Related" | "Unrelated" | "Section" | "List";

export class Separator extends WidgetProps {
	direction!: SeparatorDirection;

	type!: SeparatorType;
}

export class SwatchPairInput extends WidgetProps {
	@Type(() => Color)
	primary!: Color;

	@Type(() => Color)
	secondary!: Color;
}

export class TextAreaInput extends WidgetProps {
	value!: string;

	label!: string | undefined;

	disabled!: boolean;
}

export class TextButton extends WidgetProps {
	label!: string;

	icon!: string | undefined;

	emphasized!: boolean;

	minWidth!: number;

	disabled!: boolean;
}

export class TextInput extends WidgetProps {
	value!: string;

	label!: string | undefined;

	disabled!: boolean;
}

export class TextLabel extends WidgetProps {
	// Body
	value!: string;

	// Props
	bold!: boolean;

	italic!: boolean;

	tableAlign!: boolean;

	multiline!: boolean;
}

// WIDGET

const widgetSubTypes = [
	{ value: CheckboxInput, name: "CheckboxInput" },
	{ value: ColorInput, name: "ColorInput" },
	{ value: DropdownInput, name: "DropdownInput" },
	{ value: FontInput, name: "FontInput" },
	{ value: IconButton, name: "IconButton" },
	{ value: IconLabel, name: "IconLabel" },
	{ value: NumberInput, name: "NumberInput" },
	{ value: OptionalInput, name: "OptionalInput" },
	{ value: PopoverButton, name: "PopoverButton" },
	{ value: RadioInput, name: "RadioInput" },
	{ value: Separator, name: "Separator" },
	{ value: SwatchPairInput, name: "SwatchPairInput" },
	{ value: TextAreaInput, name: "TextAreaInput" },
	{ value: TextButton, name: "TextButton" },
	{ value: TextInput, name: "TextInput" },
	{ value: TextLabel, name: "TextLabel" },
];
export type WidgetPropsSet = InstanceType<typeof widgetSubTypes[number]["value"]>;

export class Widget {
	constructor(props: WidgetPropsSet, widgetId: bigint) {
		this.props = props;
		this.widgetId = widgetId;
	}

	@Type(() => WidgetProps, { discriminator: { property: "kind", subTypes: widgetSubTypes }, keepDiscriminatorProperty: true })
	props!: WidgetPropsSet;

	widgetId!: bigint;
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function hoistWidgetHolders(widgetHolders: any[]): Widget[] {
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	return widgetHolders.map((widgetHolder: any) => {
		const kind = Object.keys(widgetHolder.widget)[0];
		const props = widgetHolder.widget[kind];
		props.kind = kind;

		const { widgetId } = widgetHolder;

		return plainToClass(Widget, { props, widgetId });
	});
}

// WIDGET LAYOUT

export interface WidgetLayout {
	layoutTarget: unknown;
	layout: LayoutGroup[];
}

export function defaultWidgetLayout(): WidgetLayout {
	return {
		layoutTarget: null,
		layout: [],
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

// Unpacking rust types to more usable type in the frontend
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function createWidgetLayout(widgetLayout: any[]): LayoutGroup[] {
	return widgetLayout.map((layoutType): LayoutGroup => {
		if (layoutType.column) {
			const columnWidgets = hoistWidgetHolders(layoutType.column.columnWidgets);

			const result: WidgetColumn = { columnWidgets };
			return result;
		}

		if (layoutType.row) {
			const rowWidgets = hoistWidgetHolders(layoutType.row.rowWidgets);

			const result: WidgetRow = { rowWidgets };
			return result;
		}

		if (layoutType.section) {
			const { name } = layoutType.section;
			const layout = createWidgetLayout(layoutType.section.layout);

			const result: WidgetSection = { name, layout };
			return result;
		}

		throw new Error("Layout row type does not exist");
	});
}

// WIDGET LAYOUTS

export class UpdateDialogDetails extends JsMessage implements WidgetLayout {
	layoutTarget!: unknown;

	// TODO: Replace `any` with correct typing
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	@Transform(({ value }: { value: any }) => createWidgetLayout(value))
	layout!: LayoutGroup[];
}

export class UpdateDocumentModeLayout extends JsMessage implements WidgetLayout {
	layoutTarget!: unknown;

	// TODO: Replace `any` with correct typing
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	@Transform(({ value }: { value: any }) => createWidgetLayout(value))
	layout!: LayoutGroup[];
}

export class UpdateToolOptionsLayout extends JsMessage implements WidgetLayout {
	layoutTarget!: unknown;

	// TODO: Replace `any` with correct typing
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	@Transform(({ value }: { value: any }) => createWidgetLayout(value))
	layout!: LayoutGroup[];
}

export class UpdateDocumentBarLayout extends JsMessage implements WidgetLayout {
	layoutTarget!: unknown;

	// TODO: Replace `any` with correct typing
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	@Transform(({ value }: { value: any }) => createWidgetLayout(value))
	layout!: LayoutGroup[];
}

export class UpdateToolShelfLayout extends JsMessage implements WidgetLayout {
	layoutTarget!: unknown;

	// TODO: Replace `any` with correct typing
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	@Transform(({ value }: { value: any }) => createWidgetLayout(value))
	layout!: LayoutGroup[];
}

export class UpdateWorkingColorsLayout extends JsMessage implements WidgetLayout {
	layoutTarget!: unknown;

	// TODO: Replace `any` with correct typing
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	@Transform(({ value }: { value: any }) => createWidgetLayout(value))
	layout!: LayoutGroup[];
}

export class UpdatePropertyPanelOptionsLayout extends JsMessage implements WidgetLayout {
	layoutTarget!: unknown;

	// TODO: Replace `any` with correct typing
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	@Transform(({ value }: { value: any }) => createWidgetLayout(value))
	layout!: LayoutGroup[];
}

export class UpdatePropertyPanelSectionsLayout extends JsMessage implements WidgetLayout {
	layoutTarget!: unknown;

	// TODO: Replace `any` with correct typing
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	@Transform(({ value }: { value: any }) => createWidgetLayout(value))
	layout!: LayoutGroup[];
}

export class UpdateLayerTreeOptionsLayout extends JsMessage implements WidgetLayout {
	layoutTarget!: unknown;

	// TODO: Replace `any` with correct typing
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	@Transform(({ value }: { value: any }) => createWidgetLayout(value))
	layout!: LayoutGroup[];
}

export class UpdateMenuBarLayout extends JsMessage {
	layoutTarget!: unknown;

	// TODO: Replace `any` with correct typing
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	@Transform(({ value }: { value: any }) => createMenuLayout(value))
	layout!: MenuBarEntry[];
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function createMenuLayout(menuBarEntry: any[]): MenuBarEntry[] {
	return menuBarEntry.map((entry) => ({
		...entry,
		children: createMenuLayoutRecursive(entry.children),
	}));
}
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function createMenuLayoutRecursive(children: any[][]): MenuBarEntry[][] {
	return children.map((groups) =>
		groups.map((entry) => ({
			...entry,
			action: hoistWidgetHolders([entry.action])[0],
			children: entry.children ? createMenuLayoutRecursive(entry.children) : undefined,
		}))
	);
}

// `any` is used since the type of the object should be known from the Rust side
// eslint-disable-next-line @typescript-eslint/no-explicit-any
type JSMessageFactory = (data: any, wasm: WasmRawInstance, instance: WasmEditorInstance) => JsMessage;
type MessageMaker = typeof JsMessage | JSMessageFactory;

export const messageMakers: Record<string, MessageMaker> = {
	DisplayDialog,
	DisplayDialogDismiss,
	DisplayDialogPanic,
	DisplayEditableTextbox,
	DisplayRemoveEditableTextbox,
	TriggerAboutGraphiteLocalizedCommitDate,
	TriggerOpenDocument,
	TriggerFileDownload,
	TriggerFontLoad,
	TriggerImport,
	TriggerIndexedDbRemoveDocument,
	TriggerIndexedDbWriteDocument,
	TriggerPaste,
	TriggerRasterDownload,
	TriggerRefreshBoundsOfViewports,
	TriggerTextCommit,
	TriggerTextCopy,
	TriggerViewportResize,
	TriggerVisitLink,
	UpdateActiveDocument,
	UpdateDialogDetails,
	UpdateDocumentArtboards,
	UpdateDocumentArtwork,
	UpdateDocumentBarLayout,
	UpdateDocumentLayerDetails,
	UpdateDocumentLayerTreeStructure: newUpdateDocumentLayerTreeStructure,
	UpdateDocumentModeLayout,
	UpdateDocumentOverlays,
	UpdateDocumentRulers,
	UpdateDocumentScrollbars,
	UpdateImageData,
	UpdateInputHints,
	UpdateLayerTreeOptionsLayout,
	UpdateMenuBarLayout,
	UpdateMouseCursor,
	UpdateNodeGraphVisibility,
	UpdateOpenDocumentsList,
	UpdatePropertyPanelOptionsLayout,
	UpdatePropertyPanelSectionsLayout,
	UpdateToolOptionsLayout,
	UpdateToolShelfLayout,
	UpdateWorkingColorsLayout,
} as const;
export type JsMessageType = keyof typeof messageMakers;

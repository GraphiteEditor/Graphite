/* eslint-disable @typescript-eslint/no-explicit-any */
/* eslint-disable max-classes-per-file */

import { Transform, Type, plainToClass } from "class-transformer";

import { type PopoverButtonStyle, type IconName, type IconSize } from "@graphite/utility-functions/icons";
import { type WasmEditorInstance, type WasmRawInstance } from "@graphite/wasm-communication/editor";

import type MenuList from "@graphite/components/floating-menus/MenuList.svelte";

export class JsMessage {
	// The marker provides a way to check if an object is a sub-class constructor for a jsMessage.
	static readonly jsMessageMarker = true;
}

const TupleToVec2 = Transform(({ value }: { value: [number, number] | undefined }) => (value === undefined ? undefined : { x: value[0], y: value[1] }));
// const BigIntTupleToVec2 = Transform(({ value }: { value: [bigint, bigint] | undefined }) => (value === undefined ? undefined : { x: Number(value[0]), y: Number(value[1]) }));

export type XY = { x: number; y: number };

// ============================================================================
// Add additional classes below to replicate Rust's `FrontendMessage`s and data structures.
//
// Remember to add each message to the `messageConstructors` export at the bottom of the file.
//
// Read class-transformer docs at https://github.com/typestack/class-transformer#table-of-contents
// for details about how to transform the JSON from wasm-bindgen into classes.
// ============================================================================

export class UpdateNodeGraph extends JsMessage {
	@Type(() => FrontendNode)
	readonly nodes!: FrontendNode[];

	@Type(() => FrontendNodeLink)
	readonly links!: FrontendNodeLink[];
}

export class UpdateNodeTypes extends JsMessage {
	@Type(() => FrontendNode)
	readonly nodeTypes!: FrontendNodeType[];
}

export class UpdateNodeThumbnail extends JsMessage {
	readonly id!: bigint;

	readonly value!: string;
}

export class UpdateNodeGraphSelection extends JsMessage {
	@Type(() => BigInt)
	readonly selected!: bigint[];
}

export class UpdateOpenDocumentsList extends JsMessage {
	@Type(() => FrontendDocumentDetails)
	readonly openDocuments!: FrontendDocumentDetails[];
}

export class UpdateZoomWithScroll extends JsMessage {
	readonly zoomWithScroll!: boolean;
}

// Allows the auto save system to use a string for the id rather than a BigInt.
// IndexedDb does not allow for BigInts as primary keys.
// TypeScript does not allow subclasses to change the type of class variables in subclasses.
// It is an abstract class to point out that it should not be instantiated directly.
export abstract class DocumentDetails {
	readonly name!: string;

	readonly isAutoSaved!: boolean;

	readonly isSaved!: boolean;

	readonly id!: bigint | string;

	get displayName(): string {
		return `${this.name}${this.isSaved ? "" : "*"}`;
	}
}

export class FrontendDocumentDetails extends DocumentDetails {
	readonly id!: bigint;
}

export type FrontendGraphDataType = "general" | "number" | "raster" | "vector" | "color" | "artboard";

export class FrontendGraphInput {
	readonly dataType!: FrontendGraphDataType;

	readonly name!: string;

	readonly resolvedType!: string | undefined;

	readonly connected!: bigint | undefined;
}

export class FrontendGraphOutput {
	readonly dataType!: FrontendGraphDataType;

	readonly name!: string;

	readonly resolvedType!: string | undefined;

	readonly connected!: bigint | undefined;
}

export class FrontendNode {
	readonly isLayer!: boolean;

	readonly id!: bigint;

	readonly alias!: string;

	readonly name!: string;

	readonly primaryInput!: FrontendGraphInput | undefined;

	readonly exposedInputs!: FrontendGraphInput[];

	readonly primaryOutput!: FrontendGraphOutput | undefined;

	readonly exposedOutputs!: FrontendGraphOutput[];

	@TupleToVec2
	readonly position!: XY | undefined;

	readonly previewed!: boolean;

	readonly disabled!: boolean;

	readonly errors!: string | undefined;
}

export class FrontendNodeLink {
	readonly linkStart!: bigint;

	readonly linkStartOutputIndex!: bigint;

	readonly linkEnd!: bigint;

	readonly linkEndInputIndex!: bigint;
}

export class FrontendNodeType {
	readonly name!: string;

	readonly category!: string;
}

export class IndexedDbDocumentDetails extends DocumentDetails {
	@Transform(({ value }: { value: bigint }) => value.toString())
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
	readonly keyGroups!: LayoutKeysGroup[];

	readonly keyGroupsMac!: LayoutKeysGroup[] | undefined;

	readonly mouse!: MouseMotion | undefined;

	readonly label!: string;

	readonly plus!: boolean;

	readonly slash!: boolean;
}

// Rust enum `Key`
export type KeyRaw = string;
// Serde converts a Rust `Key` enum variant into this format (via a custom serializer) with both the `Key` variant name (called `RawKey` in TS) and the localized `label` for the key
export type Key = { key: KeyRaw; label: string };
export type LayoutKeysGroup = Key[];
export type ActionKeys = { keys: LayoutKeysGroup };

export type MouseMotion = string;

// Channels can have any range (0-1, 0-255, 0-100, 0-360) in the context they are being used in, these are just containers for the numbers
export type HSVA = { h: number; s: number; v: number; a: number };
export type HSV = { h: number; s: number; v: number };
export type RGBA = { r: number; g: number; b: number; a: number };
export type RGB = { r: number; g: number; b: number };

// All channels range from 0 to 1
export class Color {
	constructor();

	constructor(none: "none");

	constructor(hsva: HSVA);

	constructor(red: number, green: number, blue: number, alpha: number);

	constructor(firstArg?: "none" | HSVA | number, green?: number, blue?: number, alpha?: number) {
		// Empty constructor
		if (firstArg === undefined) {
			this.red = 0;
			this.green = 0;
			this.blue = 0;
			this.alpha = 1;
			this.none = false;
		} else if (firstArg === "none") {
			this.red = 0;
			this.green = 0;
			this.blue = 0;
			this.alpha = 1;
			this.none = true;
		}
		// HSVA constructor
		else if (typeof firstArg === "object" && green === undefined && blue === undefined && alpha === undefined) {
			const { h, s, v } = firstArg;
			const convert = (n: number): number => {
				const k = (n + h * 6) % 6;
				return v - v * s * Math.max(Math.min(...[k, 4 - k, 1]), 0);
			};

			this.red = convert(5);
			this.green = convert(3);
			this.blue = convert(1);
			this.alpha = firstArg.a;
			this.none = false;
		}
		// RGBA constructor
		else if (typeof firstArg === "number" && typeof green === "number" && typeof blue === "number" && typeof alpha === "number") {
			this.red = firstArg;
			this.green = green;
			this.blue = blue;
			this.alpha = alpha;
			this.none = false;
		}
	}

	readonly red!: number;

	readonly green!: number;

	readonly blue!: number;

	readonly alpha!: number;

	readonly none!: boolean;

	static fromCSS(colorCode: string): Color | undefined {
		// Allow single-digit hex value inputs
		let colorValue = colorCode.trim();
		if (colorValue.length === 2 && colorValue.charAt(0) === "#" && /[0-9a-f]/i.test(colorValue.charAt(1))) {
			const digit = colorValue.charAt(1);
			colorValue = `#${digit}${digit}${digit}`;
		}

		const canvas = document.createElement("canvas");
		canvas.width = 1;
		canvas.height = 1;
		const context = canvas.getContext("2d");
		if (!context) return undefined;

		context.clearRect(0, 0, 1, 1);

		context.fillStyle = "black";
		context.fillStyle = colorValue;
		const comparisonA = context.fillStyle;

		context.fillStyle = "white";
		context.fillStyle = colorValue;
		const comparisonB = context.fillStyle;

		// Invalid color
		if (comparisonA !== comparisonB) {
			// If this color code didn't start with a #, add it and try again
			if (colorValue.trim().charAt(0) !== "#") return Color.fromCSS(`#${colorValue.trim()}`);
			return undefined;
		}

		context.fillRect(0, 0, 1, 1);

		const [r, g, b, a] = [...context.getImageData(0, 0, 1, 1).data];
		return new Color(r / 255, g / 255, b / 255, a / 255);
	}

	toHexNoAlpha(): string | undefined {
		if (this.none) return undefined;

		const r = Math.round(this.red * 255)
			.toString(16)
			.padStart(2, "0");
		const g = Math.round(this.green * 255)
			.toString(16)
			.padStart(2, "0");
		const b = Math.round(this.blue * 255)
			.toString(16)
			.padStart(2, "0");

		return `#${r}${g}${b}`;
	}

	toHexOptionalAlpha(): string | undefined {
		if (this.none) return undefined;

		const hex = this.toHexNoAlpha();
		const a = Math.round(this.alpha * 255)
			.toString(16)
			.padStart(2, "0");

		return a === "ff" ? hex : `${hex}${a}`;
	}

	toRgb255(): RGB | undefined {
		if (this.none) return undefined;

		return {
			r: Math.round(this.red * 255),
			g: Math.round(this.green * 255),
			b: Math.round(this.blue * 255),
		};
	}

	toRgba255(): RGBA | undefined {
		if (this.none) return undefined;

		return {
			r: Math.round(this.red * 255),
			g: Math.round(this.green * 255),
			b: Math.round(this.blue * 255),
			a: Math.round(this.alpha * 255),
		};
	}

	toRgbCSS(): string | undefined {
		const rgba = this.toRgba255();
		if (!rgba) return undefined;

		return `rgb(${rgba.r}, ${rgba.g}, ${rgba.b})`;
	}

	toRgbaCSS(): string | undefined {
		const rgba = this.toRgba255();
		if (!rgba) return undefined;

		return `rgba(${rgba.r}, ${rgba.g}, ${rgba.b}, ${rgba.a})`;
	}

	toHSV(): HSV | undefined {
		const hsva = this.toHSVA();
		if (!hsva) return undefined;

		return { h: hsva.h, s: hsva.s, v: hsva.v };
	}

	toHSVA(): HSVA | undefined {
		if (this.none) return undefined;

		const { red: r, green: g, blue: b, alpha: a } = this;

		const max = Math.max(r, g, b);
		const min = Math.min(r, g, b);

		const d = max - min;
		const s = max === 0 ? 0 : d / max;
		const v = max;

		let h = 0;
		if (max !== min) {
			switch (max) {
				case r:
					h = (g - b) / d + (g < b ? 6 : 0);
					break;
				case g:
					h = (b - r) / d + 2;
					break;
				case b:
					h = (r - g) / d + 4;
					break;
				default:
			}
			h /= 6;
		}

		return { h, s, v, a };
	}

	toHsvDegreesAndPercent(): HSV | undefined {
		const hsva = this.toHSVA();
		if (!hsva) return undefined;

		return { h: hsva.h * 360, s: hsva.s * 100, v: hsva.v * 100 };
	}

	toHsvaDegreesAndPercent(): HSVA | undefined {
		const hsva = this.toHSVA();
		if (!hsva) return undefined;

		return { h: hsva.h * 360, s: hsva.s * 100, v: hsva.v * 100, a: hsva.a * 100 };
	}

	opaque(): Color | undefined {
		if (this.none) return undefined;

		return new Color(this.red, this.green, this.blue, 1);
	}

	contrastingColor(): "black" | "white" {
		if (this.none) return "black";

		// Convert alpha into white
		const r = this.red * this.alpha + (1 - this.alpha);
		const g = this.green * this.alpha + (1 - this.alpha);
		const b = this.blue * this.alpha + (1 - this.alpha);

		// https://stackoverflow.com/a/3943023/775283

		const linearR = r <= 0.04045 ? r / 12.92 : ((r + 0.055) / 1.055) ** 2.4;
		const linearG = g <= 0.04045 ? g / 12.92 : ((g + 0.055) / 1.055) ** 2.4;
		const linearB = b <= 0.04045 ? b / 12.92 : ((b + 0.055) / 1.055) ** 2.4;

		const linear = linearR * 0.2126 + linearG * 0.7152 + linearB * 0.0722;

		return linear > Math.sqrt(1.05 * 0.05) - 0.05 ? "black" : "white";
	}
}

export class UpdateActiveDocument extends JsMessage {
	readonly documentId!: bigint;
}

export class DisplayDialogPanic extends JsMessage {
	readonly panicInfo!: string;
}

export class DisplayDialog extends JsMessage {
	readonly title!: string;
	readonly icon!: IconName;
}

export class UpdateDocumentArtwork extends JsMessage {
	readonly svg!: string;
}

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

	readonly visible!: boolean;
}

export class UpdateEyedropperSamplingState extends JsMessage {
	@TupleToVec2
	readonly mousePosition!: XY | undefined;

	readonly primaryColor!: string;

	readonly secondaryColor!: string;

	readonly setColorChoice!: "Primary" | "Secondary" | undefined;
}

const mouseCursorIconCSSNames = {
	Default: "default",
	None: "none",
	ZoomIn: "zoom-in",
	ZoomOut: "zoom-out",
	Grabbing: "grabbing",
	Crosshair: "crosshair",
	Text: "text",
	Move: "move",
	NSResize: "ns-resize",
	EWResize: "ew-resize",
	NESWResize: "nesw-resize",
	NWSEResize: "nwse-resize",
	Rotate: "custom-rotate",
} as const;
export type MouseCursor = keyof typeof mouseCursorIconCSSNames;
export type MouseCursorIcon = (typeof mouseCursorIconCSSNames)[MouseCursor];

export class UpdateMouseCursor extends JsMessage {
	@Transform(({ value }: { value: MouseCursor }) => mouseCursorIconCSSNames[value] || "alias")
	readonly cursor!: MouseCursorIcon;
}

export class TriggerLoadAutoSaveDocuments extends JsMessage {}

export class TriggerLoadPreferences extends JsMessage {}

export class TriggerFetchAndOpenDocument extends JsMessage {
	readonly name!: string;

	readonly filename!: string;
}

export class TriggerOpenDocument extends JsMessage {}

export class TriggerImport extends JsMessage {}

export class TriggerPaste extends JsMessage {}

export class TriggerCopyToClipboardBlobUrl extends JsMessage {
	readonly blobUrl!: string;
}

export class TriggerDownloadBlobUrl extends JsMessage {
	readonly layerName!: string;

	readonly blobUrl!: string;
}

export class TriggerDownloadImage extends JsMessage {
	readonly svg!: string;

	readonly name!: string;

	readonly mime!: string;

	@TupleToVec2
	readonly size!: XY;
}

export class TriggerDownloadTextFile extends JsMessage {
	readonly document!: string;

	readonly name!: string;
}

export class TriggerRefreshBoundsOfViewports extends JsMessage {}

export class TriggerRevokeBlobUrl extends JsMessage {
	readonly url!: string;
}

export class TriggerSavePreferences extends JsMessage {
	readonly preferences!: Record<string, unknown>;
}

export class DocumentChanged extends JsMessage {}

export type DataBuffer = {
	pointer: bigint;
	length: bigint;
};

export class UpdateDocumentLayerStructureJs extends JsMessage {
	readonly dataBuffer!: DataBuffer;
}

export class DisplayEditableTextbox extends JsMessage {
	readonly text!: string;

	readonly lineWidth!: undefined | number;

	readonly fontSize!: number;

	@Type(() => Color)
	readonly color!: Color;

	readonly url!: string;

	readonly transform!: number[];
}

export class DisplayEditableTextboxTransform extends JsMessage {
	readonly transform!: number[];
}

export class DisplayRemoveEditableTextbox extends JsMessage {}

export class UpdateDocumentLayerDetails extends JsMessage {
	@Type(() => LayerPanelEntry)
	readonly data!: LayerPanelEntry;
}

export class LayerPanelEntry {
	name!: string;

	@Transform(({ value }: { value: string }) => value || undefined)
	tooltip!: string | undefined;

	layerClassification!: LayerClassification;

	expanded!: boolean;

	disabled!: boolean;

	parentId!: bigint | undefined;

	id!: bigint;

	@Transform(({ value }: { value: bigint }) => Number(value))
	depth!: number;
}

export type LayerClassification = "Folder" | "Artboard" | "Layer";

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

export class TriggerGraphViewOverlay extends JsMessage {
	open!: boolean;
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
	kind!: WidgetPropsNames;
}

export class CheckboxInput extends WidgetProps {
	checked!: boolean;

	disabled!: boolean;

	icon!: IconName;

	@Transform(({ value }: { value: string }) => value || undefined)
	tooltip!: string | undefined;
}

export class ColorButton extends WidgetProps {
	@Transform(({ value }: { value: { red: number; green: number; blue: number; alpha: number } | undefined }) =>
		value === undefined ? new Color("none") : new Color(value.red, value.green, value.blue, value.alpha),
	)
	value!: Color;

	disabled!: boolean;

	allowNone!: boolean;

	// allowTransparency!: boolean; // TODO: Implement

	@Transform(({ value }: { value: string }) => value || undefined)
	tooltip!: string | undefined;
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
	disabled?: boolean;
};

// An entry in the all-encompassing MenuList component which defines all types of menus (which are spawned by widgets like `TextButton` and `DropdownInput`)
export type MenuListEntry = MenuEntryCommon & {
	action?: () => void;
	children?: MenuListEntry[][];

	shortcutRequiresLock?: boolean;
	value?: string;
	disabled?: boolean;
	tooltip?: string;
	font?: URL;
	ref?: MenuList;
};

export class CurveManipulatorGroup {
	anchor!: [number, number];
	handles!: [[number, number], [number, number]];
}

export class Curve {
	manipulatorGroups!: CurveManipulatorGroup[];
	firstHandle!: [number, number];
	lastHandle!: [number, number];
}

export class CurveInput extends WidgetProps {
	value!: Curve;

	disabled!: boolean;

	@Transform(({ value }: { value: string }) => value || undefined)
	tooltip!: string | undefined;
}

export class DropdownInput extends WidgetProps {
	entries!: MenuListEntry[][];

	selectedIndex!: number | undefined;

	drawIcon!: boolean;

	interactive!: boolean;

	disabled!: boolean;

	@Transform(({ value }: { value: string }) => value || undefined)
	tooltip!: string | undefined;
}

export class FontInput extends WidgetProps {
	fontFamily!: string;

	fontStyle!: string;

	isStyle!: boolean;

	disabled!: boolean;

	@Transform(({ value }: { value: string }) => value || undefined)
	tooltip!: string | undefined;
}

export class IconButton extends WidgetProps {
	icon!: IconName;

	size!: IconSize;

	disabled!: boolean;

	active!: boolean;

	@Transform(({ value }: { value: string }) => value || undefined)
	tooltip!: string | undefined;
}

export class IconLabel extends WidgetProps {
	icon!: IconName;

	disabled!: boolean;

	@Transform(({ value }: { value: string }) => value || undefined)
	tooltip!: string | undefined;
}

export class ImageLabel extends WidgetProps {
	image!: IconName;

	@Transform(({ value }: { value: string }) => value || undefined)
	width!: string | undefined;

	@Transform(({ value }: { value: string }) => value || undefined)
	height!: string | undefined;

	@Transform(({ value }: { value: string }) => value || undefined)
	tooltip!: string | undefined;
}

export type NumberInputIncrementBehavior = "Add" | "Multiply" | "Callback" | "None";
export type NumberInputMode = "Increment" | "Range";

export class NumberInput extends WidgetProps {
	// Label

	label!: string | undefined;

	@Transform(({ value }: { value: string }) => value || undefined)
	tooltip!: string | undefined;

	// Disabled

	disabled!: boolean;

	// Value

	value!: number | undefined;

	min!: number | undefined;

	max!: number | undefined;

	isInteger!: boolean;

	// Number presentation

	displayDecimalPlaces!: number;

	unit!: string;

	unitIsHiddenWhenEditing!: boolean;

	// Mode behavior

	mode!: NumberInputMode;

	incrementBehavior!: NumberInputIncrementBehavior;

	step!: number;

	rangeMin!: number | undefined;

	rangeMax!: number | undefined;

	// Styling

	minWidth!: number;
}

export class PopoverButton extends WidgetProps {
	style!: PopoverButtonStyle | undefined;

	icon!: IconName | undefined;

	disabled!: boolean;

	// Body
	header!: string;

	text!: string;

	@Transform(({ value }: { value: string }) => value || undefined)
	tooltip!: string | undefined;

	popoverMinWidth: number | undefined;

	optionsWidget: LayoutGroup[] | undefined;
}

export type RadioEntryData = {
	value?: string;
	label?: string;
	icon?: IconName;
	tooltip?: string;

	// Callbacks
	action?: () => void;
};
export type RadioEntries = RadioEntryData[];

export class RadioInput extends WidgetProps {
	entries!: RadioEntries;

	disabled!: boolean;

	selectedIndex!: number | undefined;

	minWidth!: number;
}

export type SeparatorDirection = "Horizontal" | "Vertical";
export type SeparatorType = "Related" | "Unrelated" | "Section";

export class Separator extends WidgetProps {
	direction!: SeparatorDirection;

	type!: SeparatorType;
}

export class WorkingColorsInput extends WidgetProps {
	@Type(() => Color)
	primary!: Color;

	@Type(() => Color)
	secondary!: Color;
}

export class TextAreaInput extends WidgetProps {
	value!: string;

	label!: string | undefined;

	disabled!: boolean;

	@Transform(({ value }: { value: string }) => value || undefined)
	tooltip!: string | undefined;
}

export class ParameterExposeButton extends WidgetProps {
	exposed!: boolean;

	dataType!: string;

	@Transform(({ value }: { value: string }) => value || undefined)
	tooltip!: string | undefined;
}

export class TextButton extends WidgetProps {
	label!: string;

	icon!: IconName | undefined;

	emphasized!: boolean;

	flush!: boolean;

	minWidth!: number;

	disabled!: boolean;

	@Transform(({ value }: { value: string }) => value || undefined)
	tooltip!: string | undefined;

	menuListChildren!: MenuListEntry[][];
}

export type TextButtonWidget = {
	tooltip?: string;
	message?: string | object;
	callback?: () => void;
	props: {
		kind: "TextButton";
		label: string;
		icon?: IconName;
		emphasized?: boolean;
		flush?: boolean;
		minWidth?: number;
		disabled?: boolean;
		tooltip?: string;

		// Callbacks
		// `action` is used via `IconButtonWidget.callback`
	};
};

export class BreadcrumbTrailButtons extends WidgetProps {
	labels!: string[];

	disabled!: boolean;

	@Transform(({ value }: { value: string }) => value || undefined)
	tooltip!: string | undefined;
}

export class TextInput extends WidgetProps {
	value!: string;

	label!: string | undefined;

	disabled!: boolean;

	minWidth!: number;

	@Transform(({ value }: { value: string }) => value || undefined)
	tooltip!: string | undefined;
}

export class TextLabel extends WidgetProps {
	// Body
	value!: string;

	// Props
	disabled!: boolean;

	bold!: boolean;

	italic!: boolean;

	tableAlign!: boolean;

	minWidth!: number;

	multiline!: boolean;

	@Transform(({ value }: { value: string }) => value || undefined)
	tooltip!: string | undefined;
}

export type PivotPosition = "None" | "TopLeft" | "TopCenter" | "TopRight" | "CenterLeft" | "Center" | "CenterRight" | "BottomLeft" | "BottomCenter" | "BottomRight";

export class PivotInput extends WidgetProps {
	position!: PivotPosition;

	disabled!: boolean;
}

// WIDGET

const widgetSubTypes = [
	{ value: BreadcrumbTrailButtons, name: "BreadcrumbTrailButtons" },
	{ value: CheckboxInput, name: "CheckboxInput" },
	{ value: ColorButton, name: "ColorButton" },
	{ value: CurveInput, name: "CurveInput" },
	{ value: DropdownInput, name: "DropdownInput" },
	{ value: FontInput, name: "FontInput" },
	{ value: IconButton, name: "IconButton" },
	{ value: IconLabel, name: "IconLabel" },
	{ value: ImageLabel, name: "ImageLabel" },
	{ value: NumberInput, name: "NumberInput" },
	{ value: ParameterExposeButton, name: "ParameterExposeButton" },
	{ value: PivotInput, name: "PivotInput" },
	{ value: PopoverButton, name: "PopoverButton" },
	{ value: RadioInput, name: "RadioInput" },
	{ value: Separator, name: "Separator" },
	{ value: WorkingColorsInput, name: "WorkingColorsInput" },
	{ value: TextAreaInput, name: "TextAreaInput" },
	{ value: TextButton, name: "TextButton" },
	{ value: TextInput, name: "TextInput" },
	{ value: TextLabel, name: "TextLabel" },
] as const;

type WidgetSubTypes = (typeof widgetSubTypes)[number];
type WidgetKindMap = { [T in WidgetSubTypes as T["name"]]: InstanceType<T["value"]> };
export type WidgetPropsNames = keyof WidgetKindMap;
export type WidgetPropsSet = WidgetKindMap[WidgetPropsNames];

export function narrowWidgetProps<K extends WidgetPropsNames>(props: WidgetPropsSet, kind: K) {
	if (props.kind === kind) return props as WidgetKindMap[K];
	else return undefined;
}

export class Widget {
	constructor(props: WidgetPropsSet, widgetId: bigint) {
		this.props = props;
		this.widgetId = widgetId;
	}

	@Type(() => WidgetProps, { discriminator: { property: "kind", subTypes: [...widgetSubTypes] }, keepDiscriminatorProperty: true })
	props!: WidgetPropsSet;

	widgetId!: bigint;
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function hoistWidgetHolder(widgetHolder: any): Widget {
	const kind = Object.keys(widgetHolder.widget)[0];
	const props = widgetHolder.widget[kind];
	props.kind = kind;

	if (kind === "PopoverButton") {
		props.optionsWidget = props.optionsWidget.map(createLayoutGroup);
	}

	const { widgetId } = widgetHolder;

	return plainToClass(Widget, { props, widgetId });
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function hoistWidgetHolders(widgetHolders: any[]): Widget[] {
	return widgetHolders.map(hoistWidgetHolder);
}

// WIDGET LAYOUT

export type WidgetLayout = {
	layoutTarget: unknown;
	layout: LayoutGroup[];
};

export class WidgetDiffUpdate extends JsMessage {
	layoutTarget!: unknown;

	// TODO: Replace `any` with correct typing
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	@Transform(({ value }: { value: any }) => createWidgetDiff(value))
	diff!: WidgetDiff[];
}

type UIItem = LayoutGroup[] | LayoutGroup | Widget | MenuBarEntry[] | MenuBarEntry;
type WidgetDiff = { widgetPath: number[]; newValue: UIItem };

export function defaultWidgetLayout(): WidgetLayout {
	return {
		layoutTarget: undefined,
		layout: [],
	};
}

// Updates a widget layout based on a list of updates, giving the new layout by mutating the `layout` argument
export function patchWidgetLayout(layout: /* &mut */ WidgetLayout, updates: WidgetDiffUpdate) {
	layout.layoutTarget = updates.layoutTarget;

	updates.diff.forEach((update) => {
		// Find the object where the diff applies to
		const diffObject = update.widgetPath.reduce((targetLayout, index) => {
			if ("columnWidgets" in targetLayout) return targetLayout.columnWidgets[index];
			if ("rowWidgets" in targetLayout) return targetLayout.rowWidgets[index];
			if ("layout" in targetLayout) return targetLayout.layout[index];
			if (targetLayout instanceof Widget) {
				if (targetLayout.props.kind === "PopoverButton" && targetLayout.props instanceof PopoverButton && targetLayout.props.optionsWidget) {
					return targetLayout.props.optionsWidget[index];
				}
				// eslint-disable-next-line no-console
				console.error("Tried to index widget");
				return targetLayout;
			}
			// This is a path traversal so we can assume from the backend that it exists
			// eslint-disable-next-line @typescript-eslint/no-non-null-assertion
			if ("action" in targetLayout) return targetLayout.children![index];
			return targetLayout[index];
		}, layout.layout as UIItem);

		// If this is a list with a length, then set the length to 0 to clear the list
		if ("length" in diffObject) {
			diffObject.length = 0;
		}
		// Remove all of the keys from the old object
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		Object.keys(diffObject).forEach((key) => delete (diffObject as any)[key]);

		// Assign keys to the new object
		// `Object.assign` works but `diffObject = update.newValue;` doesn't.
		// https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Object/assign
		Object.assign(diffObject, update.newValue);
	});
}

export type LayoutGroup = WidgetSpanRow | WidgetSpanColumn | WidgetSection;

export type WidgetSpanColumn = { columnWidgets: Widget[] };
export function isWidgetSpanColumn(layoutColumn: LayoutGroup): layoutColumn is WidgetSpanColumn {
	return Boolean((layoutColumn as WidgetSpanColumn)?.columnWidgets);
}

export type WidgetSpanRow = { rowWidgets: Widget[] };
export function isWidgetSpanRow(layoutRow: LayoutGroup): layoutRow is WidgetSpanRow {
	return Boolean((layoutRow as WidgetSpanRow)?.rowWidgets);
}

export type WidgetSection = { name: string; layout: LayoutGroup[] };
export function isWidgetSection(layoutRow: LayoutGroup): layoutRow is WidgetSection {
	return Boolean((layoutRow as WidgetSection)?.layout);
}

// Unpacking rust types to more usable type in the frontend
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function createWidgetDiff(diffs: any[]): WidgetDiff[] {
	return diffs.map((diff) => {
		const { widgetPath, newValue } = diff;
		if (newValue.subLayout) {
			return { widgetPath, newValue: newValue.subLayout.map(createLayoutGroup) };
		}
		if (newValue.layoutGroup) {
			return { widgetPath, newValue: createLayoutGroup(newValue.layoutGroup) };
		}
		if (newValue.widget) {
			return { widgetPath, newValue: hoistWidgetHolder(newValue.widget) };
		}
		// This code should be unreachable
		throw new Error("DiffUpdate invalid");
	});
}

// Unpacking a layout group
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function createLayoutGroup(layoutGroup: any): LayoutGroup {
	if (layoutGroup.column) {
		const columnWidgets = hoistWidgetHolders(layoutGroup.column.columnWidgets);

		const result: WidgetSpanColumn = { columnWidgets };
		return result;
	}

	if (layoutGroup.row) {
		const result: WidgetSpanRow = { rowWidgets: hoistWidgetHolders(layoutGroup.row.rowWidgets) };
		return result;
	}

	if (layoutGroup.section) {
		const result: WidgetSection = { name: layoutGroup.section.name, layout: layoutGroup.section.layout.map(createLayoutGroup) };
		return result;
	}

	throw new Error("Layout row type does not exist");
}

// WIDGET LAYOUTS
export class UpdateDialogButtons extends WidgetDiffUpdate {}

export class UpdateDialogColumn1 extends WidgetDiffUpdate {}

export class UpdateDialogColumn2 extends WidgetDiffUpdate {}

export class UpdateDocumentBarLayout extends WidgetDiffUpdate {}

export class UpdateDocumentModeLayout extends WidgetDiffUpdate {}

export class UpdateLayersPanelOptionsLayout extends WidgetDiffUpdate {}

// Extends JsMessage instead of WidgetDiffUpdate because the menu bar isn't diffed
export class UpdateMenuBarLayout extends JsMessage {
	layoutTarget!: unknown;

	// TODO: Replace `any` with correct typing
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	@Transform(({ value }: { value: any }) => createMenuLayout(value))
	layout!: MenuBarEntry[];
}

export class UpdateNodeGraphBarLayout extends WidgetDiffUpdate {}

export class UpdatePropertyPanelOptionsLayout extends WidgetDiffUpdate {}

export class UpdatePropertyPanelSectionsLayout extends WidgetDiffUpdate {}

export class UpdateToolOptionsLayout extends WidgetDiffUpdate {}

export class UpdateToolShelfLayout extends WidgetDiffUpdate {}

export class UpdateWorkingColorsLayout extends WidgetDiffUpdate {}

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
			disabled: entry.disabled ?? false,
		})),
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
	DisplayEditableTextboxTransform,
	DisplayRemoveEditableTextbox,
	TriggerAboutGraphiteLocalizedCommitDate,
	TriggerCopyToClipboardBlobUrl,
	TriggerFetchAndOpenDocument,
	TriggerDownloadBlobUrl,
	TriggerDownloadImage,
	TriggerDownloadTextFile,
	TriggerFontLoad,
	TriggerGraphViewOverlay,
	TriggerImport,
	TriggerIndexedDbRemoveDocument,
	TriggerIndexedDbWriteDocument,
	TriggerLoadAutoSaveDocuments,
	TriggerLoadPreferences,
	TriggerOpenDocument,
	TriggerPaste,
	TriggerRefreshBoundsOfViewports,
	TriggerRevokeBlobUrl,
	TriggerSavePreferences,
	TriggerTextCommit,
	TriggerTextCopy,
	TriggerViewportResize,
	TriggerVisitLink,
	UpdateActiveDocument,
	UpdateDialogButtons,
	UpdateDialogColumn1,
	UpdateDialogColumn2,
	UpdateDocumentArtwork,
	UpdateDocumentBarLayout,
	UpdateDocumentLayerDetails,
	UpdateDocumentLayerStructureJs,
	UpdateDocumentModeLayout,
	UpdateDocumentRulers,
	UpdateDocumentScrollbars,
	UpdateEyedropperSamplingState,
	UpdateInputHints,
	UpdateLayersPanelOptionsLayout,
	UpdateMenuBarLayout,
	UpdateMouseCursor,
	UpdateNodeGraph,
	UpdateNodeGraphBarLayout,
	UpdateNodeGraphSelection,
	UpdateNodeThumbnail,
	UpdateNodeTypes,
	UpdateOpenDocumentsList,
	UpdatePropertyPanelOptionsLayout,
	UpdatePropertyPanelSectionsLayout,
	UpdateToolOptionsLayout,
	UpdateToolShelfLayout,
	UpdateWorkingColorsLayout,
	UpdateZoomWithScroll,
} as const;
export type JsMessageType = keyof typeof messageMakers;

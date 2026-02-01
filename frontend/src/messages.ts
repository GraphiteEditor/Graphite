/* eslint-disable @typescript-eslint/no-explicit-any */

import { Transform, Type, plainToClass } from "class-transformer";

import { type EditorHandle } from "@graphite/../wasm/pkg/graphite_wasm";
import { type PopoverButtonStyle, type IconName, type IconSize } from "@graphite/icons";

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

export class UpdateBox extends JsMessage {
	readonly box!: Box | undefined;
}

export class UpdateClickTargets extends JsMessage {
	readonly clickTargets!: FrontendClickTargets | undefined;
}

export class UpdateImportsExports extends JsMessage {
	readonly imports!: (FrontendGraphOutput | undefined)[];

	readonly exports!: (FrontendGraphInput | undefined)[];

	@TupleToVec2
	readonly importPosition!: XY;

	@TupleToVec2
	readonly exportPosition!: XY;

	readonly addImportExport!: boolean;
}

export class UpdateInSelectedNetwork extends JsMessage {
	readonly inSelectedNetwork!: boolean;
}

export class UpdateImportReorderIndex extends JsMessage {
	readonly importIndex!: number | undefined;
}

export class UpdateExportReorderIndex extends JsMessage {
	readonly exportIndex!: number | undefined;
}

const LayerWidths = Transform(({ obj }) => obj.layerWidths);
const ChainWidths = Transform(({ obj }) => obj.chainWidths);
const HasLeftInputWire = Transform(({ obj }) => obj.hasLeftInputWire);

export class UpdateLayerWidths extends JsMessage {
	@LayerWidths
	readonly layerWidths!: Map<bigint, number>;
	@ChainWidths
	readonly chainWidths!: Map<bigint, number>;
	@HasLeftInputWire
	readonly hasLeftInputWire!: Map<bigint, boolean>;
}

export class UpdateNodeGraphNodes extends JsMessage {
	@Type(() => FrontendNode)
	readonly nodes!: FrontendNode[];
}

export class UpdateNodeGraphErrorDiagnostic extends JsMessage {
	readonly error!: NodeGraphError | undefined;
}

export class NodeGraphError {
	readonly position!: XY;
	readonly error!: string;
}

export class UpdateVisibleNodes extends JsMessage {
	readonly nodes!: bigint[];
}

export class UpdateNodeGraphWires extends JsMessage {
	readonly wires!: WireUpdate[];
}

export class ClearAllNodeGraphWires extends JsMessage {}

export class UpdateNodeGraphTransform extends JsMessage {
	readonly transform!: NodeGraphTransform;
}

export class SendUIMetadata extends JsMessage {
	@Transform(({ obj }) => new Map(obj.nodeDescriptions))
	readonly nodeDescriptions!: Map<string, string>;

	readonly nodeTypes!: FrontendNodeType[];
}

export class SendShortcutFullscreen extends JsMessage {
	@Transform(({ value }: { value: ActionShortcut }) => value || undefined)
	readonly shortcut!: ActionShortcut | undefined;
	@Transform(({ value }: { value: ActionShortcut }) => value || undefined)
	readonly shortcutMac!: ActionShortcut | undefined;
}

export class SendShortcutAltClick extends JsMessage {
	@Transform(({ value }: { value: ActionShortcut }) => value || undefined)
	readonly shortcut!: ActionShortcut | undefined;
}

export class SendShortcutShiftClick extends JsMessage {
	@Transform(({ value }: { value: ActionShortcut }) => value || undefined)
	readonly shortcut!: ActionShortcut | undefined;
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
	@Type(() => OpenDocument)
	readonly openDocuments!: OpenDocument[];
}

export class UpdateWirePathInProgress extends JsMessage {
	readonly wirePath!: WirePath | undefined;
}

export class OpenDocument {
	readonly id!: bigint;

	@Type(() => DocumentDetails)
	readonly details!: DocumentDetails;
}

export class DocumentDetails {
	readonly name!: string;

	readonly isAutoSaved!: boolean;

	readonly isSaved!: boolean;
}

export class Box {
	readonly startX!: number;

	readonly startY!: number;

	readonly endX!: number;

	readonly endY!: number;
}

export type FrontendClickTargets = {
	readonly nodeClickTargets: string[];
	readonly layerClickTargets: string[];
	readonly connectorClickTargets: string[];
	readonly iconClickTargets: string[];
	readonly allNodesBoundingBox: string;
	readonly modifyImportExport: string[];
};

export type ContextMenuInformation = {
	contextMenuCoordinates: XY;
	contextMenuData: { type: "CreateNode"; data: { compatibleType: string | undefined } } | { type: "ModifyNode"; data: { canBeLayer: boolean; currentlyIsNode: boolean; nodeId: bigint } };
};

export class UpdateContextMenuInformation extends JsMessage {
	readonly contextMenuInformation!: ContextMenuInformation | undefined;
}

export type FrontendGraphDataType = "General" | "Number" | "Artboard" | "Graphic" | "Raster" | "Vector" | "Color" | "Invalid";

export class FrontendGraphInput {
	readonly dataType!: FrontendGraphDataType;

	readonly name!: string;

	readonly description!: string;

	readonly resolvedType!: string;

	readonly validTypes!: string[];

	readonly connectedTo!: string;
}

export class FrontendGraphOutput {
	readonly dataType!: FrontendGraphDataType;

	readonly name!: string;

	readonly description!: string;

	readonly resolvedType!: string;

	readonly connectedTo!: string[];
}

export class FrontendNode {
	readonly id!: bigint;

	readonly isLayer!: boolean;

	readonly canBeLayer!: boolean;

	readonly reference!: string | undefined;

	readonly displayName!: string;

	readonly implementationName!: string;

	readonly primaryInput!: FrontendGraphInput | undefined;

	readonly exposedInputs!: FrontendGraphInput[];

	readonly primaryOutput!: FrontendGraphOutput | undefined;

	readonly exposedOutputs!: FrontendGraphOutput[];

	readonly primaryInputConnectedToLayer!: boolean;

	readonly primaryOutputConnectedToLayer!: boolean;

	@TupleToVec2
	readonly position!: XY;

	// TODO: Store field for the width of the left node chain

	readonly previewed!: boolean;

	readonly visible!: boolean;

	readonly locked!: boolean;
}

export class FrontendNodeType {
	readonly identifier!: string;

	readonly name!: string;

	readonly category!: string;

	readonly inputTypes!: string[];
}

export type DefinitionIdentifier = { type: "Network" | "ProtoNode"; data: string };

export class NodeGraphTransform {
	readonly scale!: number;
	readonly x!: number;
	readonly y!: number;
}

export class WirePath {
	readonly pathString!: string;
	readonly dataType!: FrontendGraphDataType;
	readonly thick!: boolean;
	readonly dashed!: boolean;
}

export class WireUpdate {
	readonly id!: bigint;
	readonly inputIndex!: number;
	readonly wirePathUpdate!: WirePath | undefined;
}

export class TriggerPersistenceWriteDocument extends JsMessage {
	// Use a string since IndexedDB can not use BigInts for keys
	@Transform(({ value }: { value: bigint }) => value.toString())
	documentId!: string;

	document!: string;

	@Type(() => DocumentDetails)
	details!: DocumentDetails;

	version!: string;
}

export class TriggerPersistenceRemoveDocument extends JsMessage {
	// Use a string since IndexedDB can not use BigInts for keys
	@Transform(({ value }: { value: bigint }) => value.toString())
	documentId!: string;
}

export type AppWindowPlatform = "Web" | "Windows" | "Mac" | "Linux";

export class UpdatePlatform extends JsMessage {
	@Transform(({ value }: { value: AppWindowPlatform }) => value)
	readonly platform!: AppWindowPlatform;
}

export class UpdateMaximized extends JsMessage {
	readonly maximized!: boolean;
}

export class UpdateFullscreen extends JsMessage {
	readonly fullscreen!: boolean;
}

export class UpdateViewportHolePunch extends JsMessage {
	readonly active!: boolean;
}

export class UpdateViewportPhysicalBounds extends JsMessage {
	readonly x!: number;
	readonly y!: number;
	readonly width!: number;
	readonly height!: number;
}

export class UpdateUIScale extends JsMessage {
	readonly scale!: number;
}

export class WindowPointerLockMove extends JsMessage {
	readonly x!: number;
	readonly y!: number;
}

export class WindowFullscreen extends JsMessage {}

// Rust enum `Key`
export type KeyRaw = string;
// Serde converts a Rust `Key` enum variant into this format with both the `Key` variant name (called `RawKey` in TS) and the localized `label` for the key
export type LabeledKey = { key: KeyRaw; label: string };
export type MouseMotion = "None" | "Lmb" | "Rmb" | "Mmb" | "ScrollUp" | "ScrollDown" | "Drag" | "LmbDouble" | "LmbDrag" | "RmbDrag" | "RmbDouble" | "MmbDrag";
export type LabeledKeyOrMouseMotion = LabeledKey | MouseMotion;
export type LabeledShortcut = LabeledKeyOrMouseMotion[];
export type ActionShortcut = { shortcut: LabeledShortcut };

// Channels can have any range (0-1, 0-255, 0-100, 0-360) in the context they are being used in, these are just containers for the numbers
export type HSVA = { h: number; s: number; v: number; a: number };
export type HSV = { h: number; s: number; v: number };
export type RGBA = { r: number; g: number; b: number; a: number };
export type RGB = { r: number; g: number; b: number };

export class Gradient {
	readonly stops!: { position: number; color: Color }[];

	constructor(stops: { position: number; color: Color }[]) {
		this.stops = stops;
	}

	toLinearGradientCSS(): string {
		if (this.stops.length === 1) {
			return `linear-gradient(to right, ${this.stops[0].color.toHexOptionalAlpha()} 0%, ${this.stops[0].color.toHexOptionalAlpha()} 100%)`;
		}
		const pieces = this.stops.map((stop) => `${stop.color.toHexOptionalAlpha()} ${stop.position * 100}%`);
		return `linear-gradient(to right, ${pieces.join(", ")})`;
	}

	toLinearGradientCSSNoAlpha(): string {
		if (this.stops.length === 1) {
			return `linear-gradient(to right, ${this.stops[0].color.toHexNoAlpha()} 0%, ${this.stops[0].color.toHexNoAlpha()} 100%)`;
		}
		const pieces = this.stops.map((stop) => `${stop.color.toHexNoAlpha()} ${stop.position * 100}%`);
		return `linear-gradient(to right, ${pieces.join(", ")})`;
	}

	firstColor(): Color | undefined {
		return this.stops[0]?.color;
	}

	lastColor(): Color | undefined {
		return this.stops[this.stops.length - 1]?.color;
	}

	atIndex(index: number): { position: number; color: Color } | undefined {
		return this.stops[index];
	}

	colorAtIndex(index: number): Color | undefined {
		return this.stops[index]?.color;
	}

	positionAtIndex(index: number): number | undefined {
		return this.stops[index]?.position;
	}
}

// All channels range are represented by 0-1, sRGB, gamma.
export class Color {
	readonly red!: number;

	readonly green!: number;

	readonly blue!: number;

	readonly alpha!: number;

	readonly none!: boolean;

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
		const context = canvas.getContext("2d", { willReadFrequently: true });
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

	equals(other: Color): boolean {
		if (this.none && other.none) return true;
		return Math.abs(this.red - other.red) < 1e-6 && Math.abs(this.green - other.green) < 1e-6 && Math.abs(this.blue - other.blue) < 1e-6 && Math.abs(this.alpha - other.alpha) < 1e-6;
	}

	lerp(other: Color, t: number): Color {
		return new Color(this.red * (1 - t) + other.red * t, this.green * (1 - t) + other.green * t, this.blue * (1 - t) + other.blue * t, this.alpha * (1 - t) + other.alpha * t);
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

	toRgbCSS(): string | undefined {
		const rgb = this.toRgb255();
		if (!rgb) return undefined;

		return `rgb(${rgb.r}, ${rgb.g}, ${rgb.b})`;
	}

	toRgbaCSS(): string | undefined {
		const rgb = this.toRgb255();
		if (!rgb) return undefined;

		return `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, ${this.alpha})`;
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

	luminance(): number | undefined {
		if (this.none) return undefined;

		// Convert alpha into white
		const r = this.red * this.alpha + (1 - this.alpha);
		const g = this.green * this.alpha + (1 - this.alpha);
		const b = this.blue * this.alpha + (1 - this.alpha);

		// https://stackoverflow.com/a/3943023/775283

		const linearR = r <= 0.04045 ? r / 12.92 : ((r + 0.055) / 1.055) ** 2.4;
		const linearG = g <= 0.04045 ? g / 12.92 : ((g + 0.055) / 1.055) ** 2.4;
		const linearB = b <= 0.04045 ? b / 12.92 : ((b + 0.055) / 1.055) ** 2.4;

		return linearR * 0.2126 + linearG * 0.7152 + linearB * 0.0722;
	}

	contrastingColor(): "black" | "white" {
		if (this.none) return "black";

		const luminance = this.luminance();

		return luminance && luminance > Math.sqrt(1.05 * 0.05) - 0.05 ? "black" : "white";
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

export class EyedropperPreviewImage {
	readonly data!: Uint8Array;
	readonly width!: number;
	readonly height!: number;
}

export class UpdateEyedropperSamplingState extends JsMessage {
	readonly image!: EyedropperPreviewImage | undefined;

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

export class UpdateGraphViewOverlay extends JsMessage {
	open!: boolean;
}

export class UpdateGraphFadeArtwork extends JsMessage {
	readonly percentage!: number;
}

export class UpdateDataPanelState extends JsMessage {
	readonly open!: boolean;
}

export class UpdatePropertiesPanelState extends JsMessage {
	readonly open!: boolean;
}

export class UpdateLayersPanelState extends JsMessage {
	readonly open!: boolean;
}

export class UpdateMouseCursor extends JsMessage {
	@Transform(({ value }: { value: MouseCursor }) => mouseCursorIconCSSNames[value] || "alias")
	readonly cursor!: MouseCursorIcon;
}

export class TriggerLoadFirstAutoSaveDocument extends JsMessage {}
export class TriggerLoadRestAutoSaveDocuments extends JsMessage {}

export class TriggerOpenLaunchDocuments extends JsMessage {}

export class TriggerLoadPreferences extends JsMessage {}

export class TriggerFetchAndOpenDocument extends JsMessage {
	readonly name!: string;

	readonly filename!: string;
}

export class TriggerOpen extends JsMessage {}

export class TriggerImport extends JsMessage {}

export class TriggerClipboardRead extends JsMessage {}

export class TriggerSaveDocument extends JsMessage {
	readonly documentId!: bigint;

	readonly name!: string;

	readonly path!: string | undefined;

	readonly content!: ArrayBuffer;
}

export class TriggerExportImage extends JsMessage {
	readonly svg!: string;

	readonly name!: string;

	readonly mime!: string;

	@TupleToVec2
	readonly size!: XY;
}

export class TriggerSaveFile extends JsMessage {
	readonly name!: string;

	readonly content!: ArrayBuffer;
}

export class TriggerSavePreferences extends JsMessage {
	readonly preferences!: Record<string, unknown>;
}

export class TriggerSaveActiveDocument extends JsMessage {
	readonly documentId!: bigint;
}

export class DocumentChanged extends JsMessage {}

export type DataBuffer = {
	pointer: bigint;
	length: bigint;
};

export class UpdateDocumentLayerStructureJs extends JsMessage {
	readonly dataBuffer!: DataBuffer;
}

export type TextAlign = "Left" | "Center" | "Right" | "JustifyLeft";

export class DisplayEditableTextbox extends JsMessage {
	readonly text!: string;

	readonly lineHeightRatio!: number;

	readonly fontSize!: number;

	@Type(() => Color)
	readonly color!: Color;

	readonly fontData!: ArrayBuffer;

	readonly transform!: number[];

	readonly maxWidth!: undefined | number;

	readonly maxHeight!: undefined | number;

	readonly align!: TextAlign;
}

export class DisplayEditableTextboxUpdateFontData extends JsMessage {
	readonly fontData!: ArrayBuffer;
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
	id!: bigint;

	implementationName!: string;

	iconName!: IconName | undefined;

	alias!: string;

	inSelectedNetwork!: boolean;

	childrenAllowed!: boolean;

	childrenPresent!: boolean;

	expanded!: boolean;

	@Transform(({ value }: { value: bigint }) => Number(value))
	depth!: number;

	visible!: boolean;

	parentsVisible!: boolean;

	unlocked!: boolean;

	parentsUnlocked!: boolean;

	parentId!: bigint | undefined;

	selected!: boolean;

	ancestorOfSelected!: boolean;

	descendantOfSelected!: boolean;

	clipped!: boolean;

	clippable!: boolean;
}

export class DisplayDialogDismiss extends JsMessage {}

export class Font {
	fontFamily!: string;

	fontStyle!: string;
}

export class TriggerFontCatalogLoad extends JsMessage {}

export class TriggerFontDataLoad extends JsMessage {
	@Type(() => Font)
	font!: Font;

	url!: string;
}

export class TriggerVisitLink extends JsMessage {
	url!: string;
}

export class TriggerTextCommit extends JsMessage {}

export class TriggerClipboardWrite extends JsMessage {
	readonly content!: string;
}

export class TriggerSelectionRead extends JsMessage {
	readonly cut!: boolean;
}

export class TriggerSelectionWrite extends JsMessage {
	readonly content!: string;
}

export class TriggerAboutGraphiteLocalizedCommitDate extends JsMessage {
	readonly commitDate!: string;
}

export class TriggerDisplayThirdPartyLicensesDialog extends JsMessage {}

// WIDGET PROPS

export abstract class WidgetProps {
	kind!: WidgetPropsNames;
}

export class CheckboxInput extends WidgetProps {
	// Content
	checked!: boolean;
	icon!: IconName;
	forLabel!: bigint | undefined;
	disabled!: boolean;

	// Tooltips
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipLabel!: string | undefined;
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipDescription!: string | undefined;
	@Transform(({ value }: { value: ActionShortcut }) => value || undefined)
	tooltipShortcut!: ActionShortcut | undefined;
}

export class ColorInput extends WidgetProps {
	// Content
	@Transform(({ value }) => {
		if (value instanceof Gradient) return value;
		const gradient = value["Gradient"];
		if (gradient) {
			const stops = gradient.map(([position, color]: [number, color: { red: number; green: number; blue: number; alpha: number }]) => ({
				position,
				color: new Color(color.red, color.green, color.blue, color.alpha),
			}));
			return new Gradient(stops);
		}

		if (value instanceof Color) return value;
		const solid = value["Solid"];
		if (solid) {
			return new Color(solid.red, solid.green, solid.blue, solid.alpha);
		}

		return new Color("none");
	})
	value!: FillChoice;
	allowNone!: boolean;
	// allowTransparency!: boolean; // TODO: Implement
	menuDirection!: MenuDirection | undefined;
	disabled!: boolean;

	// Styling
	narrow!: boolean;

	// Tooltips
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipLabel!: string | undefined;
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipDescription!: string | undefined;
	@Transform(({ value }: { value: ActionShortcut }) => value || undefined)
	tooltipShortcut!: ActionShortcut | undefined;
}

export type FillChoice = Color | Gradient;

export function contrastingOutlineFactor(value: FillChoice, proximityColor: string | [string, string], proximityRange: number): number {
	const pair = Array.isArray(proximityColor) ? [proximityColor[0], proximityColor[1]] : [proximityColor, proximityColor];
	const [range1, range2] = pair.map((color) => Color.fromCSS(window.getComputedStyle(document.body).getPropertyValue(color)) || new Color("none"));

	const contrast = (color: Color): number => {
		const colorLuminance = color.luminance() || 0;
		let rangeLuminance1 = range1.luminance() || 0;
		let rangeLuminance2 = range2.luminance() || 0;
		[rangeLuminance1, rangeLuminance2] = [Math.min(rangeLuminance1, rangeLuminance2), Math.max(rangeLuminance1, rangeLuminance2)];

		const distance = (() => {
			if (colorLuminance < rangeLuminance1) return rangeLuminance1 - colorLuminance;
			if (colorLuminance > rangeLuminance2) return colorLuminance - rangeLuminance2;
			return 0;
		})();

		return (1 - Math.min(distance / proximityRange, 1)) * (1 - (color.toHSV()?.s || 0));
	};

	if (value instanceof Gradient) {
		if (value.stops.length === 0) return 0;

		const first = contrast(value.stops[0].color);
		const last = contrast(value.stops[value.stops.length - 1].color);

		return Math.min(first, last);
	}

	return contrast(value);
}

// An entry in the all-encompassing MenuList component which defines all types of menus (which are spawned by widgets like `TextButton` and `DropdownInput`)
export type MenuListEntry = {
	// Content
	value: string;
	label: string;
	icon?: IconName;
	disabled?: boolean;

	// Children
	children?: MenuListEntry[][];
	childrenHash?: bigint;

	// Styling
	font?: string;

	// Tooltips
	tooltipLabel?: string;
	tooltipDescription?: string;
	tooltipShortcut?: ActionShortcut;
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
	// Content
	value!: Curve;

	// Tooltips
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipLabel!: string | undefined;
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipDescription!: string | undefined;
	@Transform(({ value }: { value: ActionShortcut }) => value || undefined)
	tooltipShortcut!: ActionShortcut | undefined;
}

export class DropdownInput extends WidgetProps {
	// Content
	selectedIndex!: number | undefined;
	drawIcon!: boolean;
	disabled!: boolean;

	// Children
	entries!: MenuListEntry[][];
	entriesHash!: bigint;

	// Styling
	narrow!: boolean;

	// Behavior
	virtualScrolling!: boolean;
	interactive!: boolean;

	// Sizing
	minWidth!: number;
	maxWidth!: number;

	// Tooltips
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipLabel!: string | undefined;
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipDescription!: string | undefined;
	@Transform(({ value }: { value: ActionShortcut }) => value || undefined)
	tooltipShortcut!: ActionShortcut | undefined;
}

export class IconButton extends WidgetProps {
	// Content
	icon!: IconName;
	hoverIcon!: IconName | undefined;
	size!: IconSize;
	disabled!: boolean;

	// Styling
	emphasized!: boolean;

	// Tooltips
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipLabel!: string | undefined;
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipDescription!: string | undefined;
	@Transform(({ value }: { value: ActionShortcut }) => value || undefined)
	tooltipShortcut!: ActionShortcut | undefined;
}

export class IconLabel extends WidgetProps {
	// Content
	icon!: IconName;
	disabled!: boolean;

	// Tooltips
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipLabel!: string | undefined;
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipDescription!: string | undefined;
	@Transform(({ value }: { value: ActionShortcut }) => value || undefined)
	tooltipShortcut!: ActionShortcut | undefined;
}

export class ImageButton extends WidgetProps {
	// Content
	image!: IconName;
	@Transform(({ value }: { value: string }) => value || undefined)
	width!: string | undefined;
	@Transform(({ value }: { value: string }) => value || undefined)
	height!: string | undefined;

	// Tooltips
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipLabel!: string | undefined;
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipDescription!: string | undefined;
	@Transform(({ value }: { value: ActionShortcut }) => value || undefined)
	tooltipShortcut!: ActionShortcut | undefined;
}

export class ImageLabel extends WidgetProps {
	// Content
	url!: string;
	@Transform(({ value }: { value: string }) => value || undefined)
	width!: string | undefined;
	@Transform(({ value }: { value: string }) => value || undefined)
	height!: string | undefined;

	// Tooltips
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipLabel!: string | undefined;
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipDescription!: string | undefined;
	@Transform(({ value }: { value: ActionShortcut }) => value || undefined)
	tooltipShortcut!: ActionShortcut | undefined;
}

export class ShortcutLabel extends WidgetProps {
	// Content
	@Transform(({ value }: { value: ActionShortcut }) => value || undefined)
	shortcut!: ActionShortcut | undefined;
}

export type NumberInputIncrementBehavior = "Add" | "Multiply" | "Callback" | "None";
export type NumberInputMode = "Increment" | "Range";

export class NumberInput extends WidgetProps {
	// Content
	value!: number | undefined;
	label!: string | undefined;
	disabled!: boolean;

	// Styling
	narrow!: boolean;

	// Behavior
	mode!: NumberInputMode;
	min!: number | undefined;
	max!: number | undefined;
	rangeMin!: number | undefined;
	rangeMax!: number | undefined;
	step!: number;
	isInteger!: boolean;
	incrementBehavior!: NumberInputIncrementBehavior;
	displayDecimalPlaces!: number;
	unit!: string;
	unitIsHiddenWhenEditing!: boolean;

	// Sizing
	minWidth!: number;
	maxWidth!: number;

	// Tooltips
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipLabel!: string | undefined;
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipDescription!: string | undefined;
	@Transform(({ value }: { value: ActionShortcut }) => value || undefined)
	tooltipShortcut!: ActionShortcut | undefined;
}

export class NodeCatalog extends WidgetProps {
	// Content
	disabled!: boolean;

	// Behavior
	initialSearchTerm!: string;
}

export class PopoverButton extends WidgetProps {
	// Content
	style!: PopoverButtonStyle | undefined;
	icon!: IconName | undefined;
	disabled!: boolean;

	// Children
	popoverLayout!: Layout;
	popoverMinWidth!: number | undefined;
	menuDirection!: MenuDirection | undefined;

	// Tooltips
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipLabel!: string | undefined;
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipDescription!: string | undefined;
	@Transform(({ value }: { value: ActionShortcut }) => value || undefined)
	tooltipShortcut!: ActionShortcut | undefined;
}

export type MenuDirection = "Top" | "Bottom" | "Left" | "Right" | "TopLeft" | "TopRight" | "BottomLeft" | "BottomRight" | "Center";

export type RadioEntryData = {
	// Content
	value?: string;
	label?: string;
	icon?: IconName;

	// Tooltips
	tooltipLabel?: string;
	tooltipDescription?: string;
	tooltipShortcut?: ActionShortcut;
};

export class RadioInput extends WidgetProps {
	// Content
	selectedIndex!: number | undefined;
	disabled!: boolean;

	// Children
	entries!: RadioEntryData[];

	// Styling
	narrow!: boolean;

	// Sizing
	minWidth!: number;
}

export type SeparatorDirection = "Horizontal" | "Vertical";
export type SeparatorStyle = "Related" | "Unrelated" | "Section";

export class Separator extends WidgetProps {
	// Content
	direction!: SeparatorDirection;
	style!: SeparatorStyle;
}

export class WorkingColorsInput extends WidgetProps {
	// Content
	@Type(() => Color)
	primary!: Color;
	@Type(() => Color)
	secondary!: Color;
}

export class TextAreaInput extends WidgetProps {
	// Content
	value!: string;
	label!: string | undefined;
	disabled!: boolean;

	// Tooltips
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipLabel!: string | undefined;
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipDescription!: string | undefined;
	@Transform(({ value }: { value: ActionShortcut }) => value || undefined)
	tooltipShortcut!: ActionShortcut | undefined;
}

export class ParameterExposeButton extends WidgetProps {
	// Content
	exposed!: boolean;
	dataType!: FrontendGraphDataType;

	// Tooltips
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipLabel!: string | undefined;
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipDescription!: string | undefined;
	@Transform(({ value }: { value: ActionShortcut }) => value || undefined)
	tooltipShortcut!: ActionShortcut | undefined;
}

export class TextButton extends WidgetProps {
	// Content
	label!: string;
	icon!: IconName | undefined;
	hoverIcon!: IconName | undefined;
	disabled!: boolean;

	// Children
	menuListChildren!: MenuListEntry[][];
	menuListChildrenHash!: bigint;

	// Styling
	emphasized!: boolean;
	flush!: boolean;
	narrow!: boolean;

	// Sizing
	minWidth!: number;

	// Tooltips
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipLabel!: string | undefined;
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipDescription!: string | undefined;
	@Transform(({ value }: { value: ActionShortcut }) => value || undefined)
	tooltipShortcut!: ActionShortcut | undefined;
}

export class BreadcrumbTrailButtons extends WidgetProps {
	// Content
	labels!: string[];
	disabled!: boolean;

	// Tooltips
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipLabel!: string | undefined;
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipDescription!: string | undefined;
	@Transform(({ value }: { value: ActionShortcut }) => value || undefined)
	tooltipShortcut!: ActionShortcut | undefined;
}

export class TextInput extends WidgetProps {
	// Content
	value!: string;
	label!: string | undefined;
	placeholder!: string | undefined;
	disabled!: boolean;

	// Styling
	narrow!: boolean;
	centered!: boolean;

	// Sizing
	minWidth!: number;
	maxWidth!: number;

	// Tooltips
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipLabel!: string | undefined;
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipDescription!: string | undefined;
	@Transform(({ value }: { value: ActionShortcut }) => value || undefined)
	tooltipShortcut!: ActionShortcut | undefined;
}

export class TextLabel extends WidgetProps {
	// Content
	value!: string;
	disabled!: boolean;
	forCheckbox!: bigint | undefined;

	// Styling
	narrow!: boolean;
	bold!: boolean;
	italic!: boolean;
	monospace!: boolean;
	multiline!: boolean;
	centerAlign!: boolean;
	tableAlign!: boolean;

	// Sizing
	minWidth!: number;
	minWidthCharacters!: number;

	// Tooltips
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipLabel!: string | undefined;
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipDescription!: string | undefined;
	@Transform(({ value }: { value: ActionShortcut }) => value || undefined)
	tooltipShortcut!: ActionShortcut | undefined;
}

export type ReferencePoint = "None" | "TopLeft" | "TopCenter" | "TopRight" | "CenterLeft" | "Center" | "CenterRight" | "BottomLeft" | "BottomCenter" | "BottomRight";

export class ReferencePointInput extends WidgetProps {
	// Content
	value!: ReferencePoint;
	disabled!: boolean;

	// Tooltips
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipLabel!: string | undefined;
	@Transform(({ value }: { value: string }) => value || undefined)
	tooltipDescription!: string | undefined;
	@Transform(({ value }: { value: ActionShortcut }) => value || undefined)
	tooltipShortcut!: ActionShortcut | undefined;
}

// WIDGET

const widgetSubTypes = [
	{ value: BreadcrumbTrailButtons, name: "BreadcrumbTrailButtons" },
	{ value: CheckboxInput, name: "CheckboxInput" },
	{ value: ColorInput, name: "ColorInput" },
	{ value: CurveInput, name: "CurveInput" },
	{ value: DropdownInput, name: "DropdownInput" },
	{ value: IconButton, name: "IconButton" },
	{ value: ImageButton, name: "ImageButton" },
	{ value: ImageLabel, name: "ImageLabel" },
	{ value: ShortcutLabel, name: "ShortcutLabel" },
	{ value: IconLabel, name: "IconLabel" },
	{ value: NodeCatalog, name: "NodeCatalog" },
	{ value: NumberInput, name: "NumberInput" },
	{ value: ParameterExposeButton, name: "ParameterExposeButton" },
	{ value: ReferencePointInput, name: "ReferencePointInput" },
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

export class WidgetInstance {
	@Type(() => WidgetProps, { discriminator: { property: "kind", subTypes: [...widgetSubTypes] }, keepDiscriminatorProperty: true })
	props!: WidgetPropsSet;

	widgetId!: bigint;
}

function hoistWidgetInstance(widgetInstance: any): WidgetInstance {
	const kind = Object.keys(widgetInstance.widget)[0];
	const props = widgetInstance.widget[kind];
	props.kind = kind;

	if (kind === "PopoverButton") {
		props.popoverLayout = props.popoverLayout.map(createLayoutGroup);
	}

	const { widgetId } = widgetInstance;

	return plainToClass(WidgetInstance, { props, widgetId });
}

function hoistWidgetInstances(widgetInstance: any[]): WidgetInstance[] {
	return widgetInstance.map(hoistWidgetInstance);
}

// WIDGET LAYOUT

export type LayoutTarget =
	| "DataPanel"
	| "DialogButtons"
	| "DialogColumn1"
	| "DialogColumn2"
	| "DocumentBar"
	| "LayersPanelBottomBar"
	| "LayersPanelControlLeftBar"
	| "LayersPanelControlRightBar"
	| "MenuBar"
	| "NodeGraphControlBar"
	| "PropertiesPanel"
	| "StatusBarHints"
	| "StatusBarInfo"
	| "ToolOptions"
	| "ToolShelf"
	| "WelcomeScreenButtons"
	| "WorkingColors";

export class WidgetDiffUpdate extends JsMessage {
	// TODO: Replace `any` with correct typing
	@Transform(({ value }: { value: WidgetDiff[] }) => {
		// Unpacking rust types to more usable type in the frontend
		return value.map((diff) => {
			const { widgetPath, newValue } = diff;

			if ("layout" in newValue) return { widgetPath, newValue: newValue.layout.map(createLayoutGroup) };
			if ("layoutGroup" in newValue) return { widgetPath, newValue: createLayoutGroup(newValue.layoutGroup) };
			if ("widget" in newValue) return { widgetPath, newValue: hoistWidgetInstance(newValue.widget) };

			// This code should be unreachable
			throw new Error("DiffUpdate invalid");
		});
	})
	diff!: WidgetDiff[];
}

type DiffUpdate = { layout: Layout } | { layoutGroup: LayoutGroup } | { widget: WidgetInstance };
type WidgetDiff = { widgetPath: number[]; newValue: DiffUpdate };

type UIItem = Layout | LayoutGroup | WidgetInstance[] | WidgetInstance;

// Updates a widget layout based on a list of updates, giving the new layout by mutating the `layout` argument
export function patchLayout(layout: /* &mut */ Layout, updates: WidgetDiffUpdate) {
	updates.diff.forEach((update) => {
		// Find the object where the diff applies to
		const diffObject = update.widgetPath.reduce((targetLayout: UIItem | undefined, index: number): UIItem | undefined => {
			if (targetLayout && "columnWidgets" in targetLayout) return targetLayout.columnWidgets[index];
			if (targetLayout && "rowWidgets" in targetLayout) return targetLayout.rowWidgets[index];
			if (targetLayout && "tableWidgets" in targetLayout) return targetLayout.tableWidgets[index];
			if (targetLayout && "layout" in targetLayout) return targetLayout.layout[index];
			if (targetLayout instanceof WidgetInstance) {
				if (targetLayout.props.kind === "PopoverButton" && targetLayout.props instanceof PopoverButton && targetLayout.props.popoverLayout) {
					return targetLayout.props.popoverLayout[index];
				}
				// eslint-disable-next-line no-console
				console.error("Tried to index widget");
				return targetLayout;
			}

			return targetLayout?.[index];
		}, layout as UIItem);

		// Exit if we failed to produce a valid patch for the existing layout.
		// This means that the backend assumed an existing layout that doesn't exist in the frontend. This can happen, for
		// example, if a panel is destroyed in the frontend but was never cleared in the backend, so the next time the backend
		// tries to update the layout, it attempts to insert only the changes against the old layout that no longer exists.
		if (diffObject === undefined) {
			// eslint-disable-next-line no-console
			console.error("In `patchLayout`, the `diffObject` is undefined. The layout has not been updated. See the source code comment above this error for hints.");
			return;
		}

		// If this is a list with a length, then set the length to 0 to clear the list
		if ("length" in diffObject) {
			diffObject.length = 0;
		}
		// Remove all of the keys from the old object
		Object.keys(diffObject).forEach((key) => delete (diffObject as any)[key]);

		// Assign keys to the new object
		// `Object.assign` works but `diffObject = update.newValue;` doesn't.
		// https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Object/assign
		Object.assign(diffObject, update.newValue);
	});
}

export type LayoutGroup = WidgetSpanRow | WidgetSpanColumn | WidgetTable | WidgetSection;
export type Layout = LayoutGroup[];

export type WidgetSpanColumn = { columnWidgets: WidgetInstance[] };
export function isWidgetSpanColumn(layoutColumn: LayoutGroup): layoutColumn is WidgetSpanColumn {
	return Boolean((layoutColumn as WidgetSpanColumn)?.columnWidgets);
}

export type WidgetSpanRow = { rowWidgets: WidgetInstance[] };
export function isWidgetSpanRow(layoutRow: LayoutGroup): layoutRow is WidgetSpanRow {
	return Boolean((layoutRow as WidgetSpanRow)?.rowWidgets);
}

export type WidgetTable = { tableWidgets: WidgetInstance[][]; unstyled: boolean };
export function isWidgetTable(layoutTable: LayoutGroup): layoutTable is WidgetTable {
	return Boolean((layoutTable as WidgetTable)?.tableWidgets);
}

export type WidgetSection = { name: string; description: string; visible: boolean; pinned: boolean; id: bigint; layout: Layout };
export function isWidgetSection(layoutRow: LayoutGroup): layoutRow is WidgetSection {
	return Boolean((layoutRow as WidgetSection)?.layout);
}

// Unpacking a layout group
function createLayoutGroup(layoutGroup: any): LayoutGroup {
	if (layoutGroup.column) {
		const columnWidgets = hoistWidgetInstances(layoutGroup.column.columnWidgets);

		const result: WidgetSpanColumn = { columnWidgets };
		return result;
	}

	if (layoutGroup.row) {
		const result: WidgetSpanRow = { rowWidgets: hoistWidgetInstances(layoutGroup.row.rowWidgets) };
		return result;
	}

	if (layoutGroup.section) {
		const result: WidgetSection = {
			name: layoutGroup.section.name,
			description: layoutGroup.section.description,
			visible: layoutGroup.section.visible,
			pinned: layoutGroup.section.pinned,
			id: layoutGroup.section.id,
			layout: layoutGroup.section.layout.map(createLayoutGroup),
		};
		return result;
	}

	if (layoutGroup.table) {
		const result: WidgetTable = {
			tableWidgets: layoutGroup.table.tableWidgets.map(hoistWidgetInstances),
			unstyled: layoutGroup.table.unstyled,
		};
		return result;
	}

	throw new Error("Layout row type does not exist");
}

// WIDGET LAYOUTS
export class UpdateDialogButtons extends WidgetDiffUpdate {}

export class UpdateDialogColumn1 extends WidgetDiffUpdate {}

export class UpdateDialogColumn2 extends WidgetDiffUpdate {}

export class UpdateDocumentBarLayout extends WidgetDiffUpdate {}

export class UpdateLayersPanelControlBarLeftLayout extends WidgetDiffUpdate {}

export class UpdateLayersPanelControlBarRightLayout extends WidgetDiffUpdate {}

export class UpdateLayersPanelBottomBarLayout extends WidgetDiffUpdate {}

export class UpdateMenuBarLayout extends WidgetDiffUpdate {}

export class UpdateNodeGraphControlBarLayout extends WidgetDiffUpdate {}

export class UpdateWelcomeScreenButtonsLayout extends WidgetDiffUpdate {}

export class UpdatePropertiesPanelLayout extends WidgetDiffUpdate {}

export class UpdateDataPanelLayout extends WidgetDiffUpdate {}

export class UpdateStatusBarHintsLayout extends WidgetDiffUpdate {}

export class UpdateStatusBarInfoLayout extends WidgetDiffUpdate {}

export class UpdateToolOptionsLayout extends WidgetDiffUpdate {}

export class UpdateToolShelfLayout extends WidgetDiffUpdate {}

export class UpdateWorkingColorsLayout extends WidgetDiffUpdate {}

// `any` is used since the type of the object should be known from the Rust side
type JSMessageFactory = (data: any, wasm: WebAssembly.Memory, handle: EditorHandle) => JsMessage;
type MessageMaker = typeof JsMessage | JSMessageFactory;

export const messageMakers: Record<string, MessageMaker> = {
	ClearAllNodeGraphWires,
	DisplayDialog,
	DisplayDialogDismiss,
	DisplayDialogPanic,
	DisplayEditableTextbox,
	DisplayEditableTextboxTransform,
	DisplayEditableTextboxUpdateFontData,
	DisplayRemoveEditableTextbox,
	SendShortcutAltClick,
	SendShortcutFullscreen,
	SendShortcutShiftClick,
	SendUIMetadata,
	TriggerAboutGraphiteLocalizedCommitDate,
	TriggerClipboardRead,
	TriggerClipboardWrite,
	TriggerDisplayThirdPartyLicensesDialog,
	TriggerExportImage,
	TriggerFetchAndOpenDocument,
	TriggerFontCatalogLoad,
	TriggerFontDataLoad,
	TriggerImport,
	TriggerLoadFirstAutoSaveDocument,
	TriggerLoadPreferences,
	TriggerLoadRestAutoSaveDocuments,
	TriggerOpen,
	TriggerOpenLaunchDocuments,
	TriggerPersistenceRemoveDocument,
	TriggerPersistenceWriteDocument,
	TriggerSaveActiveDocument,
	TriggerSaveDocument,
	TriggerSaveFile,
	TriggerSavePreferences,
	TriggerSelectionRead,
	TriggerSelectionWrite,
	TriggerTextCommit,
	TriggerVisitLink,
	UpdateActiveDocument,
	UpdateBox,
	UpdateClickTargets,
	UpdateContextMenuInformation,
	UpdateDataPanelLayout,
	UpdateDataPanelState,
	UpdateDialogButtons,
	UpdateDialogColumn1,
	UpdateDialogColumn2,
	UpdateDocumentArtwork,
	UpdateDocumentBarLayout,
	UpdateDocumentLayerDetails,
	UpdateDocumentLayerStructureJs,
	UpdateDocumentRulers,
	UpdateDocumentScrollbars,
	UpdateExportReorderIndex,
	UpdateEyedropperSamplingState,
	UpdateFullscreen,
	UpdateGraphFadeArtwork,
	UpdateGraphViewOverlay,
	UpdateImportReorderIndex,
	UpdateImportsExports,
	UpdateInSelectedNetwork,
	UpdateLayersPanelBottomBarLayout,
	UpdateLayersPanelControlBarLeftLayout,
	UpdateLayersPanelControlBarRightLayout,
	UpdateLayersPanelState,
	UpdateLayerWidths,
	UpdateMaximized,
	UpdateMenuBarLayout,
	UpdateMouseCursor,
	UpdateNodeGraphControlBarLayout,
	UpdateNodeGraphErrorDiagnostic,
	UpdateNodeGraphNodes,
	UpdateNodeGraphSelection,
	UpdateNodeGraphTransform,
	UpdateNodeGraphWires,
	UpdateNodeThumbnail,
	UpdateOpenDocumentsList,
	UpdatePlatform,
	UpdatePropertiesPanelLayout,
	UpdatePropertiesPanelState,
	UpdateStatusBarHintsLayout,
	UpdateStatusBarInfoLayout,
	UpdateToolOptionsLayout,
	UpdateToolShelfLayout,
	UpdateUIScale,
	UpdateViewportHolePunch,
	UpdateViewportPhysicalBounds,
	UpdateVisibleNodes,
	UpdateWelcomeScreenButtonsLayout,
	UpdateWirePathInProgress,
	UpdateWorkingColorsLayout,
	WindowFullscreen,
	WindowPointerLockMove,
} as const;
export type JsMessageType = keyof typeof messageMakers;

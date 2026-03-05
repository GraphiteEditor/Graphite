import { sampleInterpolatedGradient } from "@graphite/../wasm/pkg/graphite_wasm";
import { type PopoverButtonStyle, type IconName, type IconSize } from "@graphite/icons";

export type NodeGraphError = {
	readonly position: [number, number];
	readonly error: string;
};

export type OpenDocument = {
	readonly id: bigint;
	readonly details: DocumentDetails;
};

export type DocumentDetails = {
	readonly name: string;
	readonly isAutoSaved: boolean;
	readonly isSaved: boolean;
};

export type Box = {
	readonly startX: number;
	readonly startY: number;
	readonly endX: number;
	readonly endY: number;
};

export type FrontendClickTargets = {
	readonly nodeClickTargets: string[];
	readonly layerClickTargets: string[];
	readonly connectorClickTargets: string[];
	readonly iconClickTargets: string[];
	readonly allNodesBoundingBox: string;
	readonly modifyImportExport: string[];
};

type ContextMenuDataCreateNode = {
	type: "CreateNode";
	data: {
		compatibleType: string | undefined;
	};
};
type ContextMenuDataModifyNode = {
	type: "ModifyNode";
	data: {
		nodeId: bigint;
		canBeLayer: boolean;
		currentlyIsNode: boolean;
		hasSelectedLayers: boolean;
		allSelectedLayersLocked: boolean;
	};
};
export type ContextMenuInformation = {
	contextMenuCoordinates: [number, number];
	contextMenuData: ContextMenuDataCreateNode | ContextMenuDataModifyNode;
};

export type FrontendGraphDataType = "General" | "Number" | "Artboard" | "Graphic" | "Raster" | "Vector" | "Color" | "Invalid";

export type FrontendGraphInput = {
	readonly dataType: FrontendGraphDataType;
	readonly name: string;
	readonly description: string;
	readonly resolvedType: string;
	readonly validTypes: string[];
	readonly connectedTo: string;
};

export type FrontendGraphOutput = {
	readonly dataType: FrontendGraphDataType;
	readonly name: string;
	readonly description: string;
	readonly resolvedType: string;
	readonly connectedTo: string[];
};

export type FrontendNode = {
	readonly id: bigint;
	readonly isLayer: boolean;
	readonly canBeLayer: boolean;
	readonly reference: string | undefined;
	readonly displayName: string;
	readonly implementationName: string;
	readonly primaryInput: FrontendGraphInput | undefined;
	readonly exposedInputs: FrontendGraphInput[];
	readonly primaryOutput: FrontendGraphOutput | undefined;
	readonly exposedOutputs: FrontendGraphOutput[];
	readonly primaryInputConnectedToLayer: boolean;
	readonly primaryOutputConnectedToLayer: boolean;
	readonly position: [number, number];
	// TODO: Store field for the width of the left node chain
	readonly previewed: boolean;
	readonly visible: boolean;
	readonly locked: boolean;
};

export type FrontendNodeType = {
	readonly identifier: string;
	readonly name: string;
	readonly category: string;
	readonly inputTypes: string[];
};

export type DefinitionIdentifier = { type: "Network" | "ProtoNode"; data: string };

export type NodeGraphTransform = {
	readonly scale: number;
	readonly x: number;
	readonly y: number;
};

export type WirePath = {
	readonly pathString: string;
	readonly dataType: FrontendGraphDataType;
	readonly thick: boolean;
	readonly dashed: boolean;
};

export type WireUpdate = {
	readonly id: bigint;
	readonly inputIndex: number;
	readonly wirePathUpdate: WirePath | undefined;
};

export type AppWindowPlatform = "Web" | "Windows" | "Mac" | "Linux";

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
export type RGB = { r: number; g: number; b: number };

export type Gradient = {
	position: number[];
	midpoint: number[];
	color: Color[];
};

// All channels range are represented by 0-1, sRGB, gamma.
export type Color = {
	readonly red: number;
	readonly green: number;
	readonly blue: number;
	readonly alpha: number;
	readonly none: boolean;
};

// COLOR FACTORY FUNCTIONS

export function createColor(red: number, green: number, blue: number, alpha: number): Color {
	return { red, green, blue, alpha, none: false };
}

export function createNoneColor(): Color {
	return { red: 0, green: 0, blue: 0, alpha: 1, none: true };
}

export function createColorFromHSVA(h: number, s: number, v: number, a: number): Color {
	const convert = (n: number): number => {
		const k = (n + h * 6) % 6;
		return v - v * s * Math.max(Math.min(...[k, 4 - k, 1]), 0);
	};

	return { red: convert(5), green: convert(3), blue: convert(1), alpha: a, none: false };
}

// COLOR UTILITY FUNCTIONS

export function colorFromCSS(colorCode: string): Color | undefined {
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
		if (colorValue.trim().charAt(0) !== "#") return colorFromCSS(`#${colorValue.trim()}`);
		return undefined;
	}

	context.fillRect(0, 0, 1, 1);

	const [r, g, b, a] = [...context.getImageData(0, 0, 1, 1).data];
	return createColor(r / 255, g / 255, b / 255, a / 255);
}

export function colorEquals(c1: Color, c2: Color): boolean {
	if (c1.none && c2.none) return true;
	return Math.abs(c1.red - c2.red) < 1e-6 && Math.abs(c1.green - c2.green) < 1e-6 && Math.abs(c1.blue - c2.blue) < 1e-6 && Math.abs(c1.alpha - c2.alpha) < 1e-6;
}

export function colorToHexNoAlpha(color: Color): string | undefined {
	if (color.none) return undefined;

	const r = Math.round(color.red * 255)
		.toString(16)
		.padStart(2, "0");
	const g = Math.round(color.green * 255)
		.toString(16)
		.padStart(2, "0");
	const b = Math.round(color.blue * 255)
		.toString(16)
		.padStart(2, "0");

	return `#${r}${g}${b}`;
}

export function colorToHexOptionalAlpha(color: Color): string | undefined {
	if (color.none) return undefined;

	const hex = colorToHexNoAlpha(color);
	const a = Math.round(color.alpha * 255)
		.toString(16)
		.padStart(2, "0");

	return a === "ff" ? hex : `${hex}${a}`;
}

export function colorToRgb255(color: Color): RGB | undefined {
	if (color.none) return undefined;

	return {
		r: Math.round(color.red * 255),
		g: Math.round(color.green * 255),
		b: Math.round(color.blue * 255),
	};
}

export function colorToRgbCSS(color: Color): string | undefined {
	const rgb = colorToRgb255(color);
	if (!rgb) return undefined;

	return `rgb(${rgb.r}, ${rgb.g}, ${rgb.b})`;
}

export function colorToRgbaCSS(color: Color): string | undefined {
	const rgb = colorToRgb255(color);
	if (!rgb) return undefined;

	return `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, ${color.alpha})`;
}

export function colorToHSVA(color: Color): HSVA | undefined {
	if (color.none) return undefined;

	const { red: r, green: g, blue: b, alpha: a } = color;

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

export function colorOpaque(color: Color): Color | undefined {
	if (color.none) return undefined;

	return createColor(color.red, color.green, color.blue, 1);
}

export function colorLuminance(color: Color): number | undefined {
	if (color.none) return undefined;

	// Convert alpha into white
	const r = color.red * color.alpha + (1 - color.alpha);
	const g = color.green * color.alpha + (1 - color.alpha);
	const b = color.blue * color.alpha + (1 - color.alpha);

	// https://stackoverflow.com/a/3943023/775283

	const linearR = r <= 0.04045 ? r / 12.92 : ((r + 0.055) / 1.055) ** 2.4;
	const linearG = g <= 0.04045 ? g / 12.92 : ((g + 0.055) / 1.055) ** 2.4;
	const linearB = b <= 0.04045 ? b / 12.92 : ((b + 0.055) / 1.055) ** 2.4;

	return linearR * 0.2126 + linearG * 0.7152 + linearB * 0.0722;
}

export function colorContrastingColor(color: Color): "black" | "white" {
	if (color.none) return "black";

	const luminance = colorLuminance(color);

	return luminance && luminance > Math.sqrt(1.05 * 0.05) - 0.05 ? "black" : "white";
}

// GRADIENT UTILITY FUNCTIONS

export function gradientToLinearGradientCSS(gradient: Gradient): string {
	if (gradient.position.length === 1) {
		return `linear-gradient(to right, ${colorToHexOptionalAlpha(gradient.color[0])} 0%, ${colorToHexOptionalAlpha(gradient.color[0])} 100%)`;
	}

	const pieces = sampleInterpolatedGradient(new Float64Array(gradient.position), new Float64Array(gradient.midpoint), gradient.color, false);
	return `linear-gradient(to right, ${pieces})`;
}

export function gradientFirstColor(gradient: Gradient): Color | undefined {
	return gradient.color[0];
}

export function gradientLastColor(gradient: Gradient): Color | undefined {
	return gradient.color[gradient.color.length - 1];
}

// COLOR/GRADIENT TYPE GUARDS

export function isColor(value: unknown): value is Color {
	return typeof value === "object" && value !== null && "red" in value;
}

export function isGradient(value: unknown): value is Gradient {
	return typeof value === "object" && value !== null && "position" in value && "midpoint" in value;
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function parseFillChoice(value: any): FillChoice {
	if (isColor(value)) return value;
	if (isGradient(value)) return value;

	const gradient: Gradient | undefined = value["Gradient"];
	if (gradient) {
		const color = gradient.color.map((c) => createColor(c.red, c.green, c.blue, c.alpha));
		return { ...gradient, color };
	}

	const solid = value["Solid"];
	if (solid) return createColor(solid.red, solid.green, solid.blue, solid.alpha);

	return createNoneColor();
}

export type EyedropperPreviewImage = {
	readonly data: Uint8Array;
	readonly width: number;
	readonly height: number;
};

export const mouseCursorIconCSSNames = {
	Default: "default",
	Alias: "alias",
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

export type LayerStructureEntry = {
	layerId: bigint;
	children: LayerStructureEntry[];
};

export type TextAlign = "Left" | "Center" | "Right" | "JustifyLeft";

export type LayerPanelEntry = {
	id: bigint;
	implementationName: string;
	iconName: IconName | undefined;
	alias: string;
	inSelectedNetwork: boolean;
	childrenAllowed: boolean;
	childrenPresent: boolean;
	expanded: boolean;
	depth: number;
	visible: boolean;
	parentsVisible: boolean;
	unlocked: boolean;
	parentsUnlocked: boolean;
	parentId: bigint | undefined;
	selected: boolean;
	ancestorOfSelected: boolean;
	descendantOfSelected: boolean;
	clipped: boolean;
	clippable: boolean;
};

export type Font = {
	fontFamily: string;
	fontStyle: string;
};

// WIDGET PROPS

export type CheckboxInput = {
	kind: WidgetPropsNames;

	// Content
	checked: boolean;
	icon: IconName;
	forLabel: bigint | undefined;
	disabled: boolean;

	// Tooltips
	tooltipLabel: string;
	tooltipDescription: string;
	tooltipShortcut: ActionShortcut | undefined;
};

export type ColorInput = {
	kind: WidgetPropsNames;

	// Content
	value: FillChoice;
	allowNone: boolean;
	// allowTransparency: boolean; // TODO: Implement
	menuDirection: MenuDirection | undefined;
	disabled: boolean;

	// Styling
	narrow: boolean;

	// Tooltips
	tooltipLabel: string;
	tooltipDescription: string;
	tooltipShortcut: ActionShortcut | undefined;
};

export type FillChoice = Color | Gradient;

export function contrastingOutlineFactor(value: FillChoice, proximityColor: string | [string, string], proximityRange: number): number {
	const pair = Array.isArray(proximityColor) ? [proximityColor[0], proximityColor[1]] : [proximityColor, proximityColor];
	const [range1, range2] = pair.map((color) => colorFromCSS(window.getComputedStyle(document.body).getPropertyValue(color)) || createNoneColor());

	const contrast = (color: Color): number => {
		const lum = colorLuminance(color) || 0;
		let rangeLuminance1 = colorLuminance(range1) || 0;
		let rangeLuminance2 = colorLuminance(range2) || 0;
		[rangeLuminance1, rangeLuminance2] = [Math.min(rangeLuminance1, rangeLuminance2), Math.max(rangeLuminance1, rangeLuminance2)];

		const distance = Math.max(0, rangeLuminance1 - lum, lum - rangeLuminance2);

		return (1 - Math.min(distance / proximityRange, 1)) * (1 - (colorToHSVA(color)?.s || 0));
	};

	if (isGradient(value)) {
		if (value.color.length === 0) return 0;

		const first = contrast(value.color[0]);
		const last = contrast(value.color[value.color.length - 1]);

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

export type CurveManipulatorGroup = {
	anchor: [number, number];
	handles: [[number, number], [number, number]];
};

export type Curve = {
	manipulatorGroups: CurveManipulatorGroup[];
	firstHandle: [number, number];
	lastHandle: [number, number];
};

export type CurveInput = {
	kind: WidgetPropsNames;

	// Content
	value: Curve;

	// Tooltips
	tooltipLabel: string;
	tooltipDescription: string;
	tooltipShortcut: ActionShortcut | undefined;
};

export type DropdownInput = {
	kind: WidgetPropsNames;

	// Content
	selectedIndex: number | undefined;
	drawIcon: boolean;
	disabled: boolean;

	// Children
	entries: MenuListEntry[][];
	entriesHash: bigint;

	// Styling
	narrow: boolean;

	// Behavior
	virtualScrolling: boolean;
	interactive: boolean;

	// Sizing
	minWidth: number;
	maxWidth: number;

	// Tooltips
	tooltipLabel: string;
	tooltipDescription: string;
	tooltipShortcut: ActionShortcut | undefined;
};

export type IconButton = {
	kind: WidgetPropsNames;

	// Content
	icon: IconName;
	hoverIcon: IconName | undefined;
	size: IconSize;
	disabled: boolean;

	// Styling
	emphasized: boolean;

	// Tooltips
	tooltipLabel: string;
	tooltipDescription: string;
	tooltipShortcut: ActionShortcut | undefined;
};

export type IconLabel = {
	kind: WidgetPropsNames;

	// Content
	icon: IconName;
	disabled: boolean;

	// Tooltips
	tooltipLabel: string;
	tooltipDescription: string;
	tooltipShortcut: ActionShortcut | undefined;
};

export type ImageButton = {
	kind: WidgetPropsNames;

	// Content
	image: IconName;
	width: string | undefined;
	height: string | undefined;

	// Tooltips
	tooltipLabel: string;
	tooltipDescription: string;
	tooltipShortcut: ActionShortcut | undefined;
};

export type ImageLabel = {
	kind: WidgetPropsNames;

	// Content
	url: string;
	width: string | undefined;
	height: string | undefined;

	// Tooltips
	tooltipLabel: string;
	tooltipDescription: string;
	tooltipShortcut: ActionShortcut | undefined;
};

export type ShortcutLabel = {
	kind: WidgetPropsNames;

	// Content
	shortcut: ActionShortcut | undefined;
};

export type NumberInputIncrementBehavior = "Add" | "Multiply" | "Callback" | "None";
export type NumberInputMode = "Increment" | "Range";

export type NumberInput = {
	kind: WidgetPropsNames;

	// Content
	value: number | undefined;
	label: string | undefined;
	disabled: boolean;

	// Styling
	narrow: boolean;

	// Behavior
	mode: NumberInputMode;
	min: number | undefined;
	max: number | undefined;
	rangeMin: number | undefined;
	rangeMax: number | undefined;
	step: number;
	isInteger: boolean;
	incrementBehavior: NumberInputIncrementBehavior;
	displayDecimalPlaces: number;
	unit: string;
	unitIsHiddenWhenEditing: boolean;

	// Sizing
	minWidth: number;
	maxWidth: number;

	// Tooltips
	tooltipLabel: string;
	tooltipDescription: string;
	tooltipShortcut: ActionShortcut | undefined;
};

export type NodeCatalog = {
	kind: WidgetPropsNames;

	// Content
	disabled: boolean;

	// Behavior
	initialSearchTerm: string;
};

export type PopoverButton = {
	kind: WidgetPropsNames;

	// Content
	style: PopoverButtonStyle | undefined;
	icon: IconName | undefined;
	disabled: boolean;

	// Children
	popoverLayout: Layout;
	popoverMinWidth: number | undefined;
	menuDirection: MenuDirection | undefined;

	// Tooltips
	tooltipLabel: string;
	tooltipDescription: string;
	tooltipShortcut: ActionShortcut | undefined;
};

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

export type RadioInput = {
	kind: WidgetPropsNames;

	// Content
	selectedIndex: number | undefined;
	disabled: boolean;

	// Children
	entries: RadioEntryData[];

	// Styling
	narrow: boolean;

	// Sizing
	minWidth: number;
};

export type SeparatorDirection = "Horizontal" | "Vertical";
export type SeparatorStyle = "Related" | "Unrelated" | "Section";

export type Separator = {
	kind: WidgetPropsNames;

	// Content
	direction: SeparatorDirection;
	style: SeparatorStyle;
};

export type WorkingColorsInput = {
	kind: WidgetPropsNames;

	// Content
	primary: Color;
	secondary: Color;
};

export type TextAreaInput = {
	kind: WidgetPropsNames;

	// Content
	value: string;
	label: string | undefined;
	disabled: boolean;

	// Tooltips
	tooltipLabel: string;
	tooltipDescription: string;
	tooltipShortcut: ActionShortcut | undefined;
};

export type ParameterExposeButton = {
	kind: WidgetPropsNames;

	// Content
	exposed: boolean;
	dataType: FrontendGraphDataType;

	// Tooltips
	tooltipLabel: string;
	tooltipDescription: string;
	tooltipShortcut: ActionShortcut | undefined;
};

export type TextButton = {
	kind: WidgetPropsNames;

	// Content
	label: string;
	icon: IconName | undefined;
	hoverIcon: IconName | undefined;
	disabled: boolean;

	// Children
	menuListChildren: MenuListEntry[][];
	menuListChildrenHash: bigint;

	// Styling
	emphasized: boolean;
	flush: boolean;
	narrow: boolean;

	// Sizing
	minWidth: number;

	// Tooltips
	tooltipLabel: string;
	tooltipDescription: string;
	tooltipShortcut: ActionShortcut | undefined;
};

export type BreadcrumbTrailButtons = {
	kind: WidgetPropsNames;

	// Content
	labels: string[];
	disabled: boolean;

	// Tooltips
	tooltipLabel: string;
	tooltipDescription: string;
	tooltipShortcut: ActionShortcut | undefined;
};

export type TextInput = {
	kind: WidgetPropsNames;

	// Content
	value: string;
	label: string | undefined;
	placeholder: string | undefined;
	disabled: boolean;

	// Styling
	narrow: boolean;
	centered: boolean;

	// Sizing
	minWidth: number;
	maxWidth: number;

	// Tooltips
	tooltipLabel: string;
	tooltipDescription: string;
	tooltipShortcut: ActionShortcut | undefined;
};

export type TextLabel = {
	kind: WidgetPropsNames;

	// Content
	value: string;
	disabled: boolean;
	forCheckbox: bigint | undefined;

	// Styling
	narrow: boolean;
	bold: boolean;
	italic: boolean;
	monospace: boolean;
	multiline: boolean;
	centerAlign: boolean;
	tableAlign: boolean;

	// Sizing
	minWidth: number;
	minWidthCharacters: number;

	// Tooltips
	tooltipLabel: string;
	tooltipDescription: string;
	tooltipShortcut: ActionShortcut | undefined;
};

export type ReferencePoint = "None" | "TopLeft" | "TopCenter" | "TopRight" | "CenterLeft" | "Center" | "CenterRight" | "BottomLeft" | "BottomCenter" | "BottomRight";

export type ReferencePointInput = {
	kind: WidgetPropsNames;

	// Content
	value: ReferencePoint;
	disabled: boolean;

	// Tooltips
	tooltipLabel: string;
	tooltipDescription: string;
	tooltipShortcut: ActionShortcut | undefined;
};

// WIDGET

export type WidgetTypes = {
	BreadcrumbTrailButtons: BreadcrumbTrailButtons;
	CheckboxInput: CheckboxInput;
	ColorInput: ColorInput;
	CurveInput: CurveInput;
	DropdownInput: DropdownInput;
	IconButton: IconButton;
	IconLabel: IconLabel;
	ImageButton: ImageButton;
	ImageLabel: ImageLabel;
	NodeCatalog: NodeCatalog;
	NumberInput: NumberInput;
	ParameterExposeButton: ParameterExposeButton;
	PopoverButton: PopoverButton;
	RadioInput: RadioInput;
	ReferencePointInput: ReferencePointInput;
	Separator: Separator;
	ShortcutLabel: ShortcutLabel;
	TextAreaInput: TextAreaInput;
	TextButton: TextButton;
	TextInput: TextInput;
	TextLabel: TextLabel;
	WorkingColorsInput: WorkingColorsInput;
};
export type WidgetPropsNames = keyof WidgetTypes;
export type WidgetPropsSet = WidgetTypes[WidgetPropsNames];

export type WidgetInstance = {
	props: WidgetPropsSet;
	widgetId: bigint;
};

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function hoistWidgetInstance(widgetInstance: any): WidgetInstance {
	const kind = Object.keys(widgetInstance.widget)[0];
	const props = widgetInstance.widget[kind];
	props.kind = kind;

	if (kind === "PopoverButton") {
		props.popoverLayout = props.popoverLayout.map(createLayoutGroup);
	}
	if (kind === "ColorInput") {
		props.value = parseFillChoice(props.value);
	}

	const { widgetId } = widgetInstance;

	return { props, widgetId };
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

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function parseWidgetDiffs(rawDiffs: any): WidgetDiff[] {
	return rawDiffs.map((diff: WidgetDiff) => {
		const { widgetPath, newValue } = diff;

		if ("layout" in newValue) return { widgetPath, newValue: newValue.layout.map(createLayoutGroup) };
		if ("layoutGroup" in newValue) return { widgetPath, newValue: createLayoutGroup(newValue.layoutGroup) };
		if ("widget" in newValue) return { widgetPath, newValue: hoistWidgetInstance(newValue.widget) };

		// This code should be unreachable
		throw new Error("DiffUpdate invalid");
	});
}

type DiffUpdate = { layout: Layout } | { layoutGroup: LayoutGroup } | { widget: WidgetInstance };
export type WidgetDiff = { widgetPath: number[]; newValue: DiffUpdate };

type UIItem = Layout | LayoutGroup | WidgetInstance[] | WidgetInstance;

// Updates a widget layout based on a list of updates, giving the new layout by mutating the `layout` argument
export function patchLayout(layout: /* &mut */ Layout, diffs: WidgetDiff[]) {
	diffs.forEach((update) => {
		// Find the object where the diff applies to
		const diffObject = update.widgetPath.reduce((targetLayout: UIItem | undefined, index: number): UIItem | undefined => {
			if (targetLayout && "columnWidgets" in targetLayout) return targetLayout.columnWidgets[index];
			if (targetLayout && "rowWidgets" in targetLayout) return targetLayout.rowWidgets[index];
			if (targetLayout && "tableWidgets" in targetLayout) return targetLayout.tableWidgets[index];
			if (targetLayout && "layout" in targetLayout) return targetLayout.layout[index];
			if (targetLayout && "props" in targetLayout && "widgetId" in targetLayout) {
				if (targetLayout.props.kind === "PopoverButton" && "popoverLayout" in targetLayout.props && targetLayout.props.popoverLayout) {
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
		Object.keys(diffObject).forEach((key) => delete (diffObject as Record<string, unknown>)[key]);

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
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function createLayoutGroup(layoutGroup: any): LayoutGroup {
	if (layoutGroup.column) {
		const columnWidgets = layoutGroup.column.columnWidgets.map(hoistWidgetInstance);

		const result: WidgetSpanColumn = { columnWidgets };
		return result;
	}

	if (layoutGroup.row) {
		const result: WidgetSpanRow = { rowWidgets: layoutGroup.row.rowWidgets.map(hoistWidgetInstance) };
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
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			tableWidgets: layoutGroup.table.tableWidgets.map((row: any) => row.map(hoistWidgetInstance)),
			unstyled: layoutGroup.table.unstyled,
		};
		return result;
	}

	throw new Error("Layout row type does not exist");
}

// JS MESSAGE TYPES

export type JsMessageTypeMap = {
	ClearAllNodeGraphWires: Record<string, never>;
	DisplayDialog: { readonly title: string; readonly icon: IconName };
	DialogClose: Record<string, never>;
	DisplayDialogPanic: { readonly panicInfo: string };
	DisplayEditableTextbox: {
		readonly text: string;
		readonly lineHeightRatio: number;
		readonly fontSize: number;
		readonly color: string;
		readonly fontData: ArrayBuffer;
		readonly transform: number[];
		readonly maxWidth: undefined | number;
		readonly maxHeight: undefined | number;
		readonly align: TextAlign;
	};
	DisplayEditableTextboxTransform: { readonly transform: number[] };
	DisplayEditableTextboxUpdateFontData: { readonly fontData: ArrayBuffer };
	DisplayRemoveEditableTextbox: Record<string, never>;
	SendShortcutAltClick: { readonly shortcut: ActionShortcut | undefined };
	SendShortcutFullscreen: { readonly shortcut: ActionShortcut | undefined; readonly shortcutMac: ActionShortcut | undefined };
	SendShortcutShiftClick: { readonly shortcut: ActionShortcut | undefined };
	SendUIMetadata: { readonly nodeDescriptions: [string, string][]; readonly nodeTypes: FrontendNodeType[] };
	TriggerAboutGraphiteLocalizedCommitDate: { readonly commitDate: string };
	TriggerClipboardRead: Record<string, never>;
	TriggerClipboardWrite: { readonly content: string };
	TriggerDisplayThirdPartyLicensesDialog: Record<string, never>;
	TriggerExportImage: { readonly svg: string; readonly name: string; readonly mime: string; readonly size: [number, number] };
	TriggerFetchAndOpenDocument: { readonly name: string; readonly filename: string };
	TriggerFontCatalogLoad: Record<string, never>;
	TriggerFontDataLoad: { font: Font; url: string };
	TriggerImport: Record<string, never>;
	TriggerLoadFirstAutoSaveDocument: Record<string, never>;
	TriggerLoadPreferences: Record<string, never>;
	TriggerLoadRestAutoSaveDocuments: Record<string, never>;
	TriggerOpen: Record<string, never>;
	TriggerOpenLaunchDocuments: Record<string, never>;
	TriggerPersistenceRemoveDocument: { documentId: bigint };
	TriggerPersistenceWriteDocument: { documentId: bigint; document: string; details: DocumentDetails; version: string };
	TriggerSaveActiveDocument: { readonly documentId: bigint };
	TriggerSaveDocument: { readonly documentId: bigint; readonly name: string; readonly path: string | undefined; readonly content: ArrayBuffer };
	TriggerSaveFile: { readonly name: string; readonly content: ArrayBuffer };
	TriggerSavePreferences: { readonly preferences: Record<string, unknown> };
	TriggerSelectionRead: { readonly cut: boolean };
	TriggerSelectionWrite: { readonly content: string };
	TriggerTextCommit: Record<string, never>;
	TriggerVisitLink: { url: string };
	UpdateActiveDocument: { readonly documentId: bigint };
	UpdateBox: { readonly box: Box | undefined };
	UpdateClickTargets: { readonly clickTargets: FrontendClickTargets | undefined };
	UpdateContextMenuInformation: { readonly contextMenuInformation: ContextMenuInformation | undefined };
	UpdateDataPanelState: { readonly open: boolean };
	UpdateDocumentArtwork: { readonly svg: string };
	UpdateDocumentLayerDetails: { readonly data: LayerPanelEntry };
	UpdateDocumentLayerStructure: { readonly layerStructure: LayerStructureEntry[] };
	UpdateDocumentRulers: { readonly origin: [number, number]; readonly spacing: number; readonly interval: number; readonly visible: boolean };
	UpdateDocumentScrollbars: { readonly position: [number, number]; readonly size: [number, number]; readonly multiplier: [number, number] };
	UpdateExportReorderIndex: { readonly exportIndex: number | undefined };
	UpdateEyedropperSamplingState: {
		readonly image: EyedropperPreviewImage | undefined;
		readonly mousePosition: [number, number] | undefined;
		readonly primaryColor: string;
		readonly secondaryColor: string;
		readonly setColorChoice: "Primary" | "Secondary" | undefined;
	};
	UpdateFullscreen: { readonly fullscreen: boolean };
	UpdateGradientStopColorPickerPosition: { readonly color: Color; readonly x: number; readonly y: number };
	UpdateGraphFadeArtwork: { readonly percentage: number };
	UpdateGraphViewOverlay: { open: boolean };
	UpdateImportReorderIndex: { readonly importIndex: number | undefined };
	UpdateImportsExports: {
		readonly imports: (FrontendGraphOutput | undefined)[];
		readonly exports: (FrontendGraphInput | undefined)[];
		readonly importPosition: [number, number];
		readonly exportPosition: [number, number];
		readonly addImportExport: boolean;
	};
	UpdateInSelectedNetwork: { readonly inSelectedNetwork: boolean };
	UpdateLayersPanelState: { readonly open: boolean };
	UpdateLayerWidths: { readonly layerWidths: Map<bigint, number>; readonly chainWidths: Map<bigint, number>; readonly hasLeftInputWire: Map<bigint, boolean> };
	UpdateLayout: { readonly layoutTarget: LayoutTarget; readonly diff: WidgetDiff[] };
	UpdateMaximized: { readonly maximized: boolean };
	UpdateMouseCursor: { readonly cursor: MouseCursor };
	UpdateNodeGraphErrorDiagnostic: { readonly error: NodeGraphError | undefined };
	UpdateNodeGraphNodes: { readonly nodes: FrontendNode[] };
	UpdateNodeGraphSelection: { readonly selected: bigint[] };
	UpdateNodeGraphTransform: { readonly transform: NodeGraphTransform };
	UpdateNodeGraphWires: { readonly wires: WireUpdate[] };
	UpdateNodeThumbnail: { readonly id: bigint; readonly value: string };
	UpdateOpenDocumentsList: { readonly openDocuments: OpenDocument[] };
	UpdatePlatform: { readonly platform: AppWindowPlatform };
	UpdatePropertiesPanelState: { readonly open: boolean };
	UpdateUIScale: { readonly scale: number };
	UpdateViewportHolePunch: { readonly active: boolean };
	UpdateViewportPhysicalBounds: { readonly x: number; readonly y: number; readonly width: number; readonly height: number };
	UpdateVisibleNodes: { readonly nodes: bigint[] };
	UpdateWirePathInProgress: { readonly wirePath: WirePath | undefined };
	WindowFullscreen: Record<string, never>;
	WindowPointerLockMove: { readonly x: number; readonly y: number };
};
export type JsMessageType = keyof JsMessageTypeMap;

// Standalone type aliases for types used outside subscriptions
export type DisplayEditableTextbox = JsMessageTypeMap["DisplayEditableTextbox"];
export type TriggerPersistenceWriteDocument = JsMessageTypeMap["TriggerPersistenceWriteDocument"];
export type TriggerSavePreferences = JsMessageTypeMap["TriggerSavePreferences"];
export type UpdateImportsExports = JsMessageTypeMap["UpdateImportsExports"];

import type { PopoverButtonStyle, IconName, IconSize } from "@graphite/icons";

export type NodeGraphErrorDiagnostic = {
	position: [number, number];
	error: string;
};

export type OpenDocument = {
	id: bigint;
	details: DocumentDetails;
};

type DocumentDetails = {
	name: string;
	path: string | undefined;
	isSaved: boolean;
	isAutoSaved: boolean;
};

export type BoxSelection = {
	startX: number;
	startY: number;
	endX: number;
	endY: number;
};

export type FrontendClickTargets = {
	nodeClickTargets: string[];
	layerClickTargets: string[];
	connectorClickTargets: string[];
	iconClickTargets: string[];
	allNodesBoundingBox: string;
	modifyImportExport: string[];
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
	dataType: FrontendGraphDataType;
	name: string;
	description: string;
	resolvedType: string;
	validTypes: string[];
	connectedTo: string;
};

export type FrontendGraphOutput = {
	dataType: FrontendGraphDataType;
	name: string;
	description: string;
	resolvedType: string;
	connectedTo: string[];
};

export type FrontendNode = {
	id: bigint;
	isLayer: boolean;
	canBeLayer: boolean;
	reference: string | undefined;
	displayName: string;
	implementationName: string;
	primaryInput: FrontendGraphInput | undefined;
	exposedInputs: FrontendGraphInput[];
	primaryOutput: FrontendGraphOutput | undefined;
	exposedOutputs: FrontendGraphOutput[];
	primaryInputConnectedToLayer: boolean;
	primaryOutputConnectedToLayer: boolean;
	position: [number, number];
	// TODO: Store field for the width of the left node chain
	previewed: boolean;
	visible: boolean;
	locked: boolean;
};

export type FrontendNodeType = {
	identifier: string;
	name: string;
	category: string;
	inputTypes: string[];
};

export type WirePath = {
	pathString: string;
	dataType: FrontendGraphDataType;
	thick: boolean;
	dashed: boolean;
};

export type AppWindowPlatform = "Web" | "Windows" | "Mac" | "Linux";

// Rust enum `Key`
export type Key = string;
// Serde converts a Rust `Key` enum variant into this format with both the `Key` variant name (called `RawKey` in TS) and the localized `label` for the key
export type MouseMotion = "None" | "Lmb" | "Rmb" | "Mmb" | "ScrollUp" | "ScrollDown" | "Drag" | "LmbDouble" | "LmbDrag" | "RmbDrag" | "RmbDouble" | "MmbDrag";
export type LabeledKey = { key: Key; label: string };
export type LabeledShortcutOrMouseMotion = LabeledKey | MouseMotion;
export type LabeledShortcut = LabeledShortcutOrMouseMotion[];
export type ActionShortcut = { shortcut: LabeledShortcut };

// All channels range are represented by 0-1, sRGB, gamma.
export type Color = {
	red: number;
	green: number;
	blue: number;
	alpha: number;
	none: boolean;
};

export type Gradient = {
	position: number[];
	midpoint: number[];
	color: Color[];
};

export type FillChoice = Color | Gradient;

export type EyedropperPreviewImage = {
	data: Uint8Array;
	width: number;
	height: number;
};

export type LayerStructureEntry = {
	layerId: bigint;
	children: LayerStructureEntry[];
};

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

export type MouseCursorIcon = "Default" | "None" | "ZoomIn" | "ZoomOut" | "Grabbing" | "Crosshair" | "Text" | "Move" | "NSResize" | "EWResize" | "NESWResize" | "NWSEResize" | "Rotate";

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

export type WirePathUpdate = {
	id: bigint;
	inputIndex: number;
	wirePathUpdate: WirePath | undefined;
};

type TextAlign = "Left" | "Center" | "Right" | "JustifyLeft";

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
	widgetId: bigint;
	props: WidgetPropsSet; // TODO: Make this be: `widget: Widget;` where `Widget` is https://files.keavon.com/-/SkyblueWorriedGlowworm/capture.png
};

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

export type WidgetDiff = {
	widgetPath: bigint[];
	newValue: { layout: Layout } | { layoutGroup: LayoutGroup } | { widget: WidgetInstance };
};

export type UIItem = Layout | LayoutGroup | WidgetInstance[] | WidgetInstance;
export type LayoutGroup = WidgetSpanRow | WidgetSpanColumn | WidgetTable | WidgetSection;
export type Layout = LayoutGroup[];

export type WidgetSpanColumn = { columnWidgets: WidgetInstance[] };
export type WidgetSpanRow = { rowWidgets: WidgetInstance[] };
export type WidgetTable = { tableWidgets: WidgetInstance[][]; unstyled: boolean };
export type WidgetSection = { name: string; description: string; visible: boolean; pinned: boolean; id: bigint; layout: Layout };

export type FrontendMessages = {
	ClearAllNodeGraphWires: Record<string, never>;
	DisplayDialog: { title: string; icon: string };
	DialogClose: Record<string, never>;
	DisplayDialogPanic: { panicInfo: string };
	DisplayEditableTextbox: {
		text: string;
		lineHeightRatio: number;
		fontSize: number;
		color: string;
		fontData: Uint8Array;
		transform: [number, number, number, number, number, number];
		maxWidth: undefined | number;
		maxHeight: undefined | number;
		align: TextAlign;
	};
	DisplayEditableTextboxTransform: { transform: [number, number, number, number, number, number] };
	DisplayEditableTextboxUpdateFontData: { fontData: Uint8Array };
	DisplayRemoveEditableTextbox: Record<string, never>;
	SendShortcutAltClick: { shortcut: ActionShortcut | undefined };
	SendShortcutFullscreen: { shortcut: ActionShortcut | undefined; shortcutMac: ActionShortcut | undefined };
	SendShortcutShiftClick: { shortcut: ActionShortcut | undefined };
	SendUIMetadata: { nodeDescriptions: [string, string][]; nodeTypes: FrontendNodeType[] };
	TriggerAboutGraphiteLocalizedCommitDate: { commitDate: string };
	TriggerClipboardRead: Record<string, never>;
	TriggerClipboardWrite: { content: string };
	TriggerDisplayThirdPartyLicensesDialog: Record<string, never>;
	TriggerExportImage: { svg: string; name: string; mime: string; size: [number, number] };
	TriggerFetchAndOpenDocument: { name: string; filename: string };
	TriggerFontCatalogLoad: Record<string, never>;
	TriggerFontDataLoad: { font: Font; url: string };
	TriggerImport: Record<string, never>;
	TriggerLoadFirstAutoSaveDocument: Record<string, never>;
	TriggerLoadPreferences: Record<string, never>;
	TriggerLoadRestAutoSaveDocuments: Record<string, never>;
	TriggerOpen: Record<string, never>;
	TriggerOpenLaunchDocuments: Record<string, never>;
	TriggerPersistenceRemoveDocument: { documentId: bigint };
	TriggerPersistenceWriteDocument: { documentId: bigint; document: string; details: DocumentDetails };
	TriggerSaveActiveDocument: { documentId: bigint };
	TriggerSaveDocument: { documentId: bigint; name: string; path: string | undefined; content: Uint8Array };
	TriggerSaveFile: { name: string; content: Uint8Array };
	TriggerSavePreferences: { preferences: unknown };
	TriggerSelectionRead: { cut: boolean };
	TriggerSelectionWrite: { content: string };
	TriggerTextCommit: Record<string, never>;
	TriggerVisitLink: { url: string };
	UpdateActiveDocument: { documentId: bigint };
	UpdateBox: { box: BoxSelection | undefined };
	UpdateClickTargets: { clickTargets: FrontendClickTargets | undefined };
	UpdateContextMenuInformation: { contextMenuInformation: ContextMenuInformation | undefined };
	UpdateDataPanelState: { open: boolean };
	UpdateDocumentArtwork: { svg: string };
	UpdateDocumentLayerDetails: { data: LayerPanelEntry };
	UpdateDocumentLayerStructure: { layerStructure: LayerStructureEntry[] };
	UpdateDocumentRulers: { origin: [number, number]; spacing: number; interval: number; visible: boolean };
	UpdateDocumentScrollbars: { position: [number, number]; size: [number, number]; multiplier: [number, number] };
	UpdateExportReorderIndex: { exportIndex: number | undefined };
	UpdateEyedropperSamplingState: {
		image: EyedropperPreviewImage | undefined;
		mousePosition: [number, number] | undefined;
		primaryColor: string;
		secondaryColor: string;
		setColorChoice: "Primary" | "Secondary" | undefined;
	};
	UpdateFullscreen: { fullscreen: boolean };
	UpdateGradientStopColorPickerPosition: { color: Color; position: [number, number] };
	UpdateGraphFadeArtwork: { percentage: number };
	UpdateGraphViewOverlay: { open: boolean };
	UpdateImportReorderIndex: { importIndex: number | undefined };
	UpdateImportsExports: {
		imports: (FrontendGraphOutput | undefined)[];
		exports: (FrontendGraphInput | undefined)[];
		importPosition: [number, number];
		exportPosition: [number, number];
		addImportExport: boolean;
	};
	UpdateInSelectedNetwork: { inSelectedNetwork: boolean };
	UpdateLayersPanelState: { open: boolean };
	UpdateLayerWidths: { layerWidths: Map<bigint, number>; chainWidths: Map<bigint, number>; hasLeftInputWire: Map<bigint, boolean> };
	UpdateLayout: { layoutTarget: LayoutTarget; diff: WidgetDiff[] };
	UpdateMaximized: { maximized: boolean };
	UpdateMouseCursor: { cursor: MouseCursorIcon };
	UpdateNodeGraphErrorDiagnostic: { error: NodeGraphErrorDiagnostic | undefined };
	UpdateNodeGraphNodes: { nodes: FrontendNode[] };
	UpdateNodeGraphSelection: { selected: bigint[] };
	UpdateNodeGraphTransform: { translation: [number, number]; scale: number };
	UpdateNodeGraphWires: { wires: WirePathUpdate[] };
	UpdateNodeThumbnail: { id: bigint; value: string };
	UpdateOpenDocumentsList: { openDocuments: OpenDocument[] };
	UpdatePlatform: { platform: AppWindowPlatform };
	UpdatePropertiesPanelState: { open: boolean };
	UpdateUIScale: { scale: number };
	UpdateViewportHolePunch: { active: boolean };
	UpdateVisibleNodes: { nodes: bigint[] };
	UpdateWirePathInProgress: { wirePath: WirePath | undefined };
	WindowFullscreen: Record<string, never>;
	WindowPointerLockMove: { position: [number, number] };
};

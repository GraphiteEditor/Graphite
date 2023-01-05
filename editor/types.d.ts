export type ActionKeys =
{ Action: MessageDiscriminant } |
{ keys: LayoutKeysGroup };

export type BreadcrumbTrailButtons =
{
labels: Array<string>,
disabled: boolean,
tooltip: string
};

export type CheckboxInput =
{
checked: boolean,
disabled: boolean,
icon: string,
tooltip: string
};

export type Color =
{
red: number,
green: number,
blue: number,
alpha: number
};

export type ColorInput =
{
value: (undefined | Color),
noTransparency: boolean,
disabled: boolean,
tooltip: string
};

export type DVec2 =
[number, number];

export type DiffUpdate =
{ subLayout: Array<LayoutGroup> } |
{ layoutGroup: LayoutGroup } |
{ widget: WidgetHolder };

export type DropdownEntryData =
{
value: string,
label: string,
icon: string,
shortcut: Array<string>,
shortcutRequiresLock: boolean,
disabled: boolean,
children: Array<Array<DropdownEntryData>>
};

export type DropdownInput =
{
entries: Array<Array<DropdownEntryData>>,
selectedIndex: (undefined | number),
drawIcon: boolean,
interactive: boolean,
disabled: boolean,
tooltip: string
};

export type Font =
{
fontFamily: string,
fontStyle: string
};

export type FontInput =
{
fontFamily: string,
fontStyle: string,
isStyle: boolean,
disabled: boolean,
tooltip: string
};

export type FrontendDocumentDetails =
{
isAutoSaved: boolean,
isSaved: boolean,
name: string,
id: number
};

export type FrontendGraphDataType =
{ general: null } |
{ raster: null } |
{ color: null } |
{ number: null } |
{ vector: null } |
{ number: null } |
{ number: null } |
{ vec2: null };

export type FrontendImageData =
{
path: Array<number>,
mime: string
};

export type FrontendMessage =
{ DisplayDialog: {
icon: string
} } |
{ DisplayDialogDismiss: null } |
{ DisplayDialogPanic: {
panicInfo: string,
header: string,
description: string
} } |
{ DisplayEditableTextbox: {
text: string,
lineWidth: (undefined | number),
fontSize: number,
color: Color
} } |
{ DisplayRemoveEditableTextbox: null } |
{ TriggerAboutGraphiteLocalizedCommitDate: {
commitDate: string
} } |
{ TriggerFileDownload: {
document: string,
name: string
} } |
{ TriggerFontLoad: {
font: Font,
isDefault: boolean
} } |
{ TriggerImaginateCheckServerStatus: {
hostname: string
} } |
{ TriggerImaginateGenerate: {
parameters: ImaginateGenerationParameters,
baseImage: (undefined | ImaginateBaseImage),
maskImage: (undefined | ImaginateMaskImage),
maskPaintMode: ImaginateMaskPaintMode,
maskBlurPx: number,
maskFillContent: ImaginateMaskStartingFill,
hostname: string,
refreshFrequency: number,
documentId: number,
layerPath: Array<number>,
nodePath: Array<number>
} } |
{ TriggerImaginateTerminate: {
documentId: number,
layerPath: Array<number>,
nodePath: Array<number>,
hostname: string
} } |
{ TriggerImport: null } |
{ TriggerIndexedDbRemoveDocument: {
documentId: number
} } |
{ TriggerIndexedDbWriteDocument: {
document: string,
details: FrontendDocumentDetails,
version: string
} } |
{ TriggerLoadAutoSaveDocuments: null } |
{ TriggerLoadPreferences: null } |
{ TriggerNodeGraphFrameGenerate: {
documentId: number,
layerPath: Array<number>,
svg: string,
size: DVec2,
imaginateNode: (undefined | Array<number>)
} } |
{ TriggerOpenDocument: null } |
{ TriggerPaste: null } |
{ TriggerRasterDownload: {
svg: string,
name: string,
mime: string,
size: (Array<number> & { length: 2 })
} } |
{ TriggerRefreshBoundsOfViewports: null } |
{ TriggerRevokeBlobUrl: {
url: string
} } |
{ TriggerSavePreferences: {
preferences: PreferencesMessageHandler
} } |
{ TriggerTextCommit: null } |
{ TriggerTextCopy: {
copyText: string
} } |
{ TriggerViewportResize: null } |
{ TriggerVisitLink: {
url: string
} } |
{ UpdateActiveDocument: {
documentId: number
} } |
{ UpdateDialogDetails: {
layoutTarget: LayoutTarget,
diff: Array<WidgetDiff>
} } |
{ UpdateDocumentArtboards: {
svg: string
} } |
{ UpdateDocumentArtwork: {
svg: string
} } |
{ UpdateDocumentBarLayout: {
layoutTarget: LayoutTarget,
diff: Array<WidgetDiff>
} } |
{ UpdateDocumentLayerDetails: {
data: LayerPanelEntry
} } |
{ UpdateDocumentLayerTreeStructure: {
dataBuffer: RawBuffer
} } |
{ UpdateDocumentLayerTreeStructureJs: {
dataBuffer: JsRawBuffer
} } |
{ UpdateDocumentModeLayout: {
layoutTarget: LayoutTarget,
diff: Array<WidgetDiff>
} } |
{ UpdateDocumentOverlays: {
svg: string
} } |
{ UpdateDocumentRulers: {
origin: (Array<number> & { length: 2 }),
spacing: number,
interval: number
} } |
{ UpdateDocumentScrollbars: {
position: (Array<number> & { length: 2 }),
size: (Array<number> & { length: 2 }),
multiplier: (Array<number> & { length: 2 })
} } |
{ UpdateEyedropperSamplingState: {
mousePosition: (undefined | (Array<number> & { length: 2 })),
primaryColor: string,
secondaryColor: string,
setColorChoice: (undefined | string)
} } |
{ UpdateImageData: {
documentId: number,
imageData: Array<FrontendImageData>
} } |
{ UpdateInputHints: {
hintData: HintData
} } |
{ UpdateLayerTreeOptionsLayout: {
layoutTarget: LayoutTarget,
diff: Array<WidgetDiff>
} } |
{ UpdateMenuBarLayout: {
layoutTarget: LayoutTarget,
layout: Array<MenuBarEntry>
} } |
{ UpdateMouseCursor: {
cursor: MouseCursorIcon
} } |
{ UpdateNodeGraph: {
nodes: Array<FrontendNode>,
links: Array<FrontendNodeLink>
} } |
{ UpdateNodeGraphBarLayout: {
layoutTarget: LayoutTarget,
diff: Array<WidgetDiff>
} } |
{ UpdateNodeGraphSelection: {
selected: Array<number>
} } |
{ UpdateNodeGraphVisibility: {
visible: boolean
} } |
{ UpdateNodeTypes: {
nodeTypes: Array<FrontendNodeType>
} } |
{ UpdateOpenDocumentsList: {
openDocuments: Array<FrontendDocumentDetails>
} } |
{ UpdatePropertyPanelOptionsLayout: {
layoutTarget: LayoutTarget,
diff: Array<WidgetDiff>
} } |
{ UpdatePropertyPanelSectionsLayout: {
layoutTarget: LayoutTarget,
diff: Array<WidgetDiff>
} } |
{ UpdateToolOptionsLayout: {
layoutTarget: LayoutTarget,
diff: Array<WidgetDiff>
} } |
{ UpdateToolShelfLayout: {
layoutTarget: LayoutTarget,
diff: Array<WidgetDiff>
} } |
{ UpdateWorkingColorsLayout: {
layoutTarget: LayoutTarget,
diff: Array<WidgetDiff>
} };

export type FrontendNode =
{
id: number,
displayName: string,
primaryInput: (undefined | FrontendGraphDataType),
exposedInputs: Array<NodeGraphInput>,
outputs: Array<FrontendGraphDataType>,
position: (Array<number> & { length: 2 }),
disabled: boolean,
output: boolean
};

export type FrontendNodeLink =
{
linkStart: number,
linkEnd: number,
linkEndInputIndex: number
};

export type FrontendNodeType =
{
name: string,
category: string
};

export type HintData =
Array<HintGroup>;

export type HintGroup =
Array<HintInfo>;

export type HintInfo =
{
keyGroups: Array<LayoutKeysGroup>,
keyGroupsMac: (undefined | Array<LayoutKeysGroup>),
mouse: (undefined | MouseMotion),
label: string,
plus: boolean
};

export type IconButton =
{
icon: string,
size: number,
disabled: boolean,
active: boolean,
tooltip: string
};

export type IconLabel =
{
icon: string,
disabled: boolean,
tooltip: string
};

export type ImaginateBaseImage =
{
mime: string,
imageData: Array<number>,
size: DVec2
};

export type ImaginateGenerationParameters =
{
seed: number,
samples: number,
samplingMethod: string,
denoisingStrength: (undefined | number),
cfgScale: number,
prompt: string,
negativePrompt: string,
resolution: (Array<number> & { length: 2 }),
restoreFaces: boolean,
tiling: boolean
};

export type ImaginateMaskImage =
{
svg: string,
size: DVec2
};

export type ImaginateMaskPaintMode =
{ Inpaint: null } |
{ Outpaint: null };

export type ImaginateMaskStartingFill =
{ Fill: null } |
{ Original: null } |
{ LatentNoise: null } |
{ LatentNothing: null };

export type InvisibleStandinInput =
{

};

export type JsRawBuffer =
Array<number>;

export type LayerMetadata =
{
selected: boolean,
expanded: boolean
};

export type LayerPanelEntry =
{
name: string,
tooltip: string,
visible: boolean,
layerType: LayerDataTypeDiscriminant,
layerMetadata: LayerMetadata,
path: Array<number>,
thumbnail: string
};

export type LayerReferenceInput =
{
value: (undefined | Array<number>),
layerName: (undefined | string),
layerType: (undefined | LayerDataTypeDiscriminant),
disabled: boolean,
tooltip: string,
minWidth: number
};

export type LayoutGroup =
{ column: {
columnWidgets: Array<WidgetHolder>
} } |
{ row: {
rowWidgets: Array<WidgetHolder>
} } |
{ section: {
name: string,
layout: Array<LayoutGroup>
} };

export type LayoutKey =
{
key: string,
label: string
};

export type LayoutKeysGroup =
Array<LayoutKey>;

export type LayoutTarget =
{ DialogDetails: null } |
{ DocumentBar: null } |
{ DocumentMode: null } |
{ LayerTreeOptions: null } |
{ MenuBar: null } |
{ NodeGraphBar: null } |
{ PropertiesOptions: null } |
{ PropertiesSections: null } |
{ ToolOptions: null } |
{ ToolShelf: null } |
{ WorkingColors: null } |
{ LayoutTargetLength: null };

export type MenuBarEntry =
{
label: string,
icon: (undefined | string),
shortcut: (undefined | ActionKeys),
action: WidgetHolder,
children: MenuBarEntryChildren
};

export type MenuBarEntryChildren =
Array<Array<MenuBarEntry>>;

export type MouseCursorIcon =
{ Default: null } |
{ None: null } |
{ ZoomIn: null } |
{ ZoomOut: null } |
{ Grabbing: null } |
{ Crosshair: null } |
{ Text: null } |
{ Move: null } |
{ NSResize: null } |
{ EWResize: null } |
{ NESWResize: null } |
{ NWSEResize: null } |
{ Rotate: null };

export type MouseMotion =
{ None: null } |
{ Lmb: null } |
{ Rmb: null } |
{ Mmb: null } |
{ ScrollUp: null } |
{ ScrollDown: null } |
{ Drag: null } |
{ LmbDrag: null } |
{ RmbDrag: null } |
{ MmbDrag: null };

export type NodeGraphInput =
{
dataType: FrontendGraphDataType,
name: string
};

export type NumberInput =
{
label: string,
tooltip: string,
disabled: boolean,
value: (undefined | number),
min: (undefined | number),
max: (undefined | number),
isInteger: boolean,
displayDecimalPlaces: number,
unit: string,
unitIsHiddenWhenEditing: boolean,
mode: NumberInputMode,
incrementBehavior: NumberInputIncrementBehavior,
step: number,
rangeMin: (undefined | number),
rangeMax: (undefined | number),
minWidth: number
};

export type NumberInputIncrementBehavior =
{ Add: null } |
{ Multiply: null } |
{ Callback: null };

export type NumberInputMode =
{ Increment: null } |
{ Range: null };

export type OptionalInput =
{
checked: boolean,
disabled: boolean,
icon: string,
tooltip: string
};

export type ParameterExposeButton =
{
exposed: boolean,
dataType: FrontendGraphDataType,
tooltip: string
};

export type PivotAssist =
{
position: PivotPosition,
disabled: boolean
};

export type PivotPosition =
{ None: null } |
{ TopLeft: null } |
{ TopCenter: null } |
{ TopRight: null } |
{ CenterLeft: null } |
{ Center: null } |
{ CenterRight: null } |
{ BottomLeft: null } |
{ BottomCenter: null } |
{ BottomRight: null };

export type PopoverButton =
{
icon: (undefined | string),
disabled: boolean,
header: string,
text: string,
tooltip: string
};

export type PreferencesMessageHandler =
{
imaginate_server_hostname: string,
imaginate_refresh_frequency: number
};

export type RadioEntryData =
{
value: string,
label: string,
icon: string,
tooltip: string
};

export type RadioInput =
{
entries: Array<RadioEntryData>,
disabled: boolean,
selectedIndex: number
};

export type RawBuffer =
Array<number>;

export type Separator =
{
direction: SeparatorDirection,
type: SeparatorType
};

export type SeparatorDirection =
{ Horizontal: null } |
{ Vertical: null };

export type SeparatorType =
{ Related: null } |
{ Unrelated: null } |
{ Section: null } |
{ List: null };

export type SwatchPairInput =
{
primary: Color,
secondary: Color
};

export type TextAreaInput =
{
value: string,
label: (undefined | string),
disabled: boolean,
tooltip: string
};

export type TextButton =
{
label: string,
icon: (undefined | string),
emphasized: boolean,
minWidth: number,
disabled: boolean,
tooltip: string
};

export type TextInput =
{
value: string,
label: (undefined | string),
disabled: boolean,
tooltip: string,
centered: boolean,
minWidth: number
};

export type TextLabel =
{
disabled: boolean,
bold: boolean,
italic: boolean,
tableAlign: boolean,
multiline: boolean,
minWidth: number,
tooltip: string,
value: string
};

export type Widget =
{ BreadcrumbTrailButtons: BreadcrumbTrailButtons } |
{ CheckboxInput: CheckboxInput } |
{ ColorInput: ColorInput } |
{ DropdownInput: DropdownInput } |
{ FontInput: FontInput } |
{ IconButton: IconButton } |
{ IconLabel: IconLabel } |
{ InvisibleStandinInput: InvisibleStandinInput } |
{ LayerReferenceInput: LayerReferenceInput } |
{ NumberInput: NumberInput } |
{ OptionalInput: OptionalInput } |
{ ParameterExposeButton: ParameterExposeButton } |
{ PivotAssist: PivotAssist } |
{ PopoverButton: PopoverButton } |
{ RadioInput: RadioInput } |
{ Separator: Separator } |
{ SwatchPairInput: SwatchPairInput } |
{ TextAreaInput: TextAreaInput } |
{ TextButton: TextButton } |
{ TextInput: TextInput } |
{ TextLabel: TextLabel };

export type WidgetDiff =
{
widgetPath: Array<number>,
newValue: DiffUpdate
};

export type WidgetHolder =
{
widgetId: number,
widget: Widget
};

export type FrontendMessage_keyed = { DisplayDialog: {
icon: string
}, DisplayDialogDismiss: null, DisplayDialogPanic: {
panicInfo: string,
header: string,
description: string
}, DisplayEditableTextbox: {
text: string,
lineWidth: (undefined | number),
fontSize: number,
color: Color
}, DisplayRemoveEditableTextbox: null, TriggerAboutGraphiteLocalizedCommitDate: {
commitDate: string
}, TriggerFileDownload: {
document: string,
name: string
}, TriggerFontLoad: {
font: Font,
isDefault: boolean
}, TriggerImaginateCheckServerStatus: {
hostname: string
}, TriggerImaginateGenerate: {
parameters: ImaginateGenerationParameters,
baseImage: (undefined | ImaginateBaseImage),
maskImage: (undefined | ImaginateMaskImage),
maskPaintMode: ImaginateMaskPaintMode,
maskBlurPx: number,
maskFillContent: ImaginateMaskStartingFill,
hostname: string,
refreshFrequency: number,
documentId: number,
layerPath: Array<number>,
nodePath: Array<number>
}, TriggerImaginateTerminate: {
documentId: number,
layerPath: Array<number>,
nodePath: Array<number>,
hostname: string
}, TriggerImport: null, TriggerIndexedDbRemoveDocument: {
documentId: number
}, TriggerIndexedDbWriteDocument: {
document: string,
details: FrontendDocumentDetails,
version: string
}, TriggerLoadAutoSaveDocuments: null, TriggerLoadPreferences: null, TriggerNodeGraphFrameGenerate: {
documentId: number,
layerPath: Array<number>,
svg: string,
size: DVec2,
imaginateNode: (undefined | Array<number>)
}, TriggerOpenDocument: null, TriggerPaste: null, TriggerRasterDownload: {
svg: string,
name: string,
mime: string,
size: (Array<number> & { length: 2 })
}, TriggerRefreshBoundsOfViewports: null, TriggerRevokeBlobUrl: {
url: string
}, TriggerSavePreferences: {
preferences: PreferencesMessageHandler
}, TriggerTextCommit: null, TriggerTextCopy: {
copyText: string
}, TriggerViewportResize: null, TriggerVisitLink: {
url: string
}, UpdateActiveDocument: {
documentId: number
}, UpdateDialogDetails: {
layoutTarget: LayoutTarget,
diff: Array<WidgetDiff>
}, UpdateDocumentArtboards: {
svg: string
}, UpdateDocumentArtwork: {
svg: string
}, UpdateDocumentBarLayout: {
layoutTarget: LayoutTarget,
diff: Array<WidgetDiff>
}, UpdateDocumentLayerDetails: {
data: LayerPanelEntry
}, UpdateDocumentLayerTreeStructure: {
dataBuffer: RawBuffer
}, UpdateDocumentLayerTreeStructureJs: {
dataBuffer: JsRawBuffer
}, UpdateDocumentModeLayout: {
layoutTarget: LayoutTarget,
diff: Array<WidgetDiff>
}, UpdateDocumentOverlays: {
svg: string
}, UpdateDocumentRulers: {
origin: (Array<number> & { length: 2 }),
spacing: number,
interval: number
}, UpdateDocumentScrollbars: {
position: (Array<number> & { length: 2 }),
size: (Array<number> & { length: 2 }),
multiplier: (Array<number> & { length: 2 })
}, UpdateEyedropperSamplingState: {
mousePosition: (undefined | (Array<number> & { length: 2 })),
primaryColor: string,
secondaryColor: string,
setColorChoice: (undefined | string)
}, UpdateImageData: {
documentId: number,
imageData: Array<FrontendImageData>
}, UpdateInputHints: {
hintData: HintData
}, UpdateLayerTreeOptionsLayout: {
layoutTarget: LayoutTarget,
diff: Array<WidgetDiff>
}, UpdateMenuBarLayout: {
layoutTarget: LayoutTarget,
layout: Array<MenuBarEntry>
}, UpdateMouseCursor: {
cursor: MouseCursorIcon
}, UpdateNodeGraph: {
nodes: Array<FrontendNode>,
links: Array<FrontendNodeLink>
}, UpdateNodeGraphBarLayout: {
layoutTarget: LayoutTarget,
diff: Array<WidgetDiff>
}, UpdateNodeGraphSelection: {
selected: Array<number>
}, UpdateNodeGraphVisibility: {
visible: boolean
}, UpdateNodeTypes: {
nodeTypes: Array<FrontendNodeType>
}, UpdateOpenDocumentsList: {
openDocuments: Array<FrontendDocumentDetails>
}, UpdatePropertyPanelOptionsLayout: {
layoutTarget: LayoutTarget,
diff: Array<WidgetDiff>
}, UpdatePropertyPanelSectionsLayout: {
layoutTarget: LayoutTarget,
diff: Array<WidgetDiff>
}, UpdateToolOptionsLayout: {
layoutTarget: LayoutTarget,
diff: Array<WidgetDiff>
}, UpdateToolShelfLayout: {
layoutTarget: LayoutTarget,
diff: Array<WidgetDiff>
}, UpdateWorkingColorsLayout: {
layoutTarget: LayoutTarget,
diff: Array<WidgetDiff>
} };


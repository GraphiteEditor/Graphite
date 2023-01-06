export type ActionKeys =
{ Action: MessageDiscriminant } |
{ keys: LayoutKeysGroup };

export type ArtboardMessageDiscriminant =
"DispatchOperation" |
"AddArtboard" |
"ClearArtboards" |
"DeleteArtboard" |
"RenderArtboards" |
"ResizeArtboard";

export type ArtboardToolMessageDiscriminant =
"Abort" |
"DocumentIsDirty" |
"DeleteSelected" |
"NudgeSelected" |
"PointerDown" |
"PointerMove" |
"PointerUp";

export type BreadcrumbTrailButtons =
{
labels: Array<string>,
disabled: boolean,
tooltip: string
};

export type BroadcastEventDiscriminant =
"DocumentIsDirty" |
"ToolAbort" |
"SelectionChanged";

export type BroadcastMessageDiscriminant =
{ TriggerEvent: BroadcastEventDiscriminant } |
"SubscribeEvent" |
"UnsubscribeEvent";

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

export type DebugMessageDiscriminant =
"ToggleTraceLogs" |
"MessageOff" |
"MessageNames" |
"MessageContents";

export type DialogMessageDiscriminant =
{ ExportDialog: ExportDialogMessageDiscriminant } |
{ NewDocumentDialog: NewDocumentDialogMessageDiscriminant } |
{ PreferencesDialog: PreferencesDialogMessageDiscriminant } |
"CloseAllDocumentsWithConfirmation" |
"CloseDialogAndThen" |
"DisplayDialogError" |
"RequestAboutGraphiteDialog" |
"RequestAboutGraphiteDialogWithLocalizedCommitDate" |
"RequestComingSoonDialog" |
"RequestExportDialog" |
"RequestNewDocumentDialog" |
"RequestPreferencesDialog";

export type DiffUpdate =
{ subLayout: Array<LayoutGroup> } |
{ layoutGroup: LayoutGroup } |
{ widget: WidgetHolder };

export type DocumentMessageDiscriminant =
"DispatchOperation" |
{ Artboard: ArtboardMessageDiscriminant } |
{ Navigation: NavigationMessageDiscriminant } |
{ Overlays: OverlaysMessageDiscriminant } |
{ TransformLayer: TransformLayerMessageDiscriminant } |
{ PropertiesPanel: PropertiesPanelMessageDiscriminant } |
{ NodeGraph: NodeGraphMessageDiscriminant } |
"AbortTransaction" |
"AddSelectedLayers" |
"AlignSelectedLayers" |
"BackupDocument" |
"BooleanOperation" |
"ClearLayerTree" |
"CommitTransaction" |
"CreateEmptyFolder" |
"DebugPrintDocument" |
"DeleteLayer" |
"DeleteSelectedLayers" |
"DeleteSelectedManipulatorPoints" |
"DeselectAllLayers" |
"DeselectAllManipulatorPoints" |
"DirtyRenderDocument" |
"DirtyRenderDocumentInOutlineView" |
"DocumentHistoryBackward" |
"DocumentHistoryForward" |
"DocumentStructureChanged" |
"DuplicateSelectedLayers" |
"ExportDocument" |
"FlipSelectedLayers" |
"FolderChanged" |
"FrameClear" |
"GroupSelectedLayers" |
"LayerChanged" |
"MoveSelectedLayersTo" |
"MoveSelectedManipulatorPoints" |
"NodeGraphFrameGenerate" |
"NodeGraphFrameImaginate" |
"NodeGraphFrameImaginateRandom" |
"NodeGraphFrameImaginateTerminate" |
"NudgeSelectedLayers" |
"PasteImage" |
"Redo" |
"RenameLayer" |
"RenderDocument" |
"RollbackTransaction" |
"SaveDocument" |
"SelectAllLayers" |
"SelectedLayersLower" |
"SelectedLayersLowerToBack" |
"SelectedLayersRaise" |
"SelectedLayersRaiseToFront" |
"SelectedLayersReorder" |
"SelectLayer" |
"SetBlendModeForSelectedLayers" |
"SetImageBlobUrl" |
"SetLayerExpansion" |
"SetLayerName" |
"SetOpacityForSelectedLayers" |
"SetOverlaysVisibility" |
"SetSelectedLayers" |
"SetSnapping" |
"SetTextboxEditability" |
"SetViewMode" |
"StartTransaction" |
"ToggleLayerExpansion" |
"ToggleLayerVisibility" |
"ToggleSelectedHandleMirroring" |
"Undo" |
"UndoFinished" |
"UngroupLayers" |
"UngroupSelectedLayers" |
"UpdateLayerMetadata" |
"ZoomCanvasTo100Percent" |
"ZoomCanvasTo200Percent" |
"ZoomCanvasToFitAll";

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

export type EllipseToolMessageDiscriminant =
"Abort" |
"DragStart" |
"DragStop" |
"Resize";

export type ExportDialogMessageDiscriminant =
"FileName" |
"FileType" |
"ScaleFactor" |
"ExportBounds" |
"Submit";

export type EyedropperToolMessageDiscriminant =
"Abort" |
"LeftPointerDown" |
"LeftPointerUp" |
"PointerMove" |
"RightPointerDown" |
"RightPointerUp";

export type FillToolMessageDiscriminant =
"Abort" |
"LeftPointerDown" |
"RightPointerDown";

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

export type FreehandToolMessageDiscriminant =
"Abort" |
"DragStart" |
"DragStop" |
"PointerMove" |
"UpdateOptions";

export type FrontendDocumentDetails =
{
isAutoSaved: boolean,
isSaved: boolean,
name: string,
id: bigint
};

export type FrontendGraphDataType =
"general" |
"raster" |
"color" |
"number" |
"vector" |
"number" |
"number" |
"vec2";

export type FrontendImageData =
{
path: Array<bigint>,
mime: string
};

export type FrontendMessage =
{ DisplayDialog: {
icon: string
} } |
"DisplayDialogDismiss" |
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
"DisplayRemoveEditableTextbox" |
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
documentId: bigint,
layerPath: Array<bigint>,
nodePath: Array<bigint>
} } |
{ TriggerImaginateTerminate: {
documentId: bigint,
layerPath: Array<bigint>,
nodePath: Array<bigint>,
hostname: string
} } |
"TriggerImport" |
{ TriggerIndexedDbRemoveDocument: {
documentId: bigint
} } |
{ TriggerIndexedDbWriteDocument: {
document: string,
details: FrontendDocumentDetails,
version: string
} } |
"TriggerLoadAutoSaveDocuments" |
"TriggerLoadPreferences" |
{ TriggerNodeGraphFrameGenerate: {
documentId: bigint,
layerPath: Array<bigint>,
svg: string,
size: DVec2,
imaginateNode: (undefined | Array<bigint>)
} } |
"TriggerOpenDocument" |
"TriggerPaste" |
{ TriggerRasterDownload: {
svg: string,
name: string,
mime: string,
size: (Array<number> & { length: 2 })
} } |
"TriggerRefreshBoundsOfViewports" |
{ TriggerRevokeBlobUrl: {
url: string
} } |
{ TriggerSavePreferences: {
preferences: PreferencesMessageHandler
} } |
"TriggerTextCommit" |
{ TriggerTextCopy: {
copyText: string
} } |
"TriggerViewportResize" |
{ TriggerVisitLink: {
url: string
} } |
{ UpdateActiveDocument: {
documentId: bigint
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
documentId: bigint,
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
selected: Array<bigint>
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

export type FrontendMessageDiscriminant =
"DisplayDialog" |
"DisplayDialogDismiss" |
"DisplayDialogPanic" |
"DisplayEditableTextbox" |
"DisplayRemoveEditableTextbox" |
"TriggerAboutGraphiteLocalizedCommitDate" |
"TriggerFileDownload" |
"TriggerFontLoad" |
"TriggerImaginateCheckServerStatus" |
"TriggerImaginateGenerate" |
"TriggerImaginateTerminate" |
"TriggerImport" |
"TriggerIndexedDbRemoveDocument" |
"TriggerIndexedDbWriteDocument" |
"TriggerLoadAutoSaveDocuments" |
"TriggerLoadPreferences" |
"TriggerNodeGraphFrameGenerate" |
"TriggerOpenDocument" |
"TriggerPaste" |
"TriggerRasterDownload" |
"TriggerRefreshBoundsOfViewports" |
"TriggerRevokeBlobUrl" |
"TriggerSavePreferences" |
"TriggerTextCommit" |
"TriggerTextCopy" |
"TriggerViewportResize" |
"TriggerVisitLink" |
"UpdateActiveDocument" |
"UpdateDialogDetails" |
"UpdateDocumentArtboards" |
"UpdateDocumentArtwork" |
"UpdateDocumentBarLayout" |
"UpdateDocumentLayerDetails" |
"UpdateDocumentLayerTreeStructure" |
"UpdateDocumentLayerTreeStructureJs" |
"UpdateDocumentModeLayout" |
"UpdateDocumentOverlays" |
"UpdateDocumentRulers" |
"UpdateDocumentScrollbars" |
"UpdateEyedropperSamplingState" |
"UpdateImageData" |
"UpdateInputHints" |
"UpdateLayerTreeOptionsLayout" |
"UpdateMenuBarLayout" |
"UpdateMouseCursor" |
"UpdateNodeGraph" |
"UpdateNodeGraphBarLayout" |
"UpdateNodeGraphSelection" |
"UpdateNodeGraphVisibility" |
"UpdateNodeTypes" |
"UpdateOpenDocumentsList" |
"UpdatePropertyPanelOptionsLayout" |
"UpdatePropertyPanelSectionsLayout" |
"UpdateToolOptionsLayout" |
"UpdateToolShelfLayout" |
"UpdateWorkingColorsLayout";

export type FrontendNode =
{
id: bigint,
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
linkStart: bigint,
linkEnd: bigint,
linkEndInputIndex: bigint
};

export type FrontendNodeType =
{
name: string,
category: string
};

export type GlobalsMessageDiscriminant =
"SetPlatform";

export type GradientToolMessageDiscriminant =
"Abort" |
"DocumentIsDirty" |
"DeleteStop" |
"InsertStop" |
"PointerDown" |
"PointerMove" |
"PointerUp" |
"UpdateOptions";

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
seed: bigint,
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
"Inpaint" |
"Outpaint";

export type ImaginateMaskStartingFill =
"Fill" |
"Original" |
"LatentNoise" |
"LatentNothing";

export type ImaginateToolMessageDiscriminant =
"Abort" |
"DragStart" |
"DragStop" |
"Resize";

export type InputMapperMessageDiscriminant =
{ KeyDown: KeyDiscriminant } |
{ KeyUp: KeyDiscriminant } |
"DoubleClick" |
"PointerMove" |
"WheelScroll";

export type InputPreprocessorMessageDiscriminant =
"BoundsOfViewports" |
"DoubleClick" |
"KeyDown" |
"KeyUp" |
"PointerDown" |
"PointerMove" |
"PointerUp" |
"WheelScroll";

export type InvisibleStandinInput =
{

};

export type JsRawBuffer =
Array<number>;

export type KeyDiscriminant =
"Digit0" |
"Digit1" |
"Digit2" |
"Digit3" |
"Digit4" |
"Digit5" |
"Digit6" |
"Digit7" |
"Digit8" |
"Digit9" |
"KeyA" |
"KeyB" |
"KeyC" |
"KeyD" |
"KeyE" |
"KeyF" |
"KeyG" |
"KeyH" |
"KeyI" |
"KeyJ" |
"KeyK" |
"KeyL" |
"KeyM" |
"KeyN" |
"KeyO" |
"KeyP" |
"KeyQ" |
"KeyR" |
"KeyS" |
"KeyT" |
"KeyU" |
"KeyV" |
"KeyW" |
"KeyX" |
"KeyY" |
"KeyZ" |
"Backquote" |
"Backslash" |
"BracketLeft" |
"BracketRight" |
"Comma" |
"Equal" |
"Minus" |
"Period" |
"Quote" |
"Semicolon" |
"Slash" |
"Alt" |
"Meta" |
"Shift" |
"Control" |
"Backspace" |
"CapsLock" |
"ContextMenu" |
"Enter" |
"Space" |
"Tab" |
"Delete" |
"End" |
"Help" |
"Home" |
"Insert" |
"PageDown" |
"PageUp" |
"ArrowDown" |
"ArrowLeft" |
"ArrowRight" |
"ArrowUp" |
"NumLock" |
"NumpadAdd" |
"NumpadHash" |
"NumpadMultiply" |
"NumpadParenLeft" |
"NumpadParenRight" |
"Escape" |
"F1" |
"F2" |
"F3" |
"F4" |
"F5" |
"F6" |
"F7" |
"F8" |
"F9" |
"F10" |
"F11" |
"F12" |
"F13" |
"F14" |
"F15" |
"F16" |
"F17" |
"F18" |
"F19" |
"F20" |
"F21" |
"F22" |
"F23" |
"F24" |
"Fn" |
"FnLock" |
"PrintScreen" |
"ScrollLock" |
"Pause" |
"Unidentified" |
"Command" |
"Accel" |
"Lmb" |
"Rmb" |
"Mmb" |
"NumKeys";

export type LayerDataTypeDiscriminant =
"Folder" |
"Shape" |
"Text" |
"Image" |
"NodeGraphFrame";

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
path: Array<bigint>,
thumbnail: string
};

export type LayerReferenceInput =
{
value: (undefined | Array<bigint>),
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

export type LayoutMessageDiscriminant =
"ResendActiveWidget" |
"SendLayout" |
"UpdateLayout";

export type LayoutTarget =
"DialogDetails" |
"DocumentBar" |
"DocumentMode" |
"LayerTreeOptions" |
"MenuBar" |
"NodeGraphBar" |
"PropertiesOptions" |
"PropertiesSections" |
"ToolOptions" |
"ToolShelf" |
"WorkingColors" |
"LayoutTargetLength";

export type LineToolMessageDiscriminant =
"Abort" |
"DragStart" |
"DragStop" |
"Redraw" |
"UpdateOptions";

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

export type MenuBarMessageDiscriminant =
"SendLayout";

export type MessageDiscriminant =
"NoOp" |
"Init" |
{ Broadcast: BroadcastMessageDiscriminant } |
{ Debug: DebugMessageDiscriminant } |
{ Dialog: DialogMessageDiscriminant } |
{ Frontend: FrontendMessageDiscriminant } |
{ Globals: GlobalsMessageDiscriminant } |
{ InputMapper: InputMapperMessageDiscriminant } |
{ InputPreprocessor: InputPreprocessorMessageDiscriminant } |
{ Layout: LayoutMessageDiscriminant } |
{ Portfolio: PortfolioMessageDiscriminant } |
{ Preferences: PreferencesMessageDiscriminant } |
{ Tool: ToolMessageDiscriminant } |
{ Workspace: WorkspaceMessageDiscriminant };

export type MouseCursorIcon =
"Default" |
"None" |
"ZoomIn" |
"ZoomOut" |
"Grabbing" |
"Crosshair" |
"Text" |
"Move" |
"NSResize" |
"EWResize" |
"NESWResize" |
"NWSEResize" |
"Rotate";

export type MouseMotion =
"None" |
"Lmb" |
"Rmb" |
"Mmb" |
"ScrollUp" |
"ScrollDown" |
"Drag" |
"LmbDrag" |
"RmbDrag" |
"MmbDrag";

export type NavigateToolMessageDiscriminant =
"Abort" |
"ClickZoom" |
"PointerMove" |
"RotateCanvasBegin" |
"TransformCanvasEnd" |
"TranslateCanvasBegin" |
"ZoomCanvasBegin";

export type NavigationMessageDiscriminant =
"DecreaseCanvasZoom" |
"FitViewportToBounds" |
"IncreaseCanvasZoom" |
"PointerMove" |
"RotateCanvasBegin" |
"SetCanvasRotation" |
"SetCanvasZoom" |
"TransformCanvasEnd" |
"TranslateCanvas" |
"TranslateCanvasBegin" |
"TranslateCanvasByViewportFraction" |
"WheelCanvasTranslate" |
"WheelCanvasZoom" |
"ZoomCanvasBegin";

export type NewDocumentDialogMessageDiscriminant =
"Name" |
"Infinite" |
"DimensionsX" |
"DimensionsY" |
"Submit";

export type NodeGraphFrameToolMessageDiscriminant =
"Abort" |
"DragStart" |
"DragStop" |
"Resize";

export type NodeGraphInput =
{
dataType: FrontendGraphDataType,
name: string
};

export type NodeGraphMessageDiscriminant =
"CloseNodeGraph" |
"ConnectNodesByLink" |
"Copy" |
"CreateNode" |
"Cut" |
"DeleteNode" |
"DeleteSelectedNodes" |
"DisconnectNodes" |
"DoubleClickNode" |
"DuplicateSelectedNodes" |
"ExitNestedNetwork" |
"ExposeInput" |
"InsertNode" |
"MoveSelectedNodes" |
"OpenNodeGraph" |
"PasteNodes" |
"SelectNodes" |
"SendGraph" |
"SetDrawing" |
"SetInputValue" |
"SetNodeInput" |
"SetQualifiedInputValue" |
"ShiftNode" |
"ToggleHidden" |
"ToggleHiddenImpl" |
"TogglePreview" |
"TogglePreviewImpl";

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
"Add" |
"Multiply" |
"Callback";

export type NumberInputMode =
"Increment" |
"Range";

export type OptionalInput =
{
checked: boolean,
disabled: boolean,
icon: string,
tooltip: string
};

export type OverlaysMessageDiscriminant =
"DispatchOperation" |
"ClearAllOverlays" |
"Rerender";

export type ParameterExposeButton =
{
exposed: boolean,
dataType: FrontendGraphDataType,
tooltip: string
};

export type PathToolMessageDiscriminant =
"Abort" |
"DocumentIsDirty" |
"SelectionChanged" |
"Delete" |
"DragStart" |
"DragStop" |
"InsertPoint" |
"PointerMove";

export type PenToolMessageDiscriminant =
"DocumentIsDirty" |
"Abort" |
"SelectionChanged" |
"Confirm" |
"DragStart" |
"DragStop" |
"PointerMove" |
"Undo" |
"UpdateOptions";

export type PivotAssist =
{
position: PivotPosition,
disabled: boolean
};

export type PivotPosition =
"None" |
"TopLeft" |
"TopCenter" |
"TopRight" |
"CenterLeft" |
"Center" |
"CenterRight" |
"BottomLeft" |
"BottomCenter" |
"BottomRight";

export type PopoverButton =
{
icon: (undefined | string),
disabled: boolean,
header: string,
text: string,
tooltip: string
};

export type PortfolioMessageDiscriminant =
{ MenuBar: MenuBarMessageDiscriminant } |
{ Document: DocumentMessageDiscriminant } |
"DocumentPassMessage" |
"AutoSaveActiveDocument" |
"AutoSaveDocument" |
"CloseActiveDocumentWithConfirmation" |
"CloseAllDocuments" |
"CloseDocument" |
"CloseDocumentWithConfirmation" |
"Copy" |
"Cut" |
"DeleteDocument" |
"DestroyAllDocuments" |
"FontLoaded" |
"ImaginateCheckServerStatus" |
"ImaginateSetGeneratingStatus" |
"ImaginateSetImageData" |
"ImaginateSetServerStatus" |
"Import" |
"LoadDocumentResources" |
"LoadFont" |
"NewDocumentWithName" |
"NextDocument" |
"OpenDocument" |
"OpenDocumentFile" |
"OpenDocumentFileWithId" |
"Paste" |
"PasteIntoFolder" |
"PasteSerializedData" |
"PrevDocument" |
"ProcessNodeGraphFrame" |
"SelectDocument" |
"SetActiveDocument" |
"SetImageBlobUrl" |
"UpdateDocumentWidgets" |
"UpdateOpenDocumentsList";

export type PreferencesDialogMessageDiscriminant =
"Confirm";

export type PreferencesMessageDiscriminant =
"Load" |
"ResetToDefaults" |
"ImaginateRefreshFrequency" |
"ImaginateServerHostname";

export type PreferencesMessageHandler =
{
imaginate_server_hostname: string,
imaginate_refresh_frequency: number
};

export type PropertiesPanelMessageDiscriminant =
"CheckSelectedWasDeleted" |
"CheckSelectedWasUpdated" |
"ClearSelection" |
"Deactivate" |
"Init" |
"ModifyFill" |
"ModifyFont" |
"ModifyName" |
"ModifyPreserveAspect" |
"ModifyStroke" |
"ModifyText" |
"ModifyTransform" |
"ResendActiveProperties" |
"SetActiveLayers" |
"SetPivot" |
"UpdateSelectedDocumentProperties";

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

export type RectangleToolMessageDiscriminant =
"Abort" |
"DragStart" |
"DragStop" |
"Resize";

export type SelectToolMessageDiscriminant =
"Abort" |
"DocumentIsDirty" |
"SelectionChanged" |
"Align" |
"DragStart" |
"DragStop" |
"EditLayer" |
"FlipHorizontal" |
"FlipVertical" |
"PointerMove" |
"SetPivot";

export type Separator =
{
direction: SeparatorDirection,
type: SeparatorType
};

export type SeparatorDirection =
"Horizontal" |
"Vertical";

export type SeparatorType =
"Related" |
"Unrelated" |
"Section" |
"List";

export type ShapeToolMessageDiscriminant =
"Abort" |
"DragStart" |
"DragStop" |
"Resize" |
"UpdateOptions";

export type SplineToolMessageDiscriminant =
"Abort" |
"Confirm" |
"DragStart" |
"DragStop" |
"PointerMove" |
"Undo" |
"UpdateOptions";

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

export type TextToolMessageDiscriminant =
"Abort" |
"DocumentIsDirty" |
"CommitText" |
"Interact" |
"TextChange" |
"UpdateBounds" |
"UpdateOptions";

export type ToolMessageDiscriminant =
{ Select: SelectToolMessageDiscriminant } |
{ Artboard: ArtboardToolMessageDiscriminant } |
{ Navigate: NavigateToolMessageDiscriminant } |
{ Eyedropper: EyedropperToolMessageDiscriminant } |
{ Fill: FillToolMessageDiscriminant } |
{ Gradient: GradientToolMessageDiscriminant } |
{ Path: PathToolMessageDiscriminant } |
{ Pen: PenToolMessageDiscriminant } |
{ Freehand: FreehandToolMessageDiscriminant } |
{ Spline: SplineToolMessageDiscriminant } |
{ Line: LineToolMessageDiscriminant } |
{ Rectangle: RectangleToolMessageDiscriminant } |
{ Ellipse: EllipseToolMessageDiscriminant } |
{ Shape: ShapeToolMessageDiscriminant } |
{ Text: TextToolMessageDiscriminant } |
{ Imaginate: ImaginateToolMessageDiscriminant } |
{ NodeGraphFrame: NodeGraphFrameToolMessageDiscriminant } |
"ActivateToolSelect" |
"ActivateToolArtboard" |
"ActivateToolNavigate" |
"ActivateToolEyedropper" |
"ActivateToolText" |
"ActivateToolFill" |
"ActivateToolGradient" |
"ActivateToolPath" |
"ActivateToolPen" |
"ActivateToolFreehand" |
"ActivateToolSpline" |
"ActivateToolLine" |
"ActivateToolRectangle" |
"ActivateToolEllipse" |
"ActivateToolShape" |
"ActivateToolImaginate" |
"ActivateToolNodeGraphFrame" |
"ActivateTool" |
"DeactivateTools" |
"InitTools" |
"RefreshToolOptions" |
"ResetColors" |
"SelectPrimaryColor" |
"SelectRandomPrimaryColor" |
"SelectSecondaryColor" |
"SwapColors" |
"UpdateCursor" |
"UpdateHints";

export type TransformLayerMessageDiscriminant =
"ApplyTransformOperation" |
"BeginGrab" |
"BeginRotate" |
"BeginScale" |
"CancelTransformOperation" |
"ConstrainX" |
"ConstrainY" |
"PointerMove" |
"TypeBackspace" |
"TypeDecimalPoint" |
"TypeDigit" |
"TypeNegate";

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
widgetPath: Array<bigint>,
newValue: DiffUpdate
};

export type WidgetHolder =
{
widgetId: bigint,
widget: Widget
};

export type WorkspaceMessageDiscriminant =
"NodeGraphToggleVisibility";

export type FrontendMessage_keyed = { DisplayDialog: {
icon: string
}, DisplayDialogDismiss: void, DisplayDialogPanic: {
panicInfo: string,
header: string,
description: string
}, DisplayEditableTextbox: {
text: string,
lineWidth: (undefined | number),
fontSize: number,
color: Color
}, DisplayRemoveEditableTextbox: void, TriggerAboutGraphiteLocalizedCommitDate: {
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
documentId: bigint,
layerPath: Array<bigint>,
nodePath: Array<bigint>
}, TriggerImaginateTerminate: {
documentId: bigint,
layerPath: Array<bigint>,
nodePath: Array<bigint>,
hostname: string
}, TriggerImport: void, TriggerIndexedDbRemoveDocument: {
documentId: bigint
}, TriggerIndexedDbWriteDocument: {
document: string,
details: FrontendDocumentDetails,
version: string
}, TriggerLoadAutoSaveDocuments: void, TriggerLoadPreferences: void, TriggerNodeGraphFrameGenerate: {
documentId: bigint,
layerPath: Array<bigint>,
svg: string,
size: DVec2,
imaginateNode: (undefined | Array<bigint>)
}, TriggerOpenDocument: void, TriggerPaste: void, TriggerRasterDownload: {
svg: string,
name: string,
mime: string,
size: (Array<number> & { length: 2 })
}, TriggerRefreshBoundsOfViewports: void, TriggerRevokeBlobUrl: {
url: string
}, TriggerSavePreferences: {
preferences: PreferencesMessageHandler
}, TriggerTextCommit: void, TriggerTextCopy: {
copyText: string
}, TriggerViewportResize: void, TriggerVisitLink: {
url: string
}, UpdateActiveDocument: {
documentId: bigint
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
documentId: bigint,
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
selected: Array<bigint>
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


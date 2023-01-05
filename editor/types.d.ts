export type ActionKeys =
{ Action: MessageDiscriminant } |
{ keys: LayoutKeysGroup };

export type ArtboardMessageDiscriminant =
{ DispatchOperation: null } |
{ AddArtboard: null } |
{ ClearArtboards: null } |
{ DeleteArtboard: null } |
{ RenderArtboards: null } |
{ ResizeArtboard: null };

export type ArtboardToolMessageDiscriminant =
{ Abort: null } |
{ DocumentIsDirty: null } |
{ DeleteSelected: null } |
{ NudgeSelected: null } |
{ PointerDown: null } |
{ PointerMove: null } |
{ PointerUp: null };

export type BreadcrumbTrailButtons =
{ labels: Array<string>,
disabled: boolean,
tooltip: string };

export type BroadcastEventDiscriminant =
{ DocumentIsDirty: null } |
{ ToolAbort: null } |
{ SelectionChanged: null };

export type BroadcastMessageDiscriminant =
{ TriggerEvent: BroadcastEventDiscriminant } |
{ SubscribeEvent: null } |
{ UnsubscribeEvent: null };

export type CheckboxInput =
{ checked: boolean,
disabled: boolean,
icon: string,
tooltip: string };

export type Color =
{ red: number,
green: number,
blue: number,
alpha: number };

export type ColorInput =
{ value: (undefined | Color),
noTransparency: boolean,
disabled: boolean,
tooltip: string };

export type DVec2 =
[number, number];

export type DebugMessageDiscriminant =
{ ToggleTraceLogs: null } |
{ MessageOff: null } |
{ MessageNames: null } |
{ MessageContents: null };

export type DialogMessageDiscriminant =
{ ExportDialog: ExportDialogMessageDiscriminant } |
{ NewDocumentDialog: NewDocumentDialogMessageDiscriminant } |
{ PreferencesDialog: PreferencesDialogMessageDiscriminant } |
{ CloseAllDocumentsWithConfirmation: null } |
{ CloseDialogAndThen: null } |
{ DisplayDialogError: null } |
{ RequestAboutGraphiteDialog: null } |
{ RequestAboutGraphiteDialogWithLocalizedCommitDate: null } |
{ RequestComingSoonDialog: null } |
{ RequestExportDialog: null } |
{ RequestNewDocumentDialog: null } |
{ RequestPreferencesDialog: null };

export type DiffUpdate =
{ subLayout: Array<LayoutGroup> } |
{ layoutGroup: LayoutGroup } |
{ widget: WidgetHolder };

export type DocumentMessageDiscriminant =
{ DispatchOperation: null } |
{ Artboard: ArtboardMessageDiscriminant } |
{ Navigation: NavigationMessageDiscriminant } |
{ Overlays: OverlaysMessageDiscriminant } |
{ TransformLayer: TransformLayerMessageDiscriminant } |
{ PropertiesPanel: PropertiesPanelMessageDiscriminant } |
{ NodeGraph: NodeGraphMessageDiscriminant } |
{ AbortTransaction: null } |
{ AddSelectedLayers: null } |
{ AlignSelectedLayers: null } |
{ BackupDocument: null } |
{ BooleanOperation: null } |
{ ClearLayerTree: null } |
{ CommitTransaction: null } |
{ CreateEmptyFolder: null } |
{ DebugPrintDocument: null } |
{ DeleteLayer: null } |
{ DeleteSelectedLayers: null } |
{ DeleteSelectedManipulatorPoints: null } |
{ DeselectAllLayers: null } |
{ DeselectAllManipulatorPoints: null } |
{ DirtyRenderDocument: null } |
{ DirtyRenderDocumentInOutlineView: null } |
{ DocumentHistoryBackward: null } |
{ DocumentHistoryForward: null } |
{ DocumentStructureChanged: null } |
{ DuplicateSelectedLayers: null } |
{ ExportDocument: null } |
{ FlipSelectedLayers: null } |
{ FolderChanged: null } |
{ FrameClear: null } |
{ GroupSelectedLayers: null } |
{ LayerChanged: null } |
{ MoveSelectedLayersTo: null } |
{ MoveSelectedManipulatorPoints: null } |
{ NodeGraphFrameGenerate: null } |
{ NodeGraphFrameImaginate: null } |
{ NodeGraphFrameImaginateRandom: null } |
{ NodeGraphFrameImaginateTerminate: null } |
{ NudgeSelectedLayers: null } |
{ PasteImage: null } |
{ Redo: null } |
{ RenameLayer: null } |
{ RenderDocument: null } |
{ RollbackTransaction: null } |
{ SaveDocument: null } |
{ SelectAllLayers: null } |
{ SelectedLayersLower: null } |
{ SelectedLayersLowerToBack: null } |
{ SelectedLayersRaise: null } |
{ SelectedLayersRaiseToFront: null } |
{ SelectedLayersReorder: null } |
{ SelectLayer: null } |
{ SetBlendModeForSelectedLayers: null } |
{ SetImageBlobUrl: null } |
{ SetLayerExpansion: null } |
{ SetLayerName: null } |
{ SetOpacityForSelectedLayers: null } |
{ SetOverlaysVisibility: null } |
{ SetSelectedLayers: null } |
{ SetSnapping: null } |
{ SetTextboxEditability: null } |
{ SetViewMode: null } |
{ StartTransaction: null } |
{ ToggleLayerExpansion: null } |
{ ToggleLayerVisibility: null } |
{ ToggleSelectedHandleMirroring: null } |
{ Undo: null } |
{ UndoFinished: null } |
{ UngroupLayers: null } |
{ UngroupSelectedLayers: null } |
{ UpdateLayerMetadata: null } |
{ ZoomCanvasTo100Percent: null } |
{ ZoomCanvasTo200Percent: null } |
{ ZoomCanvasToFitAll: null };

export type DropdownEntryData =
{ value: string,
label: string,
icon: string,
shortcut: Array<string>,
shortcutRequiresLock: boolean,
disabled: boolean,
children: Array<Array<DropdownEntryData>> };

export type DropdownInput =
{ entries: Array<Array<DropdownEntryData>>,
selectedIndex: (undefined | number),
drawIcon: boolean,
interactive: boolean,
disabled: boolean,
tooltip: string };

export type EllipseToolMessageDiscriminant =
{ Abort: null } |
{ DragStart: null } |
{ DragStop: null } |
{ Resize: null };

export type ExportDialogMessageDiscriminant =
{ FileName: null } |
{ FileType: null } |
{ ScaleFactor: null } |
{ ExportBounds: null } |
{ Submit: null };

export type EyedropperToolMessageDiscriminant =
{ Abort: null } |
{ LeftPointerDown: null } |
{ LeftPointerUp: null } |
{ PointerMove: null } |
{ RightPointerDown: null } |
{ RightPointerUp: null };

export type FillToolMessageDiscriminant =
{ Abort: null } |
{ LeftPointerDown: null } |
{ RightPointerDown: null };

export type Font =
{ fontFamily: string,
fontStyle: string };

export type FontInput =
{ fontFamily: string,
fontStyle: string,
isStyle: boolean,
disabled: boolean,
tooltip: string };

export type FreehandToolMessageDiscriminant =
{ Abort: null } |
{ DragStart: null } |
{ DragStop: null } |
{ PointerMove: null } |
{ UpdateOptions: null };

export type FrontendDocumentDetails =
{ isAutoSaved: boolean,
isSaved: boolean,
name: string,
id: number };

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
{ path: Array<number>,
mime: string };

export type FrontendMessage =
{ DisplayDialog: { icon: string } } |
{ DisplayDialogDismiss: null } |
{ DisplayDialogPanic: { panicInfo: string,
header: string,
description: string } } |
{ DisplayEditableTextbox: { text: string,
lineWidth: (undefined | number),
fontSize: number,
color: Color } } |
{ DisplayRemoveEditableTextbox: null } |
{ TriggerAboutGraphiteLocalizedCommitDate: { commitDate: string } } |
{ TriggerFileDownload: { document: string,
name: string } } |
{ TriggerFontLoad: { font: Font,
isDefault: boolean } } |
{ TriggerImaginateCheckServerStatus: { hostname: string } } |
{ TriggerImaginateGenerate: { parameters: ImaginateGenerationParameters,
baseImage: (undefined | ImaginateBaseImage),
maskImage: (undefined | ImaginateMaskImage),
maskPaintMode: ImaginateMaskPaintMode,
maskBlurPx: number,
maskFillContent: ImaginateMaskStartingFill,
hostname: string,
refreshFrequency: number,
documentId: number,
layerPath: Array<number>,
nodePath: Array<number> } } |
{ TriggerImaginateTerminate: { documentId: number,
layerPath: Array<number>,
nodePath: Array<number>,
hostname: string } } |
{ TriggerImport: null } |
{ TriggerIndexedDbRemoveDocument: { documentId: number } } |
{ TriggerIndexedDbWriteDocument: { document: string,
details: FrontendDocumentDetails,
version: string } } |
{ TriggerLoadAutoSaveDocuments: null } |
{ TriggerLoadPreferences: null } |
{ TriggerNodeGraphFrameGenerate: { documentId: number,
layerPath: Array<number>,
svg: string,
size: DVec2,
imaginateNode: (undefined | Array<number>) } } |
{ TriggerOpenDocument: null } |
{ TriggerPaste: null } |
{ TriggerRasterDownload: { svg: string,
name: string,
mime: string,
size: (Array<number> & { length: 2 }) } } |
{ TriggerRefreshBoundsOfViewports: null } |
{ TriggerRevokeBlobUrl: { url: string } } |
{ TriggerSavePreferences: { preferences: PreferencesMessageHandler } } |
{ TriggerTextCommit: null } |
{ TriggerTextCopy: { copyText: string } } |
{ TriggerViewportResize: null } |
{ TriggerVisitLink: { url: string } } |
{ UpdateActiveDocument: { documentId: number } } |
{ UpdateDialogDetails: { layoutTarget: LayoutTarget,
diff: Array<WidgetDiff> } } |
{ UpdateDocumentArtboards: { svg: string } } |
{ UpdateDocumentArtwork: { svg: string } } |
{ UpdateDocumentBarLayout: { layoutTarget: LayoutTarget,
diff: Array<WidgetDiff> } } |
{ UpdateDocumentLayerDetails: { data: LayerPanelEntry } } |
{ UpdateDocumentLayerTreeStructure: { dataBuffer: RawBuffer } } |
{ UpdateDocumentLayerTreeStructureJs: { dataBuffer: JsRawBuffer } } |
{ UpdateDocumentModeLayout: { layoutTarget: LayoutTarget,
diff: Array<WidgetDiff> } } |
{ UpdateDocumentOverlays: { svg: string } } |
{ UpdateDocumentRulers: { origin: (Array<number> & { length: 2 }),
spacing: number,
interval: number } } |
{ UpdateDocumentScrollbars: { position: (Array<number> & { length: 2 }),
size: (Array<number> & { length: 2 }),
multiplier: (Array<number> & { length: 2 }) } } |
{ UpdateEyedropperSamplingState: { mousePosition: (undefined | (Array<number> & { length: 2 })),
primaryColor: string,
secondaryColor: string,
setColorChoice: (undefined | string) } } |
{ UpdateImageData: { documentId: number,
imageData: Array<FrontendImageData> } } |
{ UpdateInputHints: { hintData: HintData } } |
{ UpdateLayerTreeOptionsLayout: { layoutTarget: LayoutTarget,
diff: Array<WidgetDiff> } } |
{ UpdateMenuBarLayout: { layoutTarget: LayoutTarget,
layout: Array<MenuBarEntry> } } |
{ UpdateMouseCursor: { cursor: MouseCursorIcon } } |
{ UpdateNodeGraph: { nodes: Array<FrontendNode>,
links: Array<FrontendNodeLink> } } |
{ UpdateNodeGraphBarLayout: { layoutTarget: LayoutTarget,
diff: Array<WidgetDiff> } } |
{ UpdateNodeGraphSelection: { selected: Array<number> } } |
{ UpdateNodeGraphVisibility: { visible: boolean } } |
{ UpdateNodeTypes: { nodeTypes: Array<FrontendNodeType> } } |
{ UpdateOpenDocumentsList: { openDocuments: Array<FrontendDocumentDetails> } } |
{ UpdatePropertyPanelOptionsLayout: { layoutTarget: LayoutTarget,
diff: Array<WidgetDiff> } } |
{ UpdatePropertyPanelSectionsLayout: { layoutTarget: LayoutTarget,
diff: Array<WidgetDiff> } } |
{ UpdateToolOptionsLayout: { layoutTarget: LayoutTarget,
diff: Array<WidgetDiff> } } |
{ UpdateToolShelfLayout: { layoutTarget: LayoutTarget,
diff: Array<WidgetDiff> } } |
{ UpdateWorkingColorsLayout: { layoutTarget: LayoutTarget,
diff: Array<WidgetDiff> } };

export type FrontendMessageDiscriminant =
{ DisplayDialog: null } |
{ DisplayDialogDismiss: null } |
{ DisplayDialogPanic: null } |
{ DisplayEditableTextbox: null } |
{ DisplayRemoveEditableTextbox: null } |
{ TriggerAboutGraphiteLocalizedCommitDate: null } |
{ TriggerFileDownload: null } |
{ TriggerFontLoad: null } |
{ TriggerImaginateCheckServerStatus: null } |
{ TriggerImaginateGenerate: null } |
{ TriggerImaginateTerminate: null } |
{ TriggerImport: null } |
{ TriggerIndexedDbRemoveDocument: null } |
{ TriggerIndexedDbWriteDocument: null } |
{ TriggerLoadAutoSaveDocuments: null } |
{ TriggerLoadPreferences: null } |
{ TriggerNodeGraphFrameGenerate: null } |
{ TriggerOpenDocument: null } |
{ TriggerPaste: null } |
{ TriggerRasterDownload: null } |
{ TriggerRefreshBoundsOfViewports: null } |
{ TriggerRevokeBlobUrl: null } |
{ TriggerSavePreferences: null } |
{ TriggerTextCommit: null } |
{ TriggerTextCopy: null } |
{ TriggerViewportResize: null } |
{ TriggerVisitLink: null } |
{ UpdateActiveDocument: null } |
{ UpdateDialogDetails: null } |
{ UpdateDocumentArtboards: null } |
{ UpdateDocumentArtwork: null } |
{ UpdateDocumentBarLayout: null } |
{ UpdateDocumentLayerDetails: null } |
{ UpdateDocumentLayerTreeStructure: null } |
{ UpdateDocumentLayerTreeStructureJs: null } |
{ UpdateDocumentModeLayout: null } |
{ UpdateDocumentOverlays: null } |
{ UpdateDocumentRulers: null } |
{ UpdateDocumentScrollbars: null } |
{ UpdateEyedropperSamplingState: null } |
{ UpdateImageData: null } |
{ UpdateInputHints: null } |
{ UpdateLayerTreeOptionsLayout: null } |
{ UpdateMenuBarLayout: null } |
{ UpdateMouseCursor: null } |
{ UpdateNodeGraph: null } |
{ UpdateNodeGraphBarLayout: null } |
{ UpdateNodeGraphSelection: null } |
{ UpdateNodeGraphVisibility: null } |
{ UpdateNodeTypes: null } |
{ UpdateOpenDocumentsList: null } |
{ UpdatePropertyPanelOptionsLayout: null } |
{ UpdatePropertyPanelSectionsLayout: null } |
{ UpdateToolOptionsLayout: null } |
{ UpdateToolShelfLayout: null } |
{ UpdateWorkingColorsLayout: null };

export type FrontendNode =
{ id: number,
displayName: string,
primaryInput: (undefined | FrontendGraphDataType),
exposedInputs: Array<NodeGraphInput>,
outputs: Array<FrontendGraphDataType>,
position: (Array<number> & { length: 2 }),
disabled: boolean,
output: boolean };

export type FrontendNodeLink =
{ linkStart: number,
linkEnd: number,
linkEndInputIndex: number };

export type FrontendNodeType =
{ name: string,
category: string };

export type GlobalsMessageDiscriminant =
{ SetPlatform: null };

export type GradientToolMessageDiscriminant =
{ Abort: null } |
{ DocumentIsDirty: null } |
{ DeleteStop: null } |
{ InsertStop: null } |
{ PointerDown: null } |
{ PointerMove: null } |
{ PointerUp: null } |
{ UpdateOptions: null };

export type HintData =
Array<HintGroup>;

export type HintGroup =
Array<HintInfo>;

export type HintInfo =
{ keyGroups: Array<LayoutKeysGroup>,
keyGroupsMac: (undefined | Array<LayoutKeysGroup>),
mouse: (undefined | MouseMotion),
label: string,
plus: boolean };

export type IconButton =
{ icon: string,
size: number,
disabled: boolean,
active: boolean,
tooltip: string };

export type IconLabel =
{ icon: string,
disabled: boolean,
tooltip: string };

export type ImaginateBaseImage =
{ mime: string,
imageData: Array<number>,
size: DVec2 };

export type ImaginateGenerationParameters =
{ seed: number,
samples: number,
samplingMethod: string,
denoisingStrength: (undefined | number),
cfgScale: number,
prompt: string,
negativePrompt: string,
resolution: (Array<number> & { length: 2 }),
restoreFaces: boolean,
tiling: boolean };

export type ImaginateMaskImage =
{ svg: string,
size: DVec2 };

export type ImaginateMaskPaintMode =
{ Inpaint: null } |
{ Outpaint: null };

export type ImaginateMaskStartingFill =
{ Fill: null } |
{ Original: null } |
{ LatentNoise: null } |
{ LatentNothing: null };

export type ImaginateToolMessageDiscriminant =
{ Abort: null } |
{ DragStart: null } |
{ DragStop: null } |
{ Resize: null };

export type InputMapperMessageDiscriminant =
{ KeyDown: KeyDiscriminant } |
{ KeyUp: KeyDiscriminant } |
{ DoubleClick: null } |
{ PointerMove: null } |
{ WheelScroll: null };

export type InputPreprocessorMessageDiscriminant =
{ BoundsOfViewports: null } |
{ DoubleClick: null } |
{ KeyDown: null } |
{ KeyUp: null } |
{ PointerDown: null } |
{ PointerMove: null } |
{ PointerUp: null } |
{ WheelScroll: null };

export type InvisibleStandinInput =
{  };

export type JsRawBuffer =
Array<number>;

export type KeyDiscriminant =
{ Digit0: null } |
{ Digit1: null } |
{ Digit2: null } |
{ Digit3: null } |
{ Digit4: null } |
{ Digit5: null } |
{ Digit6: null } |
{ Digit7: null } |
{ Digit8: null } |
{ Digit9: null } |
{ KeyA: null } |
{ KeyB: null } |
{ KeyC: null } |
{ KeyD: null } |
{ KeyE: null } |
{ KeyF: null } |
{ KeyG: null } |
{ KeyH: null } |
{ KeyI: null } |
{ KeyJ: null } |
{ KeyK: null } |
{ KeyL: null } |
{ KeyM: null } |
{ KeyN: null } |
{ KeyO: null } |
{ KeyP: null } |
{ KeyQ: null } |
{ KeyR: null } |
{ KeyS: null } |
{ KeyT: null } |
{ KeyU: null } |
{ KeyV: null } |
{ KeyW: null } |
{ KeyX: null } |
{ KeyY: null } |
{ KeyZ: null } |
{ Backquote: null } |
{ Backslash: null } |
{ BracketLeft: null } |
{ BracketRight: null } |
{ Comma: null } |
{ Equal: null } |
{ Minus: null } |
{ Period: null } |
{ Quote: null } |
{ Semicolon: null } |
{ Slash: null } |
{ Alt: null } |
{ Meta: null } |
{ Shift: null } |
{ Control: null } |
{ Backspace: null } |
{ CapsLock: null } |
{ ContextMenu: null } |
{ Enter: null } |
{ Space: null } |
{ Tab: null } |
{ Delete: null } |
{ End: null } |
{ Help: null } |
{ Home: null } |
{ Insert: null } |
{ PageDown: null } |
{ PageUp: null } |
{ ArrowDown: null } |
{ ArrowLeft: null } |
{ ArrowRight: null } |
{ ArrowUp: null } |
{ NumLock: null } |
{ NumpadAdd: null } |
{ NumpadHash: null } |
{ NumpadMultiply: null } |
{ NumpadParenLeft: null } |
{ NumpadParenRight: null } |
{ Escape: null } |
{ F1: null } |
{ F2: null } |
{ F3: null } |
{ F4: null } |
{ F5: null } |
{ F6: null } |
{ F7: null } |
{ F8: null } |
{ F9: null } |
{ F10: null } |
{ F11: null } |
{ F12: null } |
{ F13: null } |
{ F14: null } |
{ F15: null } |
{ F16: null } |
{ F17: null } |
{ F18: null } |
{ F19: null } |
{ F20: null } |
{ F21: null } |
{ F22: null } |
{ F23: null } |
{ F24: null } |
{ Fn: null } |
{ FnLock: null } |
{ PrintScreen: null } |
{ ScrollLock: null } |
{ Pause: null } |
{ Unidentified: null } |
{ Command: null } |
{ Accel: null } |
{ Lmb: null } |
{ Rmb: null } |
{ Mmb: null } |
{ NumKeys: null };

export type LayerDataTypeDiscriminant =
{ Folder: null } |
{ Shape: null } |
{ Text: null } |
{ Image: null } |
{ NodeGraphFrame: null };

export type LayerMetadata =
{ selected: boolean,
expanded: boolean };

export type LayerPanelEntry =
{ name: string,
tooltip: string,
visible: boolean,
layerType: LayerDataTypeDiscriminant,
layerMetadata: LayerMetadata,
path: Array<number>,
thumbnail: string };

export type LayerReferenceInput =
{ value: (undefined | Array<number>),
layerName: (undefined | string),
layerType: (undefined | LayerDataTypeDiscriminant),
disabled: boolean,
tooltip: string,
minWidth: number };

export type LayoutGroup =
{ column: { columnWidgets: Array<WidgetHolder> } } |
{ row: { rowWidgets: Array<WidgetHolder> } } |
{ section: { name: string,
layout: Array<LayoutGroup> } };

export type LayoutKey =
{ key: string,
label: string };

export type LayoutKeysGroup =
Array<LayoutKey>;

export type LayoutMessageDiscriminant =
{ ResendActiveWidget: null } |
{ SendLayout: null } |
{ UpdateLayout: null };

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

export type LineToolMessageDiscriminant =
{ Abort: null } |
{ DragStart: null } |
{ DragStop: null } |
{ Redraw: null } |
{ UpdateOptions: null };

export type MenuBarEntry =
{ label: string,
icon: (undefined | string),
shortcut: (undefined | ActionKeys),
action: WidgetHolder,
children: MenuBarEntryChildren };

export type MenuBarEntryChildren =
Array<Array<MenuBarEntry>>;

export type MenuBarMessageDiscriminant =
{ SendLayout: null };

export type MessageDiscriminant =
{ NoOp: null } |
{ Init: null } |
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

export type NavigateToolMessageDiscriminant =
{ Abort: null } |
{ ClickZoom: null } |
{ PointerMove: null } |
{ RotateCanvasBegin: null } |
{ TransformCanvasEnd: null } |
{ TranslateCanvasBegin: null } |
{ ZoomCanvasBegin: null };

export type NavigationMessageDiscriminant =
{ DecreaseCanvasZoom: null } |
{ FitViewportToBounds: null } |
{ IncreaseCanvasZoom: null } |
{ PointerMove: null } |
{ RotateCanvasBegin: null } |
{ SetCanvasRotation: null } |
{ SetCanvasZoom: null } |
{ TransformCanvasEnd: null } |
{ TranslateCanvas: null } |
{ TranslateCanvasBegin: null } |
{ TranslateCanvasByViewportFraction: null } |
{ WheelCanvasTranslate: null } |
{ WheelCanvasZoom: null } |
{ ZoomCanvasBegin: null };

export type NewDocumentDialogMessageDiscriminant =
{ Name: null } |
{ Infinite: null } |
{ DimensionsX: null } |
{ DimensionsY: null } |
{ Submit: null };

export type NodeGraphFrameToolMessageDiscriminant =
{ Abort: null } |
{ DragStart: null } |
{ DragStop: null } |
{ Resize: null };

export type NodeGraphInput =
{ dataType: FrontendGraphDataType,
name: string };

export type NodeGraphMessageDiscriminant =
{ CloseNodeGraph: null } |
{ ConnectNodesByLink: null } |
{ Copy: null } |
{ CreateNode: null } |
{ Cut: null } |
{ DeleteNode: null } |
{ DeleteSelectedNodes: null } |
{ DisconnectNodes: null } |
{ DoubleClickNode: null } |
{ DuplicateSelectedNodes: null } |
{ ExitNestedNetwork: null } |
{ ExposeInput: null } |
{ InsertNode: null } |
{ MoveSelectedNodes: null } |
{ OpenNodeGraph: null } |
{ PasteNodes: null } |
{ SelectNodes: null } |
{ SendGraph: null } |
{ SetDrawing: null } |
{ SetInputValue: null } |
{ SetNodeInput: null } |
{ SetQualifiedInputValue: null } |
{ ShiftNode: null } |
{ ToggleHidden: null } |
{ ToggleHiddenImpl: null } |
{ TogglePreview: null } |
{ TogglePreviewImpl: null };

export type NumberInput =
{ label: string,
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
minWidth: number };

export type NumberInputIncrementBehavior =
{ Add: null } |
{ Multiply: null } |
{ Callback: null };

export type NumberInputMode =
{ Increment: null } |
{ Range: null };

export type OptionalInput =
{ checked: boolean,
disabled: boolean,
icon: string,
tooltip: string };

export type OverlaysMessageDiscriminant =
{ DispatchOperation: null } |
{ ClearAllOverlays: null } |
{ Rerender: null };

export type ParameterExposeButton =
{ exposed: boolean,
dataType: FrontendGraphDataType,
tooltip: string };

export type PathToolMessageDiscriminant =
{ Abort: null } |
{ DocumentIsDirty: null } |
{ SelectionChanged: null } |
{ Delete: null } |
{ DragStart: null } |
{ DragStop: null } |
{ InsertPoint: null } |
{ PointerMove: null };

export type PenToolMessageDiscriminant =
{ DocumentIsDirty: null } |
{ Abort: null } |
{ SelectionChanged: null } |
{ Confirm: null } |
{ DragStart: null } |
{ DragStop: null } |
{ PointerMove: null } |
{ Undo: null } |
{ UpdateOptions: null };

export type PivotAssist =
{ position: PivotPosition,
disabled: boolean };

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
{ icon: (undefined | string),
disabled: boolean,
header: string,
text: string,
tooltip: string };

export type PortfolioMessageDiscriminant =
{ MenuBar: MenuBarMessageDiscriminant } |
{ Document: DocumentMessageDiscriminant } |
{ DocumentPassMessage: null } |
{ AutoSaveActiveDocument: null } |
{ AutoSaveDocument: null } |
{ CloseActiveDocumentWithConfirmation: null } |
{ CloseAllDocuments: null } |
{ CloseDocument: null } |
{ CloseDocumentWithConfirmation: null } |
{ Copy: null } |
{ Cut: null } |
{ DeleteDocument: null } |
{ DestroyAllDocuments: null } |
{ FontLoaded: null } |
{ ImaginateCheckServerStatus: null } |
{ ImaginateSetGeneratingStatus: null } |
{ ImaginateSetImageData: null } |
{ ImaginateSetServerStatus: null } |
{ Import: null } |
{ LoadDocumentResources: null } |
{ LoadFont: null } |
{ NewDocumentWithName: null } |
{ NextDocument: null } |
{ OpenDocument: null } |
{ OpenDocumentFile: null } |
{ OpenDocumentFileWithId: null } |
{ Paste: null } |
{ PasteIntoFolder: null } |
{ PasteSerializedData: null } |
{ PrevDocument: null } |
{ ProcessNodeGraphFrame: null } |
{ SelectDocument: null } |
{ SetActiveDocument: null } |
{ SetImageBlobUrl: null } |
{ UpdateDocumentWidgets: null } |
{ UpdateOpenDocumentsList: null };

export type PreferencesDialogMessageDiscriminant =
{ Confirm: null };

export type PreferencesMessageDiscriminant =
{ Load: null } |
{ ResetToDefaults: null } |
{ ImaginateRefreshFrequency: null } |
{ ImaginateServerHostname: null };

export type PreferencesMessageHandler =
{ imaginate_server_hostname: string,
imaginate_refresh_frequency: number };

export type PropertiesPanelMessageDiscriminant =
{ CheckSelectedWasDeleted: null } |
{ CheckSelectedWasUpdated: null } |
{ ClearSelection: null } |
{ Deactivate: null } |
{ Init: null } |
{ ModifyFill: null } |
{ ModifyFont: null } |
{ ModifyName: null } |
{ ModifyPreserveAspect: null } |
{ ModifyStroke: null } |
{ ModifyText: null } |
{ ModifyTransform: null } |
{ ResendActiveProperties: null } |
{ SetActiveLayers: null } |
{ SetPivot: null } |
{ UpdateSelectedDocumentProperties: null };

export type RadioEntryData =
{ value: string,
label: string,
icon: string,
tooltip: string };

export type RadioInput =
{ entries: Array<RadioEntryData>,
disabled: boolean,
selectedIndex: number };

export type RawBuffer =
Array<number>;

export type RectangleToolMessageDiscriminant =
{ Abort: null } |
{ DragStart: null } |
{ DragStop: null } |
{ Resize: null };

export type SelectToolMessageDiscriminant =
{ Abort: null } |
{ DocumentIsDirty: null } |
{ SelectionChanged: null } |
{ Align: null } |
{ DragStart: null } |
{ DragStop: null } |
{ EditLayer: null } |
{ FlipHorizontal: null } |
{ FlipVertical: null } |
{ PointerMove: null } |
{ SetPivot: null };

export type Separator =
{ direction: SeparatorDirection,
type: SeparatorType };

export type SeparatorDirection =
{ Horizontal: null } |
{ Vertical: null };

export type SeparatorType =
{ Related: null } |
{ Unrelated: null } |
{ Section: null } |
{ List: null };

export type ShapeToolMessageDiscriminant =
{ Abort: null } |
{ DragStart: null } |
{ DragStop: null } |
{ Resize: null } |
{ UpdateOptions: null };

export type SplineToolMessageDiscriminant =
{ Abort: null } |
{ Confirm: null } |
{ DragStart: null } |
{ DragStop: null } |
{ PointerMove: null } |
{ Undo: null } |
{ UpdateOptions: null };

export type SwatchPairInput =
{ primary: Color,
secondary: Color };

export type TextAreaInput =
{ value: string,
label: (undefined | string),
disabled: boolean,
tooltip: string };

export type TextButton =
{ label: string,
icon: (undefined | string),
emphasized: boolean,
minWidth: number,
disabled: boolean,
tooltip: string };

export type TextInput =
{ value: string,
label: (undefined | string),
disabled: boolean,
tooltip: string,
centered: boolean,
minWidth: number };

export type TextLabel =
{ disabled: boolean,
bold: boolean,
italic: boolean,
tableAlign: boolean,
multiline: boolean,
minWidth: number,
tooltip: string,
value: string };

export type TextToolMessageDiscriminant =
{ Abort: null } |
{ DocumentIsDirty: null } |
{ CommitText: null } |
{ Interact: null } |
{ TextChange: null } |
{ UpdateBounds: null } |
{ UpdateOptions: null };

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
{ ActivateToolSelect: null } |
{ ActivateToolArtboard: null } |
{ ActivateToolNavigate: null } |
{ ActivateToolEyedropper: null } |
{ ActivateToolText: null } |
{ ActivateToolFill: null } |
{ ActivateToolGradient: null } |
{ ActivateToolPath: null } |
{ ActivateToolPen: null } |
{ ActivateToolFreehand: null } |
{ ActivateToolSpline: null } |
{ ActivateToolLine: null } |
{ ActivateToolRectangle: null } |
{ ActivateToolEllipse: null } |
{ ActivateToolShape: null } |
{ ActivateToolImaginate: null } |
{ ActivateToolNodeGraphFrame: null } |
{ ActivateTool: null } |
{ DeactivateTools: null } |
{ InitTools: null } |
{ RefreshToolOptions: null } |
{ ResetColors: null } |
{ SelectPrimaryColor: null } |
{ SelectRandomPrimaryColor: null } |
{ SelectSecondaryColor: null } |
{ SwapColors: null } |
{ UpdateCursor: null } |
{ UpdateHints: null };

export type TransformLayerMessageDiscriminant =
{ ApplyTransformOperation: null } |
{ BeginGrab: null } |
{ BeginRotate: null } |
{ BeginScale: null } |
{ CancelTransformOperation: null } |
{ ConstrainX: null } |
{ ConstrainY: null } |
{ PointerMove: null } |
{ TypeBackspace: null } |
{ TypeDecimalPoint: null } |
{ TypeDigit: null } |
{ TypeNegate: null };

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
{ widgetPath: Array<number>,
newValue: DiffUpdate };

export type WidgetHolder =
{ widgetId: number,
widget: Widget };

export type WorkspaceMessageDiscriminant =
{ NodeGraphToggleVisibility: null };


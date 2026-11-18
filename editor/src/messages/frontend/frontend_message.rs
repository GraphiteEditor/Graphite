use super::IconName;
use super::utility_types::{MouseCursorIcon, PersistedState, RenderStatus};
use crate::messages::app_window::app_window_message_handler::AppWindowPlatform;
use crate::messages::frontend::utility_types::{DocumentInfo, EyedropperPreviewImage, FileDialogOptions, FileFilter, RasterizedImage};
use crate::messages::input_mapper::utility_types::misc::ActionShortcut;
use crate::messages::layout::utility_types::math_expression::MathExpressionCompletions;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::node_graph::graph_ui::GraphNameField;
use crate::messages::portfolio::document::node_graph::utility_types::{ContextMenuInformation, FrontendNodeType, FrontendRemoteCursor};
use crate::messages::portfolio::document::utility_types::nodes::{LayerPanelEntry, LayerStructureEntry};
use crate::messages::portfolio::ingest::utility_types::{DataTransfer, IngestAction};
use crate::messages::portfolio::utility_types::WorkspacePanelLayout;
use crate::messages::prelude::*;
use crate::messages::tool::tool_messages::eyedropper_tool::PrimarySecondary;
use graph_craft::document::NodeId;
use graphene_std::color::SRGBA8;
use graphene_std::vector::style::FillChoice;
use std::path::PathBuf;

#[impl_message(Message, Frontend)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(derivative::Derivative, Clone, serde::Serialize, serde::Deserialize)]
#[derivative(Debug, PartialEq)]
pub enum FrontendMessage {
	// Display prefix: make the frontend show something, like a dialog
	DisplayDialog {
		title: String,
		icon: IconName,
	},
	DialogClose,
	DisplayDialogPanic {
		#[serde(rename = "panicInfo")]
		panic_info: String,
	},
	DisplayEditableTextbox {
		text: String,
		#[serde(rename = "lineHeightRatio")]
		line_height_ratio: f64,
		#[serde(rename = "fontSize")]
		font_size: f64,
		color: String,
		#[serde(rename = "fontData")]
		font_data: serde_bytes::ByteBuf,
		transform: [f64; 6],
		#[serde(rename = "maxWidth")]
		max_width: Option<f64>,
		#[serde(rename = "maxHeight")]
		max_height: Option<f64>,
		align: String,
		#[serde(rename = "alignLast")]
		align_last: String,
	},
	DisplayEditableTextboxUpdateFontData {
		#[serde(rename = "fontData")]
		font_data: serde_bytes::ByteBuf,
	},
	DisplayEditableTextboxTransform {
		transform: [f64; 6],
	},
	DisplayRemoveEditableTextbox,

	// Send prefix: Send global, static data to the frontend that is never updated
	SendUIMetadata {
		#[serde(rename = "nodeDescriptions")]
		node_descriptions: Vec<(String, String)>,
		#[serde(rename = "nodeTypes")]
		node_types: Vec<FrontendNodeType>,
	},
	SendShortcutFullscreen {
		shortcut: Option<ActionShortcut>,
		#[serde(rename = "shortcutMac")]
		shortcut_mac: Option<ActionShortcut>,
	},
	SendShortcutAltClick {
		shortcut: Option<ActionShortcut>,
	},
	SendShortcutShiftClick {
		shortcut: Option<ActionShortcut>,
	},

	// Trigger prefix: cause a frontend specific API to do something
	TriggerAboutGraphiteLocalizedCommitDate {
		#[serde(rename = "commitDate")]
		commit_date: String,
	},
	TriggerDisplayThirdPartyLicensesDialog,
	TriggerBrowse {
		options: FileDialogOptions,
		action: IngestAction,
	},
	TriggerSaveDocument {
		document_id: DocumentId,
		name: String,
		path: Option<PathBuf>,
		folder: Option<PathBuf>,
		filters: Vec<FileFilter>,
		content: serde_bytes::ByteBuf,
	},
	TriggerSaveFile {
		name: String,
		folder: Option<PathBuf>,
		filters: Vec<FileFilter>,
		content: serde_bytes::ByteBuf,
	},
	TriggerFetchAndOpenDocument {
		name: String,
		filename: String,
	},
	TriggerPersistenceReadState,
	TriggerPersistenceWriteState {
		state: PersistedState,
	},
	TriggerOpenLaunchDocuments,
	TriggerLoadPreferences,
	TriggerUpdatePreferences {
		#[cfg_attr(feature = "wasm", tsify(type = "unknown"))]
		preferences: PreferencesMessageHandler,
	},
	TriggerTextCommit,
	TriggerVisitLink {
		url: String,
	},
	TriggerClipboardRead,
	TriggerClipboardWrite {
		#[cfg_attr(feature = "wasm", tsify(type = "unknown"))]
		content: DataTransfer,
	},
	TriggerSelectionRead {
		cut: bool,
	},
	TriggerSelectionWrite {
		content: String,
	},

	// Update prefix: give the frontend a new value or state for it to use
	UpdateActiveDocument {
		#[serde(rename = "documentId")]
		document_id: DocumentId,
	},
	UpdateGradientStopColorPickerPosition {
		color: SRGBA8, // TODO: Color (without `none`) -> Color (with `none`)
		position: (f64, f64),
	},
	/// The Rust color picker handler picked a new color/gradient. The frontend `<ColorPicker />` forwards this as its `colorOrGradient` event.
	ColorPickerColorChanged {
		value: FillChoice<SRGBA8>,
	},
	/// The Rust color picker handler is starting an undo transaction. The frontend `<ColorPicker />` forwards this as its `startHistoryTransaction` event.
	ColorPickerStartHistoryTransaction,
	/// The Rust color picker handler is committing the in-flight undo transaction. The frontend `<ColorPicker />` forwards this as its `commitHistoryTransaction` event.
	ColorPickerCommitHistoryTransaction,
	/// Whether the network shown in the graph lies inside a built-in node, whose contents are read-only until made editable.
	UpdateNetworkReadOnly {
		#[serde(rename = "readOnly")]
		read_only: bool,
	},
	UpdateRemoteCursors {
		cursors: Vec<FrontendRemoteCursor>,
	},
	UpdateHistoryPanel {
		state: crate::messages::portfolio::history::utility_types::HistoryPanelState,
	},
	UpdateContextMenuInformation {
		#[serde(rename = "contextMenuInformation")]
		context_menu_information: Option<ContextMenuInformation>,
	},
	UpdateGraphViewOverlay {
		open: bool,
	},
	UpdateGraphTooltip {
		label: String,
		description: String,
	},
	UpdateGraphNameField {
		field: Option<GraphNameField>,
		opens: bool,
	},
	UpdateWorkspacePanelLayout {
		#[serde(rename = "panelLayout")]
		panel_layout: WorkspacePanelLayout,
	},
	UpdateLayout {
		#[serde(rename = "layoutTarget")]
		layout_target: LayoutTarget,
		diff: Vec<WidgetDiff>, // TODO: Align this with what's generated
	},
	/// The tokens of what a math expression widget holds, for it to color and typeset once they arrive, and the names offered to
	/// finish the one at the caret, where they were asked for.
	UpdateMathExpressionAnalysis {
		#[serde(rename = "widgetId")]
		widget_id: WidgetId,
		source: String,
		tokens: Vec<MathExpressionToken>,
		tooltips: Vec<MathExpressionTooltip>,
		completions: Option<MathExpressionCompletions>,
	},
	/// The value of math typed in a math expression widget's number popover, written as a number, or `None` where it isn't a finite real number.
	UpdateMathExpressionEvaluation {
		#[serde(rename = "widgetId")]
		widget_id: WidgetId,
		source: String,
		result: Option<String>,
	},
	UpdateDocumentArtwork {
		svg: String,
	},
	UpdateRenderStatus {
		status: RenderStatus,
	},
	/// Gives the frontend the canvas images it lacks, and names every canvas the latest render uses so it can drop the rest.
	UpdateImageData {
		image_data: Vec<RasterizedImage>,
		#[serde(rename = "canvasIdsInUse")]
		canvas_ids_in_use: Vec<u64>,
	},
	UpdateDocumentLayerDetails {
		data: LayerPanelEntry,
	},
	UpdateDocumentLayerStructure {
		#[serde(rename = "layerStructure")]
		layer_structure: Vec<LayerStructureEntry>,
	},
	UpdateDocumentRulers {
		origin: (f64, f64),
		spacing: f64,
		interval: f64,
		visible: bool,
		tilt: f64,
		flip: bool,
		#[serde(rename = "selectionQuad")]
		selection_quad: Option<[(f64, f64); 4]>,
	},
	UpdateDocumentScrollbars {
		position: (f64, f64),
		size: (f64, f64),
		multiplier: (f64, f64),
	},
	UpdateEyedropperSamplingState {
		image: Option<EyedropperPreviewImage>,
		#[serde(rename = "mousePosition")]
		mouse_position: Option<(f64, f64)>,
		#[serde(rename = "primaryColor")]
		primary_color: String,
		#[serde(rename = "secondaryColor")]
		secondary_color: String,
		#[serde(rename = "setColorChoice")]
		set_color_choice: Option<PrimarySecondary>,
	},
	UpdateMouseCursor {
		cursor: MouseCursorIcon,
	},
	UpdateNodeGraphSelection {
		selected: Vec<NodeId>,
	},
	UpdateNodeGraphTransform {
		translation: (f64, f64),
		scale: f64,
	},
	ClearNodeThumbnails {
		ids: Vec<NodeId>,
	},
	UpdateNodeThumbnailPixels {
		id: NodeId,
		width: u32,
		height: u32,
		pixels: serde_bytes::ByteBuf,
	},
	UpdateNodeThumbnailCanvas {
		id: NodeId,
		#[serde(rename = "canvasId")]
		canvas_id: u64,
	},
	UpdateOpenDocumentsList {
		#[serde(rename = "openDocuments")]
		open_documents: Vec<DocumentInfo>,
	},
	UpdatePlatform {
		platform: AppWindowPlatform,
	},
	UpdateMaximized {
		maximized: bool,
	},
	UpdateFullscreen {
		fullscreen: bool,
	},
	UpdateViewportHolePunch {
		active: bool,
	},
	#[cfg(not(target_family = "wasm"))]
	UpdateViewportPhysicalBounds {
		x: f64,
		y: f64,
		width: f64,
		height: f64,
	},
	UpdateUIScale {
		scale: f64,
	},

	/// Wakes the desktop's graph runtime so it draws the new overlays.
	#[cfg(not(target_family = "wasm"))]
	WakeNodeRuntime,

	// Window prefix: cause the application window to do something
	#[cfg(not(target_family = "wasm"))]
	WindowPointerLock,
	WindowPointerLockMove {
		position: (f64, f64),
	},
	#[cfg(not(target_family = "wasm"))]
	WindowUpdateDirectInput {
		enabled: bool,
	},
	#[cfg(not(target_family = "wasm"))]
	WindowClose,
	#[cfg(not(target_family = "wasm"))]
	WindowMinimize,
	#[cfg(not(target_family = "wasm"))]
	WindowMaximize,
	WindowFullscreen,
	#[cfg(not(target_family = "wasm"))]
	WindowDrag,
	#[cfg(not(target_family = "wasm"))]
	WindowHide,
	#[cfg(not(target_family = "wasm"))]
	WindowFocus,
	#[cfg(not(target_family = "wasm"))]
	WindowHideOthers,
	#[cfg(not(target_family = "wasm"))]
	WindowShowAll,
	#[cfg(not(target_family = "wasm"))]
	WindowRestart,
}

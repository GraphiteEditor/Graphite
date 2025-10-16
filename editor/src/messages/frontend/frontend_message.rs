use super::utility_types::{DocumentDetails, MouseCursorIcon, OpenDocument};
use crate::messages::app_window::app_window_message_handler::AppWindowPlatform;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::node_graph::utility_types::{
	BoxSelection, ContextMenuInformation, FrontendClickTargets, FrontendGraphInput, FrontendGraphOutput, FrontendNode, FrontendNodeType, Transform,
};
use crate::messages::portfolio::document::utility_types::nodes::{JsRawBuffer, LayerPanelEntry, RawBuffer};
use crate::messages::portfolio::document::utility_types::wires::{WirePath, WirePathUpdate};
use crate::messages::prelude::*;
use crate::messages::tool::utility_types::HintData;
use glam::IVec2;
use graph_craft::document::NodeId;
use graphene_std::raster::Image;
use graphene_std::raster::color::Color;
use graphene_std::text::{Font, TextAlign};
use std::path::PathBuf;

#[cfg(not(target_family = "wasm"))]
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;

#[impl_message(Message, Frontend)]
#[derive(derivative::Derivative, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[derivative(Debug, PartialEq)]
pub enum FrontendMessage {
	// Display prefix: make the frontend show something, like a dialog
	DisplayDialog {
		title: String,
		icon: String,
	},
	DisplayDialogDismiss,
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
		color: Color,
		url: String,
		transform: [f64; 6],
		#[serde(rename = "maxWidth")]
		max_width: Option<f64>,
		#[serde(rename = "maxHeight")]
		max_height: Option<f64>,
		align: TextAlign,
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

	// Trigger prefix: cause a browser API to do something
	TriggerAboutGraphiteLocalizedCommitDate {
		#[serde(rename = "commitDate")]
		commit_date: String,
	},
	TriggerDisplayThirdPartyLicensesDialog,
	TriggerSaveDocument {
		document_id: DocumentId,
		name: String,
		path: Option<PathBuf>,
		content: Vec<u8>,
	},
	TriggerSaveFile {
		name: String,
		content: Vec<u8>,
	},
	TriggerExportImage {
		svg: String,
		name: String,
		mime: String,
		size: (f64, f64),
	},
	TriggerFetchAndOpenDocument {
		name: String,
		filename: String,
	},
	TriggerFontLoad {
		font: Font,
	},
	TriggerImport,
	TriggerPersistenceRemoveDocument {
		#[serde(rename = "documentId")]
		document_id: DocumentId,
	},
	TriggerPersistenceWriteDocument {
		#[serde(rename = "documentId")]
		document_id: DocumentId,
		document: String,
		details: DocumentDetails,
	},
	TriggerLoadFirstAutoSaveDocument,
	TriggerLoadRestAutoSaveDocuments,
	TriggerOpenLaunchDocuments,
	TriggerLoadPreferences,
	TriggerOpenDocument,
	TriggerPaste,
	TriggerSavePreferences {
		preferences: PreferencesMessageHandler,
	},
	TriggerSaveActiveDocument {
		#[serde(rename = "documentId")]
		document_id: DocumentId,
	},
	TriggerTextCommit,
	TriggerTextCopy {
		#[serde(rename = "copyText")]
		copy_text: String,
	},
	TriggerVisitLink {
		url: String,
	},
	TriggerMinimizeWindow,
	TriggerMaximizeWindow,

	// Update prefix: give the frontend a new value or state for it to use
	UpdateActiveDocument {
		#[serde(rename = "documentId")]
		document_id: DocumentId,
	},
	UpdateImportsExports {
		/// If the primary import is not visible, then it is None.
		imports: Vec<Option<FrontendGraphOutput>>,
		/// If the primary export is not visible, then it is None.
		exports: Vec<Option<FrontendGraphInput>>,
		/// The primary import location.
		#[serde(rename = "importPosition")]
		import_position: IVec2,
		/// The primary export location.
		#[serde(rename = "exportPosition")]
		export_position: IVec2,
		/// The document network does not have an add import or export button.
		#[serde(rename = "addImportExport")]
		add_import_export: bool,
	},
	UpdateInSelectedNetwork {
		#[serde(rename = "inSelectedNetwork")]
		in_selected_network: bool,
	},
	UpdateBox {
		#[serde(rename = "box")]
		box_selection: Option<BoxSelection>,
	},
	UpdateContextMenuInformation {
		#[serde(rename = "contextMenuInformation")]
		context_menu_information: Option<ContextMenuInformation>,
	},
	UpdateClickTargets {
		#[serde(rename = "clickTargets")]
		click_targets: Option<FrontendClickTargets>,
	},
	UpdateGraphViewOverlay {
		open: bool,
	},
	UpdateDataPanelState {
		open: bool,
	},
	UpdatePropertiesPanelState {
		open: bool,
	},
	UpdateLayersPanelState {
		open: bool,
	},
	UpdateDataPanelLayout {
		#[serde(rename = "layoutTarget")]
		layout_target: LayoutTarget,
		diff: Vec<WidgetDiff>,
	},
	UpdateImportReorderIndex {
		#[serde(rename = "importIndex")]
		index: Option<usize>,
	},
	UpdateExportReorderIndex {
		#[serde(rename = "exportIndex")]
		index: Option<usize>,
	},
	UpdateLayerWidths {
		#[serde(rename = "layerWidths")]
		layer_widths: HashMap<NodeId, u32>,
		#[serde(rename = "chainWidths")]
		chain_widths: HashMap<NodeId, u32>,
		#[serde(rename = "hasLeftInputWire")]
		has_left_input_wire: HashMap<NodeId, bool>,
	},
	UpdateDialogButtons {
		#[serde(rename = "layoutTarget")]
		layout_target: LayoutTarget,
		diff: Vec<WidgetDiff>,
	},
	UpdateDialogColumn1 {
		#[serde(rename = "layoutTarget")]
		layout_target: LayoutTarget,
		diff: Vec<WidgetDiff>,
	},
	UpdateDialogColumn2 {
		#[serde(rename = "layoutTarget")]
		layout_target: LayoutTarget,
		diff: Vec<WidgetDiff>,
	},
	UpdateDocumentArtwork {
		svg: String,
	},
	UpdateImageData {
		image_data: Vec<(u64, Image<Color>)>,
	},
	UpdateDocumentBarLayout {
		#[serde(rename = "layoutTarget")]
		layout_target: LayoutTarget,
		diff: Vec<WidgetDiff>,
	},
	UpdateDocumentLayerDetails {
		data: LayerPanelEntry,
	},
	UpdateDocumentLayerStructure {
		#[serde(rename = "dataBuffer")]
		data_buffer: RawBuffer,
	},
	UpdateDocumentLayerStructureJs {
		#[serde(rename = "dataBuffer")]
		data_buffer: JsRawBuffer,
	},
	UpdateDocumentModeLayout {
		#[serde(rename = "layoutTarget")]
		layout_target: LayoutTarget,
		diff: Vec<WidgetDiff>,
	},
	UpdateDocumentRulers {
		origin: (f64, f64),
		spacing: f64,
		interval: f64,
		visible: bool,
	},
	UpdateDocumentScrollbars {
		position: (f64, f64),
		size: (f64, f64),
		multiplier: (f64, f64),
	},
	UpdateEyedropperSamplingState {
		#[serde(rename = "mousePosition")]
		mouse_position: Option<(f64, f64)>,
		#[serde(rename = "primaryColor")]
		primary_color: String,
		#[serde(rename = "secondaryColor")]
		secondary_color: String,
		#[serde(rename = "setColorChoice")]
		set_color_choice: Option<String>,
	},
	UpdateGraphFadeArtwork {
		percentage: f64,
	},
	UpdateInputHints {
		#[serde(rename = "hintData")]
		hint_data: HintData,
	},
	UpdateLayersPanelControlBarLeftLayout {
		#[serde(rename = "layoutTarget")]
		layout_target: LayoutTarget,
		diff: Vec<WidgetDiff>,
	},
	UpdateLayersPanelControlBarRightLayout {
		#[serde(rename = "layoutTarget")]
		layout_target: LayoutTarget,
		diff: Vec<WidgetDiff>,
	},
	UpdateLayersPanelBottomBarLayout {
		#[serde(rename = "layoutTarget")]
		layout_target: LayoutTarget,
		diff: Vec<WidgetDiff>,
	},
	UpdateMenuBarLayout {
		#[serde(rename = "layoutTarget")]
		layout_target: LayoutTarget,
		layout: Vec<MenuBarEntry>,
	},
	UpdateMouseCursor {
		cursor: MouseCursorIcon,
	},
	UpdateNodeGraphNodes {
		nodes: Vec<FrontendNode>,
	},
	UpdateVisibleNodes {
		nodes: Vec<NodeId>,
	},
	UpdateNodeGraphWires {
		wires: Vec<WirePathUpdate>,
	},
	ClearAllNodeGraphWires,
	UpdateNodeGraphControlBarLayout {
		#[serde(rename = "layoutTarget")]
		layout_target: LayoutTarget,
		diff: Vec<WidgetDiff>,
	},
	UpdateNodeGraphSelection {
		selected: Vec<NodeId>,
	},
	UpdateNodeGraphTransform {
		transform: Transform,
	},
	UpdateNodeThumbnail {
		id: NodeId,
		value: String,
	},
	UpdateOpenDocumentsList {
		#[serde(rename = "openDocuments")]
		open_documents: Vec<OpenDocument>,
	},
	UpdatePropertiesPanelLayout {
		#[serde(rename = "layoutTarget")]
		layout_target: LayoutTarget,
		diff: Vec<WidgetDiff>,
	},
	UpdateToolOptionsLayout {
		#[serde(rename = "layoutTarget")]
		layout_target: LayoutTarget,
		diff: Vec<WidgetDiff>,
	},
	UpdateToolShelfLayout {
		#[serde(rename = "layoutTarget")]
		layout_target: LayoutTarget,
		diff: Vec<WidgetDiff>,
	},
	UpdateWirePathInProgress {
		#[serde(rename = "wirePath")]
		wire_path: Option<WirePath>,
	},
	UpdateWorkingColorsLayout {
		#[serde(rename = "layoutTarget")]
		layout_target: LayoutTarget,
		diff: Vec<WidgetDiff>,
	},
	UpdatePlatform {
		platform: AppWindowPlatform,
	},
	UpdateMaximized {
		maximized: bool,
	},
	DragWindow,
	CloseWindow,
	UpdateViewportHolePunch {
		active: bool,
	},
	#[cfg(not(target_family = "wasm"))]
	RenderOverlays {
		#[serde(skip, default = "OverlayContext::default")]
		#[derivative(Debug = "ignore", PartialEq = "ignore")]
		context: OverlayContext,
	},
}

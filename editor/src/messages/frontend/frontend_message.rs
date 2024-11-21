use super::utility_types::{FrontendDocumentDetails, MouseCursorIcon};
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::node_graph::utility_types::{
	BoxSelection, ContextMenuInformation, FrontendClickTargets, FrontendGraphInput, FrontendGraphOutput, FrontendNode, FrontendNodeType, FrontendNodeWire, Transform, WirePath,
};
use crate::messages::portfolio::document::utility_types::nodes::{JsRawBuffer, LayerPanelEntry, RawBuffer};
use crate::messages::prelude::*;
use crate::messages::tool::utility_types::HintData;

use graph_craft::document::NodeId;
use graphene_core::raster::color::Color;
use graphene_core::text::Font;

#[impl_message(Message, Frontend)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
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
		#[serde(rename = "lineWidth")]
		line_width: Option<f64>,
		#[serde(rename = "fontSize")]
		font_size: f64,
		color: Color,
		url: String,
		transform: [f64; 6],
	},
	DisplayEditableTextboxTransform {
		transform: [f64; 6],
	},
	DisplayRemoveEditableTextbox,

	// Send prefix: Send global, static data to the frontend that is never updated
	SendUIMetadata {
		#[serde(rename = "inputTypeDescriptions")]
		input_type_descriptions: Vec<(String, String)>,
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
	TriggerCopyToClipboardBlobUrl {
		#[serde(rename = "blobUrl")]
		blob_url: String,
	},
	TriggerDelayedZoomCanvasToFitAll,
	TriggerDownloadBlobUrl {
		#[serde(rename = "layerName")]
		layer_name: String,
		#[serde(rename = "blobUrl")]
		blob_url: String,
	},
	TriggerDownloadImage {
		svg: String,
		name: String,
		mime: String,
		size: (f64, f64),
	},
	TriggerDownloadTextFile {
		document: String,
		name: String,
	},
	TriggerFetchAndOpenDocument {
		name: String,
		filename: String,
	},
	TriggerFontLoad {
		font: Font,
	},
	TriggerImport,
	TriggerIndexedDbRemoveDocument {
		#[serde(rename = "documentId")]
		document_id: DocumentId,
	},
	TriggerIndexedDbWriteDocument {
		document: String,
		details: FrontendDocumentDetails,
	},
	TriggerLoadAutoSaveDocuments,
	TriggerLoadPreferences,
	TriggerOpenDocument,
	TriggerPaste,
	TriggerRevokeBlobUrl {
		url: String,
	},
	TriggerSavePreferences {
		preferences: PreferencesMessageHandler,
	},
	TriggerTextCommit,
	TriggerTextCopy {
		#[serde(rename = "copyText")]
		copy_text: String,
	},
	// TODO: Eventually remove this (probably starting late 2024)
	TriggerUpgradeDocumentToVectorManipulationFormat {
		#[serde(rename = "documentId")]
		document_id: DocumentId,
		#[serde(rename = "documentName")]
		document_name: String,
		#[serde(rename = "documentIsAutoSaved")]
		document_is_auto_saved: bool,
		#[serde(rename = "documentIsSaved")]
		document_is_saved: bool,
		#[serde(rename = "documentSerializedContent")]
		document_serialized_content: String,
	},
	TriggerVisitLink {
		url: String,
	},

	// Update prefix: give the frontend a new value or state for it to use
	UpdateActiveDocument {
		#[serde(rename = "documentId")]
		document_id: DocumentId,
	},
	UpdateImportsExports {
		imports: Vec<(FrontendGraphOutput, i32, i32)>,
		exports: Vec<(FrontendGraphInput, i32, i32)>,
		#[serde(rename = "addImport")]
		add_import: Option<(i32, i32)>,
		#[serde(rename = "addExport")]
		add_export: Option<(i32, i32)>,
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
	UpdateLayersPanelOptionsLayout {
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
	UpdateNodeGraph {
		nodes: Vec<FrontendNode>,
		wires: Vec<FrontendNodeWire>,
	},
	UpdateNodeGraphBarLayout {
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
		open_documents: Vec<FrontendDocumentDetails>,
	},
	UpdatePropertyPanelSectionsLayout {
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
	UpdateZoomWithScroll {
		#[serde(rename = "zoomWithScroll")]
		zoom_with_scroll: bool,
	},
}

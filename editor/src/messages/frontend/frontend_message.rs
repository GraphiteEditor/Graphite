use super::utility_types::{FrontendDocumentDetails, MouseCursorIcon};
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::node_graph::{FrontendNode, FrontendNodeLink, FrontendNodeType};
use crate::messages::portfolio::document::utility_types::nodes::{JsRawBuffer, LayerPanelEntry, RawBuffer};
use crate::messages::prelude::*;
use crate::messages::tool::utility_types::HintData;

use graph_craft::document::NodeId;
use graphene_core::raster::color::Color;
use graphene_core::text::Font;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, Frontend)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
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

	// Trigger prefix: cause a browser API to do something
	TriggerAboutGraphiteLocalizedCommitDate {
		#[serde(rename = "commitDate")]
		commit_date: String,
	},
	TriggerCopyToClipboardBlobUrl {
		#[serde(rename = "blobUrl")]
		blob_url: String,
	},
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
		#[serde(rename = "isDefault")]
		is_default: bool,
	},
	TriggerGraphViewOverlay {
		open: bool,
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
	TriggerRefreshBoundsOfViewports,
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
	TriggerViewportResize,
	TriggerVisitLink {
		url: String,
	},

	// Update prefix: give the frontend a new value or state for it to use
	UpdateActiveDocument {
		#[serde(rename = "documentId")]
		document_id: DocumentId,
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
		links: Vec<FrontendNodeLink>,
	},
	UpdateNodeGraphBarLayout {
		#[serde(rename = "layoutTarget")]
		layout_target: LayoutTarget,
		diff: Vec<WidgetDiff>,
	},
	UpdateNodeGraphSelection {
		selected: Vec<NodeId>,
	},
	UpdateNodeThumbnail {
		id: NodeId,
		value: String,
	},
	UpdateNodeTypes {
		#[serde(rename = "nodeTypes")]
		node_types: Vec<FrontendNodeType>,
	},
	UpdateOpenDocumentsList {
		#[serde(rename = "openDocuments")]
		open_documents: Vec<FrontendDocumentDetails>,
	},
	UpdatePropertyPanelOptionsLayout {
		#[serde(rename = "layoutTarget")]
		layout_target: LayoutTarget,
		diff: Vec<WidgetDiff>,
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

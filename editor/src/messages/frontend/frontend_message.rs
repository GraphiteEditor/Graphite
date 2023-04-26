use super::utility_types::{FrontendDocumentDetails, FrontendImageData, MouseCursorIcon};
use crate::messages::layout::utility_types::layout_widget::WidgetDiff;
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::layout::utility_types::widgets::menu_widgets::MenuBarEntry;
use crate::messages::portfolio::document::node_graph::{FrontendNode, FrontendNodeLink, FrontendNodeType};
use crate::messages::portfolio::document::utility_types::layer_panel::{JsRawBuffer, LayerPanelEntry, RawBuffer};
use crate::messages::prelude::*;
use crate::messages::tool::utility_types::HintData;

use document_legacy::layers::text_layer::Font;
use document_legacy::LayerId;
use graph_craft::document::NodeId;
use graph_craft::imaginate_input::*;
use graphene_core::raster::color::Color;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, Frontend)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize, specta::Type)]
pub enum FrontendMessage {
	// Display prefix: make the frontend show something, like a dialog
	DisplayDialog {
		icon: String,
	},
	DisplayDialogDismiss,
	DisplayDialogPanic {
		#[serde(rename = "panicInfo")]
		panic_info: String,
		header: String,
		description: String,
	},
	DisplayEditableTextbox {
		text: String,
		#[serde(rename = "lineWidth")]
		line_width: Option<f64>,
		#[serde(rename = "fontSize")]
		font_size: f64,
		color: Color,
	},
	DisplayRemoveEditableTextbox,

	// Trigger prefix: cause a browser API to do something
	TriggerAboutGraphiteLocalizedCommitDate {
		#[serde(rename = "commitDate")]
		commit_date: String,
	},
	TriggerFileDownload {
		document: String,
		name: String,
	},
	TriggerFontLoad {
		font: Font,
		#[serde(rename = "isDefault")]
		is_default: bool,
	},
	TriggerImaginateCheckServerStatus {
		hostname: String,
	},
	TriggerImaginateGenerate {
		parameters: Box<ImaginateGenerationParameters>,
		#[serde(rename = "baseImage")]
		base_image: Option<Box<ImaginateBaseImage>>,
		#[serde(rename = "maskImage")]
		mask_image: Option<Box<ImaginateMaskImage>>,
		#[serde(rename = "maskPaintMode")]
		mask_paint_mode: ImaginateMaskPaintMode,
		#[serde(rename = "maskBlurPx")]
		mask_blur_px: u32,
		#[serde(rename = "maskFillContent")]
		imaginate_mask_starting_fill: ImaginateMaskStartingFill,
		hostname: String,
		#[serde(rename = "refreshFrequency")]
		refresh_frequency: f64,
		#[serde(rename = "documentId")]
		document_id: u64,
		#[serde(rename = "layerPath")]
		layer_path: Vec<LayerId>,
		#[serde(rename = "nodePath")]
		node_path: Vec<NodeId>,
	},
	TriggerImaginateTerminate {
		#[serde(rename = "documentId")]
		document_id: u64,
		#[serde(rename = "layerPath")]
		layer_path: Vec<LayerId>,
		#[serde(rename = "nodePath")]
		node_path: Vec<NodeId>,
		hostname: String,
	},
	TriggerImport,
	TriggerIndexedDbRemoveDocument {
		#[serde(rename = "documentId")]
		document_id: u64,
	},
	TriggerIndexedDbWriteDocument {
		document: String,
		details: FrontendDocumentDetails,
		version: String,
	},
	TriggerLoadAutoSaveDocuments,
	TriggerLoadPreferences,
	TriggerNodeGraphFrameGenerate {
		#[serde(rename = "documentId")]
		document_id: u64,
		#[serde(rename = "layerPath")]
		layer_path: Vec<LayerId>,
		svg: String,
		size: glam::DVec2,
		#[serde(rename = "imaginateNode")]
		imaginate_node: Option<Vec<NodeId>>,
	},
	TriggerOpenDocument,
	TriggerPaste,
	TriggerRasterDownload {
		svg: String,
		name: String,
		mime: String,
		size: (f64, f64),
	},
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
		document_id: u64,
	},
	UpdateDialogDetails {
		#[serde(rename = "layoutTarget")]
		layout_target: LayoutTarget,
		diff: Vec<WidgetDiff>,
	},
	UpdateDocumentArtboards {
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
	UpdateDocumentLayerTreeStructure {
		#[serde(rename = "dataBuffer")]
		data_buffer: RawBuffer,
	},
	UpdateDocumentLayerTreeStructureJs {
		#[serde(rename = "dataBuffer")]
		data_buffer: JsRawBuffer,
	},
	UpdateDocumentModeLayout {
		#[serde(rename = "layoutTarget")]
		layout_target: LayoutTarget,
		diff: Vec<WidgetDiff>,
	},
	UpdateDocumentOverlays {
		svg: String,
	},
	UpdateDocumentRulers {
		origin: (f64, f64),
		spacing: f64,
		interval: f64,
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
	UpdateImageData {
		#[serde(rename = "documentId")]
		document_id: u64,
		#[serde(rename = "imageData")]
		image_data: Vec<FrontendImageData>,
	},
	UpdateInputHints {
		#[serde(rename = "hintData")]
		hint_data: HintData,
	},
	UpdateLayerTreeOptionsLayout {
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
	UpdateNodeGraphDocument {
		document: document_legacy::document::Document,
	},
	UpdateNodeGraphSelection {
		selected: Vec<NodeId>,
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

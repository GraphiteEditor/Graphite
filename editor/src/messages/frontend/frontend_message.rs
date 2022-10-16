use super::utility_types::{FrontendDocumentDetails, FrontendImageData, MouseCursorIcon};
use crate::messages::layout::utility_types::layout_widget::SubLayout;
use crate::messages::layout::utility_types::misc::LayoutTarget;
use crate::messages::layout::utility_types::widgets::menu_widgets::MenuBarEntry;
use crate::messages::portfolio::document::utility_types::layer_panel::{LayerPanelEntry, RawBuffer};
use crate::messages::prelude::*;
use crate::messages::tool::utility_types::HintData;

use graphene::color::Color;
use graphene::layers::text_layer::Font;
use graphene::LayerId;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, Frontend)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
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
	TriggerAiArtist {
		svg: Option<String>,
		#[serde(rename = "rasterizeSize")]
		rasterize_size: Option<(f64, f64)>,
		#[serde(rename = "documentId")]
		document_id: u64,
		#[serde(rename = "layerPath")]
		layer_path: Vec<LayerId>,
		hostname: String,
		#[serde(rename = "refreshFrequency")]
		refresh_frequency: f64,
		prompt: String,
		#[serde(rename = "negativePrompt")]
		negative_prompt: String,
		resolution: (u64, u64),
		seed: u64,
		samples: u32,
		#[serde(rename = "cfgScale")]
		cfg_scale: f64,
		#[serde(rename = "denoisingStrength")]
		denoising_strength: Option<f64>,
		#[serde(rename = "restoreFaces")]
		restore_faces: bool,
		tiling: bool,
	},
	TriggerAiArtistCheckServerStatus {
		hostname: String,
	},
	TriggerAiArtistTerminate {
		#[serde(rename = "documentId")]
		document_id: u64,
		#[serde(rename = "layerPath")]
		layer_path: Vec<LayerId>,
		hostname: String,
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
		layout: SubLayout,
	},
	UpdateDocumentArtboards {
		svg: String,
	},
	UpdateDocumentArtwork {
		svg: String,
	},
	UpdateDocumentBarLayout {
		#[serde(rename = "layoutTarget")]
		layout_target: LayoutTarget,
		layout: SubLayout,
	},
	UpdateDocumentLayerDetails {
		data: LayerPanelEntry,
	},
	UpdateDocumentLayerTreeStructure {
		#[serde(rename = "dataBuffer")]
		data_buffer: RawBuffer,
	},
	UpdateDocumentModeLayout {
		#[serde(rename = "layoutTarget")]
		layout_target: LayoutTarget,
		layout: SubLayout,
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
	UpdateImageData {
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
		layout: SubLayout,
	},
	UpdateMenuBarLayout {
		#[serde(rename = "layoutTarget")]
		layout_target: LayoutTarget,
		layout: Vec<MenuBarEntry>,
	},
	UpdateMouseCursor {
		cursor: MouseCursorIcon,
	},
	UpdateNodeGraphVisibility {
		visible: bool,
	},
	UpdateOpenDocumentsList {
		#[serde(rename = "openDocuments")]
		open_documents: Vec<FrontendDocumentDetails>,
	},
	UpdatePropertyPanelOptionsLayout {
		#[serde(rename = "layoutTarget")]
		layout_target: LayoutTarget,
		layout: SubLayout,
	},
	UpdatePropertyPanelSectionsLayout {
		#[serde(rename = "layoutTarget")]
		layout_target: LayoutTarget,
		layout: SubLayout,
	},
	UpdateToolOptionsLayout {
		#[serde(rename = "layoutTarget")]
		layout_target: LayoutTarget,
		layout: SubLayout,
	},
	UpdateToolShelfLayout {
		#[serde(rename = "layoutTarget")]
		layout_target: LayoutTarget,
		layout: SubLayout,
	},
	UpdateWorkingColorsLayout {
		#[serde(rename = "layoutTarget")]
		layout_target: LayoutTarget,
		layout: SubLayout,
	},
}

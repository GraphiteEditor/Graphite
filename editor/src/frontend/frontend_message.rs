use super::utility_types::{FrontendDocumentDetails, FrontendImageData, MouseCursorIcon};
use crate::document::layer_panel::{LayerPanelEntry, RawBuffer};
use crate::layout::layout_message::LayoutTarget;
use crate::layout::widgets::{MenuColumn, SubLayout};
use crate::message_prelude::*;
use crate::misc::HintData;
use crate::Color;

use graphene::layers::text_layer::Font;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, Frontend)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum FrontendMessage {
	// Display prefix: make the frontend show something, like a dialog
	DisplayDialog { icon: String },
	DisplayDialogDismiss,
	DisplayDialogPanic { panic_info: String, header: String, description: String },
	DisplayEditableTextbox { text: String, line_width: Option<f64>, font_size: f64, color: Color },
	DisplayRemoveEditableTextbox,

	// Trigger prefix: cause a browser API to do something
	TriggerAboutGraphiteLocalizedCommitDate { commit_date: String },
	TriggerFileDownload { document: String, name: String },
	TriggerFontLoad { font: Font, is_default: bool },
	TriggerImport,
	TriggerIndexedDbRemoveDocument { document_id: u64 },
	TriggerIndexedDbWriteDocument { document: String, details: FrontendDocumentDetails, version: String },
	TriggerOpenDocument,
	TriggerPaste,
	TriggerRasterDownload { document: String, name: String, mime: String, size: (f64, f64) },
	TriggerRefreshBoundsOfViewports,
	TriggerTextCommit,
	TriggerTextCopy { copy_text: String },
	TriggerViewportResize,
	TriggerVisitLink { url: String },

	// Update prefix: give the frontend a new value or state for it to use
	UpdateActiveDocument { document_id: u64 },
	UpdateDialogDetails { layout_target: LayoutTarget, layout: SubLayout },
	UpdateDocumentArtboards { svg: String },
	UpdateDocumentArtwork { svg: String },
	UpdateDocumentBarLayout { layout_target: LayoutTarget, layout: SubLayout },
	UpdateDocumentLayerDetails { data: LayerPanelEntry },
	UpdateDocumentLayerTreeStructure { data_buffer: RawBuffer },
	UpdateDocumentModeLayout { layout_target: LayoutTarget, layout: SubLayout },
	UpdateDocumentOverlays { svg: String },
	UpdateDocumentRulers { origin: (f64, f64), spacing: f64, interval: f64 },
	UpdateDocumentScrollbars { position: (f64, f64), size: (f64, f64), multiplier: (f64, f64) },
	UpdateImageData { image_data: Vec<FrontendImageData> },
	UpdateInputHints { hint_data: HintData },
	UpdateLayerTreeOptionsLayout { layout_target: LayoutTarget, layout: SubLayout },
	UpdateMenuBarLayout { layout_target: LayoutTarget, layout: Vec<MenuColumn> },
	UpdateMouseCursor { cursor: MouseCursorIcon },
	UpdateNodeGraphVisibility { visible: bool },
	UpdateOpenDocumentsList { open_documents: Vec<FrontendDocumentDetails> },
	UpdatePropertyPanelOptionsLayout { layout_target: LayoutTarget, layout: SubLayout },
	UpdatePropertyPanelSectionsLayout { layout_target: LayoutTarget, layout: SubLayout },
	UpdateToolOptionsLayout { layout_target: LayoutTarget, layout: SubLayout },
	UpdateToolShelfLayout { layout_target: LayoutTarget, layout: SubLayout },
	UpdateWorkingColorsLayout { layout_target: LayoutTarget, layout: SubLayout },
}

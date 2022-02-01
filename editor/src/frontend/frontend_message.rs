use super::utility_types::{FrontendDocumentDetails, MouseCursorIcon};
use crate::document::layer_panel::{LayerPanelEntry, RawBuffer};
use crate::layout::layout_message::LayoutTarget;
use crate::layout::widgets::SubLayout;
use crate::message_prelude::*;
use crate::misc::HintData;
use crate::Color;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, Frontend)]
#[derive(PartialEq, Clone, Deserialize, Serialize, Debug)]
pub enum FrontendMessage {
	// Display prefix: make the frontend show something, like a dialog
	DisplayConfirmationToCloseAllDocuments,
	DisplayConfirmationToCloseDocument { document_id: u64 },
	DisplayDialogAboutGraphite,
	DisplayDialogComingSoon { issue: Option<i32> },
	DisplayDialogError { title: String, description: String },
	DisplayDialogPanic { panic_info: String, title: String, description: String },
	DisplayDocumentLayerTreeStructure { data_buffer: RawBuffer },
	DisplayEditableTextbox { text: String, line_width: Option<f64>, font_size: f64 },
	DisplayRemoveEditableTextbox,

	// Trigger prefix: cause a browser API to do something
	TriggerFileDownload { document: String, name: String },
	TriggerFileUpload,
	TriggerIndexedDbRemoveDocument { document_id: u64 },
	TriggerIndexedDbWriteDocument { document: String, details: FrontendDocumentDetails, version: String },
	TriggerTextCommit,

	// Update prefix: give the frontend a new value or state for it to use
	UpdateActiveDocument { document_id: u64 },
	UpdateActiveTool { tool_name: String },
	UpdateCanvasRotation { angle_radians: f64 },
	UpdateCanvasZoom { factor: f64 },
	UpdateDocumentArtboards { svg: String },
	UpdateDocumentArtwork { svg: String },
	UpdateDocumentBarLayout { layout_target: LayoutTarget, layout: SubLayout },
	UpdateDocumentLayer { data: LayerPanelEntry },
	UpdateDocumentOverlays { svg: String },
	UpdateDocumentRulers { origin: (f64, f64), spacing: f64, interval: f64 },
	UpdateDocumentScrollbars { position: (f64, f64), size: (f64, f64), multiplier: (f64, f64) },
	UpdateInputHints { hint_data: HintData },
	UpdateMouseCursor { cursor: MouseCursorIcon },
	UpdateOpenDocumentsList { open_documents: Vec<FrontendDocumentDetails> },
	UpdateToolOptionsLayout { layout_target: LayoutTarget, layout: SubLayout },
	UpdateWorkingColors { primary: Color, secondary: Color },
}

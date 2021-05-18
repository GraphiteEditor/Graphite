use super::message::prelude::*;
use document_core::{response::LayerPanelEntry, DocumentResponse, LayerId};
use graphite_proc_macros::*;
use serde::{Deserialize, Serialize};

#[impl_message(Message, Frontend)]
#[derive(PartialEq, Clone, Deserialize, Serialize, Debug)]
pub enum FrontendMessage {
	CollapseFolder { path: Vec<LayerId> },
	ExpandFolder { path: Vec<LayerId>, children: Vec<LayerPanelEntry> },
	SetActiveTool { tool_name: String },
	UpdateCanvas { document: String },
	EnableTextInput,
	DisableTextInput,
}

impl From<DocumentResponse> for Message {
	fn from(response: DocumentResponse) -> Self {
		let frontend: FrontendMessage = response.into();
		frontend.into()
	}
}
impl From<DocumentResponse> for FrontendMessage {
	fn from(response: DocumentResponse) -> Self {
		match response {
			DocumentResponse::ExpandFolder { path, children } => Self::ExpandFolder { path, children },
			DocumentResponse::CollapseFolder { path } => Self::CollapseFolder { path },
			_ => unimplemented!("The frontend does not handle {:?}", response),
		}
	}
}

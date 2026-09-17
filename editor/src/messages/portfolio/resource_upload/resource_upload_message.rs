use super::utility_types::UploadTarget;
use crate::messages::prelude::*;
use std::sync::Arc;

#[impl_message(Message, PortfolioMessage, ResourceUpload)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ResourceUploadMessage {
	/// Opens a file dialog filtered to what the target accepts, whose pick arrives as `ReceiveUpload`
	RequestUpload { target: UploadTarget },
	/// The file picked for the pending `RequestUpload`
	ReceiveUpload { name: Option<String>, data: Arc<[u8]> },
	/// Stores a file as an embedded resource of the active document and hands it to its target
	Upload { name: Option<String>, data: Arc<[u8]>, target: UploadTarget },
}

use crate::messages::prelude::*;

#[impl_message(Message, DialogMessage, DocumentSetupDialog)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum DocumentSetupDialogMessage {
	Resolution { pixels_per_inch: f64 },

	Submit,
}

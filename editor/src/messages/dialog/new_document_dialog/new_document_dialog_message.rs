use crate::messages::prelude::*;

#[impl_message(Message, DialogMessage, NewDocumentDialog)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum NewDocumentDialogMessage {
	Name(String),
	Infinite(bool),
	DimensionsX(f64),
	DimensionsY(f64),

	Submit,
}

use crate::messages::prelude::*;

#[impl_message(Message, DialogMessage, NewDocumentDialog)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum NewDocumentDialogMessage {
	Name { name: String },
	Infinite { infinite: bool },
	DimensionsX { width: f64 },
	DimensionsY { height: f64 },

	Submit,
}

use crate::messages::prelude::*;

#[impl_message(Message, DocumentMessage, Palette)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum PaletteMessage {
	Refresh,
	Upload,
}

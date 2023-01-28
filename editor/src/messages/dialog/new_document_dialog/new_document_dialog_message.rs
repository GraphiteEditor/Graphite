use crate::messages::prelude::*;

use serde::{Deserialize, Serialize};

#[impl_message(Message, DialogMessage, NewDocumentDialog)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum NewDocumentDialogMessage {
	Name(String),
	Infinite(bool),
	DimensionsX(f64),
	DimensionsY(f64),

	Submit,
}

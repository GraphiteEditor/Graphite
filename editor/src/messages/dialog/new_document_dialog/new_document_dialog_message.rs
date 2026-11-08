use crate::messages::prelude::*;
use graphene_std::units::TypedLength;

#[impl_message(Message, DialogMessage, NewDocumentDialog)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum NewDocumentDialogMessage {
	Name { name: String },
	Infinite { infinite: bool },
	DimensionsX { width: TypedLength },
	DimensionsY { height: TypedLength },
	Resolution { pixels_per_inch: f64 },

	Submit,
}

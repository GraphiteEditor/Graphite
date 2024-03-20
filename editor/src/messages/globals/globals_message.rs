use crate::messages::portfolio::utility_types::Platform;
use crate::messages::prelude::*;

#[impl_message(Message, Globals)]
#[derive(PartialEq, Eq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum GlobalsMessage {
	SetPlatform { platform: Platform },
}

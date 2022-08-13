use crate::messages::portfolio::document::utility_types::misc::Platform;
use crate::messages::prelude::*;

use serde::{Deserialize, Serialize};

#[impl_message(Message, Globals)]
#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize)]
pub enum GlobalsMessage {
	SetPlatform { platform: Platform },
}

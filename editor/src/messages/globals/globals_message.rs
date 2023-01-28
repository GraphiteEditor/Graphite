use crate::messages::portfolio::utility_types::Platform;
use crate::messages::prelude::*;

use serde::{Deserialize, Serialize};

#[impl_message(Message, Globals)]
#[derive(PartialEq, Eq, Clone, Debug, Serialize, Deserialize)]
pub enum GlobalsMessage {
	SetPlatform { platform: Platform },
}

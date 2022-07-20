use crate::message_prelude::*;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, Global)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize)]
pub enum GlobalMessage {
	LogMaxLevelDebug,
	LogMaxLevelInfo,
	LogMaxLevelTrace,

	TraceMessageContents,
	TraceMessageDiscriminants,
}

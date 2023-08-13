use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, Layout)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum LayoutMessage {
	ResendActiveWidget { layout_target: LayoutTarget, dirty_id: u64 },
	SendLayout { layout: Layout, layout_target: LayoutTarget },
	UpdateLayout { layout_target: LayoutTarget, widget_id: u64, value: serde_json::Value },
}

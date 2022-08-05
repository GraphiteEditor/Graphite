use super::utility_types::misc::LayoutTarget;
use crate::messages::layout::utility_types::layout_widget::Layout;
use crate::messages::prelude::*;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, Layout)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum LayoutMessage {
	RefreshLayout { layout_target: LayoutTarget },
	SendLayout { layout: Layout, layout_target: LayoutTarget },
	UpdateLayout { layout_target: LayoutTarget, widget_id: u64, value: serde_json::Value },
}

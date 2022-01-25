use crate::message_prelude::*;

use serde::{Deserialize, Serialize};

use super::widgets::WidgetLayout;

#[remain::sorted]
#[impl_message(Message, Layout)]
#[derive(PartialEq, Clone, Deserialize, Serialize, Debug)]
pub enum LayoutMessage {
	SendLayout { layout: WidgetLayout, layout_target: LayoutTarget },
	UpdateLayout { layout_target: LayoutTarget, widget_id: u64, value: serde_json::Value },
	WidgetDefaultMarker,
}

#[remain::sorted]
#[derive(PartialEq, Clone, Deserialize, Serialize, Debug, Hash, Eq)]
pub enum LayoutTarget {
	ToolOptions,
}

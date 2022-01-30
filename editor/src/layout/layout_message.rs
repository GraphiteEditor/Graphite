use crate::message_prelude::*;

use serde::{Deserialize, Serialize};

use super::widgets::WidgetLayout;

#[remain::sorted]
#[impl_message(Message, Layout)]
#[derive(PartialEq, Clone, Deserialize, Serialize, Debug)]
pub enum LayoutMessage {
	SendLayout { layout: WidgetLayout, layout_target: LayoutTarget },
	UpdateLayout { layout_target: LayoutTarget, widget_id: u64, value: serde_json::Value },
}

#[remain::sorted]
#[derive(PartialEq, Clone, Deserialize, Serialize, Debug, Hash, Eq, Copy)]
#[repr(u8)]
pub enum LayoutTarget {
	DocumentBar,
	ToolOptions,

	// KEEP THIS ENUM LAST
	// This is a marker that is used to define an array that is used to hold widgets
	#[remain::unsorted]
	LayoutTargetLength,
}

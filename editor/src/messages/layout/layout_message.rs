use super::utility_types::misc::LayoutTarget;
use crate::messages::layout::utility_types::layout_widget::Layout;
use crate::messages::prelude::*;
use document_legacy::LayerId;

use serde::{Deserialize, Serialize};

#[remain::sorted]
#[impl_message(Message, Layout)]
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum LayoutMessage {
	ResendActiveWidget {
		layout_target: LayoutTarget,
		dirty_id: LayerId,
	},
	SendLayout {
		layout: Layout,
		layout_target: LayoutTarget,
	},
	UpdateLayout {
		layout_target: LayoutTarget,
		widget_id: LayerId,
		value: serde_json::Value,
	},
}

use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

#[impl_message(Message, Layout)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum LayoutMessage {
	ResendActiveWidget {
		layout_target: LayoutTarget,
		widget_id: WidgetId,
	},
	SendLayout {
		layout: Layout,
		layout_target: LayoutTarget,
	},
	WidgetValueCommit {
		layout_target: LayoutTarget,
		widget_id: WidgetId,
		value: serde_json::Value,
	},
	WidgetValueUpdate {
		layout_target: LayoutTarget,
		widget_id: WidgetId,
		value: serde_json::Value,
	},
}

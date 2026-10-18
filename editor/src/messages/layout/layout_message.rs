use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

#[impl_message(Message, Layout)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum LayoutMessage {
	ResendActiveWidget {
		layout_target: LayoutTarget,
		widget_id: WidgetId,
	},
	ResendAllLayouts,
	SendLayout {
		layout: Layout,
		layout_target: LayoutTarget,
	},
	DestroyLayout {
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
	WidgetValueDragDrop {
		layout_target: LayoutTarget,
		widget_id: WidgetId,
	},
	WidgetValueFileDrop {
		layout_target: LayoutTarget,
		widget_id: WidgetId,
		file: DroppedFile,
	},
	/// Tokenizes what a math expression widget holds as it is typed, replying with what the frontend colors and typesets, and with
	/// the names that finish the one at the caret, where the caret is given. Unless they're `asked` for, as by a shortcut, names are
	/// offered only once part of one is typed.
	AnalyzeMathExpression {
		layout_target: LayoutTarget,
		widget_id: WidgetId,
		source: String,
		caret: Option<u32>,
		asked: bool,
	},
	/// Evaluates math typed in a math expression widget's number popover, replying with its value written as the number to take its place.
	EvaluateMathExpression {
		widget_id: WidgetId,
		source: String,
	},
}

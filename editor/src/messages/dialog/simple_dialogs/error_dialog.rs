use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, PropertyHolder, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::messages::layout::utility_types::widgets::button_widgets::TextButton;
use crate::messages::layout::utility_types::widgets::label_widgets::TextLabel;
use crate::messages::prelude::*;

/// A dialog to notify users of a non-fatal error.
pub struct ErrorDialog {
	pub title: String,
	pub description: String,
}

impl PropertyHolder for ErrorDialog {
	fn properties(&self) -> Layout {
		Layout::WidgetLayout(WidgetLayout::new(vec![
			LayoutGroup::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: self.title.clone(),
					bold: true,
					..Default::default()
				}))],
			},
			LayoutGroup::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: self.description.clone(),
					multiline: true,
					..Default::default()
				}))],
			},
			LayoutGroup::Row {
				widgets: vec![WidgetHolder::new(Widget::TextButton(TextButton {
					label: "OK".to_string(),
					emphasized: true,
					min_width: 96,
					on_update: widget_callback!(|_| FrontendMessage::DisplayDialogDismiss.into()),
					..Default::default()
				}))],
			},
		]))
	}
}

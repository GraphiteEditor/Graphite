use crate::{layout::widgets::*, message_prelude::FrontendMessage};

/// A dialog to notify users of a non-fatal error.
pub struct Error {
	pub title: String,
	pub description: String,
}

impl PropertyHolder for Error {
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
					on_update: WidgetCallback::new(|_| FrontendMessage::DisplayDialogDismiss.into()),
					..Default::default()
				}))],
			},
		]))
	}
}

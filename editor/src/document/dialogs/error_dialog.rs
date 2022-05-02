use crate::{layout::widgets::*, message_prelude::FrontendMessage};

pub struct Error {
	pub description: String,
}

impl PropertyHolder for Error {
	fn properties(&self) -> WidgetLayout {
		WidgetLayout::new(vec![
			LayoutRow::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: self.description.clone(),
					preserve_whitespace: true,
					..Default::default()
				}))],
			},
			LayoutRow::Row {
				widgets: vec![WidgetHolder::new(Widget::Separator(Separator {
					direction: SeparatorDirection::Vertical,
					separator_type: SeparatorType::Unrelated,
				}))],
			},
			LayoutRow::Row {
				widgets: vec![WidgetHolder::new(Widget::TextButton(TextButton {
					label: "OK".to_string(),
					emphasized: true,
					min_width: 96,
					on_update: WidgetCallback::new(|_| FrontendMessage::TriggerDismissDialog.into()),
					..Default::default()
				}))],
			},
		])
	}
}

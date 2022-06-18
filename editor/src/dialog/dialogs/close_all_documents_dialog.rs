use crate::layout::widgets::*;
use crate::message_prelude::{DialogMessage, FrontendMessage, PortfolioMessage};

/// A dialog for confirming the closing of all documents viewable via `file -> close all` in the menu bar.
pub struct CloseAllDocuments;

impl PropertyHolder for CloseAllDocuments {
	fn properties(&self) -> Layout {
		let button_widgets = vec![
			WidgetHolder::new(Widget::TextButton(TextButton {
				label: "Discard All".to_string(),
				min_width: 96,
				on_update: WidgetCallback::new(|_| {
					DialogMessage::CloseDialogAndThen {
						followup: Box::new(PortfolioMessage::CloseAllDocuments.into()),
					}
					.into()
				}),
				..Default::default()
			})),
			WidgetHolder::new(Widget::TextButton(TextButton {
				label: "Cancel".to_string(),
				min_width: 96,
				on_update: WidgetCallback::new(|_| FrontendMessage::DisplayDialogDismiss.into()),
				..Default::default()
			})),
		];

		Layout::WidgetLayout(WidgetLayout::new(vec![
			LayoutRow::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: "Close all documents?".to_string(),
					bold: true,
					..Default::default()
				}))],
			},
			LayoutRow::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: "Unsaved work will be lost!".to_string(),
					multiline: true,
					..Default::default()
				}))],
			},
			LayoutRow::Row { widgets: button_widgets },
		]))
	}
}

use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, PropertyHolder, Widget, WidgetHolder, WidgetLayout};
use crate::messages::layout::utility_types::widgets::button_widgets::TextButton;
use crate::messages::layout::utility_types::widgets::label_widgets::TextLabel;
use crate::messages::prelude::*;

/// A dialog for confirming the closing of all documents viewable via `file -> close all` in the menu bar.
pub struct CloseAllDocumentsDialog;

impl PropertyHolder for CloseAllDocumentsDialog {
	fn properties(&self) -> Layout {
		let button_widgets = vec![
			WidgetHolder::new(Widget::TextButton(TextButton {
				label: "Discard All".to_string(),
				min_width: 96,
				on_update: widget_callback!(|_| {
					DialogMessage::CloseDialogAndThen {
						followups: vec![PortfolioMessage::CloseAllDocuments.into()],
					}
					.into()
				}),
				..Default::default()
			})),
			WidgetHolder::new(Widget::TextButton(TextButton {
				label: "Cancel".to_string(),
				min_width: 96,
				on_update: widget_callback!(|_| FrontendMessage::DisplayDialogDismiss.into()),
				..Default::default()
			})),
		];

		Layout::WidgetLayout(WidgetLayout::new(vec![
			LayoutGroup::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: "Close all documents?".to_string(),
					bold: true,
					..Default::default()
				}))],
			},
			LayoutGroup::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: "Unsaved work will be lost!".to_string(),
					multiline: true,
					..Default::default()
				}))],
			},
			LayoutGroup::Row { widgets: button_widgets },
		]))
	}
}

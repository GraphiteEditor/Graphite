use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

/// A dialog for confirming the closing of all documents viewable via `file -> close all` in the menu bar.
pub struct CloseAllDocumentsDialog;

impl LayoutHolder for CloseAllDocumentsDialog {
	fn layout(&self) -> Layout {
		let discard = TextButton::new("Discard All")
			.min_width(96)
			.emphasized(true)
			.on_update(|_| {
				DialogMessage::CloseDialogAndThen {
					followups: vec![PortfolioMessage::CloseAllDocuments.into()],
				}
				.into()
			})
			.widget_holder();
		let cancel = TextButton::new("Cancel").min_width(96).on_update(|_| FrontendMessage::DisplayDialogDismiss.into()).widget_holder();

		Layout::WidgetLayout(WidgetLayout::new(vec![
			LayoutGroup::Row {
				widgets: vec![TextLabel::new("Close all documents?").multiline(true).widget_holder()],
			},
			LayoutGroup::Row {
				widgets: vec![TextLabel::new("Unsaved work will be lost!").multiline(true).widget_holder()],
			},
			LayoutGroup::Row { widgets: vec![discard, cancel] },
		]))
	}
}

use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

/// A dialog for confirming the closing of all documents viewable via `File -> Close All` in the menu bar.
pub struct CloseAllDocumentsDialog {
	pub unsaved_document_names: Vec<String>,
}

impl DialogLayoutHolder for CloseAllDocumentsDialog {
	const ICON: &'static str = "Warning";
	const TITLE: &'static str = "Closing All Documents";

	fn layout_buttons(&self) -> Layout {
		let widgets = vec![
			TextButton::new("Discard All")
				.emphasized(true)
				.on_update(|_| {
					DialogMessage::CloseDialogAndThen {
						followups: vec![PortfolioMessage::CloseAllDocuments.into()],
					}
					.into()
				})
				.widget_holder(),
			TextButton::new("Cancel").on_update(|_| FrontendMessage::DisplayDialogDismiss.into()).widget_holder(),
		];

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

impl LayoutHolder for CloseAllDocumentsDialog {
	fn layout(&self) -> Layout {
		let unsaved_list = "• ".to_string() + &self.unsaved_document_names.join("\n• ");

		Layout::WidgetLayout(WidgetLayout::new(vec![
			LayoutGroup::Row {
				widgets: vec![TextLabel::new("Save documents before closing them?").bold(true).multiline(true).widget_holder()],
			},
			LayoutGroup::Row {
				widgets: vec![TextLabel::new(format!("Documents with unsaved changes:\n{unsaved_list}")).multiline(true).widget_holder()],
			},
		]))
	}
}

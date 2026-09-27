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
					DialogMessage::CloseAndThen {
						followups: vec![PortfolioMessage::CloseAllDocuments.into()],
					}
					.into()
				})
				.widget_instance(),
			TextButton::new("Cancel").on_update(|_| FrontendMessage::DialogClose.into()).widget_instance(),
		];

		Layout(vec![LayoutGroup::row(widgets)])
	}
}

impl LayoutHolder for CloseAllDocumentsDialog {
	fn layout(&self) -> Layout {
		let max_docs_to_show = 5;
		let mut unsaved_list = "• ".to_string() + &self.unsaved_document_names.iter().take(max_docs_to_show).cloned().collect::<Vec<_>>().join("\n• ");

		let remaining = self.unsaved_document_names.len().saturating_sub(max_docs_to_show);
		if remaining > 0 {
			let s = if remaining == 1 { "" } else { "s" };
			unsaved_list.push_str(&format!("\n... and {remaining} more document{s}"));
		}

		Layout(vec![
			LayoutGroup::row(vec![TextLabel::new("Save documents before closing them?").bold(true).multiline(true).widget_instance()]),
			LayoutGroup::row(vec![TextLabel::new(format!("Documents with unsaved changes:\n{unsaved_list}")).multiline(true).widget_instance()]),
		])
	}
}

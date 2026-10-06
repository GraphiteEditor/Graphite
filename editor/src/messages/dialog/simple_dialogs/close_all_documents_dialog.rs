use crate::consts::{MAX_DOCUMENT_NAME_LENGTH_IN_DIALOG, MAX_UNSAVED_DOCUMENTS_IN_DIALOG};
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

use std::borrow::Cow;
use std::fmt::Write;

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
		let mut unsaved_list = "• ".to_string()
			+ &self
				.unsaved_document_names
				.iter()
				.take(MAX_UNSAVED_DOCUMENTS_IN_DIALOG)
				.map(|name| {
					if name.chars().count() > MAX_DOCUMENT_NAME_LENGTH_IN_DIALOG {
						Cow::Owned(name.chars().take(MAX_DOCUMENT_NAME_LENGTH_IN_DIALOG).chain(std::iter::once('…')).collect())
					} else {
						Cow::Borrowed(name.as_str())
					}
				})
				.collect::<Vec<_>>()
				.join("\n• ");

		let remaining = self.unsaved_document_names.len().saturating_sub(MAX_UNSAVED_DOCUMENTS_IN_DIALOG);
		if remaining > 0 {
			let s = if remaining == 1 { "" } else { "s" };
			let _ = write!(unsaved_list, "\n... and {remaining} more document{s}");
		}

		Layout(vec![
			LayoutGroup::row(vec![TextLabel::new("Save documents before closing them?").bold(true).multiline(true).widget_instance()]),
			LayoutGroup::row(vec![TextLabel::new(format!("Documents with unsaved changes:\n{unsaved_list}")).multiline(true).widget_instance()]),
		])
	}
}

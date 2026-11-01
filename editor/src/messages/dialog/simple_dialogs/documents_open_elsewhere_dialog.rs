use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

/// Lists the stored documents that stayed closed because another tab has them open.
pub struct DocumentsOpenElsewhereDialog {
	/// Each document's id, name and whether it is in a live session.
	pub documents: Vec<(DocumentId, String, bool)>,
}

impl DialogLayoutHolder for DocumentsOpenElsewhereDialog {
	const ICON: &'static str = "Warning";
	const TITLE: &'static str = "Open in Another Tab";

	fn layout_buttons(&self) -> Layout {
		let widgets = vec![TextButton::new("Dismiss").emphasized(true).on_update(|_| DialogMessage::Close.into()).widget_instance()];

		Layout(vec![LayoutGroup::row(widgets)])
	}
}

impl LayoutHolder for DocumentsOpenElsewhereDialog {
	fn layout(&self) -> Layout {
		let count = self.documents.len();
		let (documents, are) = if count == 1 { ("document is", "It stays") } else { ("documents are", "They stay") };
		let mut rows = vec![
			LayoutGroup::row(vec![TextLabel::new(format!("{count} {documents} open in another tab.")).bold(true).multiline(true).widget_instance()]),
			LayoutGroup::row(vec![
				TextLabel::new(format!(
					"{are} closed here so the two tabs don't overwrite each other.\nA copy is a separate document, while joining a session edits it live."
				))
				.multiline(true)
				.widget_instance(),
			]),
		];
		rows.extend(self.documents.iter().cloned().map(|(source, name, shared)| {
			let label = TextLabel::new(name.clone()).widget_instance();
			let button = TextButton::new(if shared { "Join in This Tab" } else { "Open a Copy" })
				.on_update(move |_| {
					let followup = if shared {
						PortfolioMessage::JoinStoredSession { source }
					} else {
						PortfolioMessage::OpenDocumentCopy { source, document_name: name.clone() }
					};
					DialogMessage::CloseAndThen { followups: vec![followup.into()] }.into()
				})
				.widget_instance();
			LayoutGroup::row(vec![label, Separator::new(SeparatorStyle::Unrelated).widget_instance(), button])
		}));

		Layout(rows)
	}
}

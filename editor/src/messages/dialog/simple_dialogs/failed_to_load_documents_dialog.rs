use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

/// A dialog shown after startup when one or more autosaved documents fail to deserialize.
/// Offers the user a chance to download the raw content (so the data isn't lost), discard the failed
/// documents, or dismiss the dialog (keeping the autosave for a later session, when the deserialization
/// bug may be fixed in a future release).
pub struct FailedToLoadDocumentsDialog {
	pub failed_document_names: Vec<String>,
}

impl DialogLayoutHolder for FailedToLoadDocumentsDialog {
	const ICON: &'static str = "Warning";
	const TITLE: &'static str = "Failed to Open Documents";

	fn layout_buttons(&self) -> Layout {
		let widgets = vec![
			TextButton::new("Download")
				.emphasized(true)
				.tooltip_description("Save the raw document data to disk so it can be recovered later.")
				.on_update(|_| {
					DialogMessage::CloseAndThen {
						followups: vec![PortfolioMessage::DownloadFailedToLoadDocuments.into()],
					}
					.into()
				})
				.widget_instance(),
			TextButton::new("Discard")
				.tooltip_description("Permanently delete the autosaved data for these documents.")
				.on_update(|_| {
					DialogMessage::CloseAndThen {
						followups: vec![PortfolioMessage::DiscardFailedToLoadDocuments.into()],
					}
					.into()
				})
				.widget_instance(),
			TextButton::new("Dismiss")
				.tooltip_description("Close this dialog. The autosaved data is kept and this dialog will reappear on next launch.")
				.on_update(|_| FrontendMessage::DialogClose.into())
				.widget_instance(),
		];

		Layout(vec![LayoutGroup::row(widgets)])
	}
}

impl LayoutHolder for FailedToLoadDocumentsDialog {
	fn layout(&self) -> Layout {
		let count = self.failed_document_names.len();
		let header = format!("{count} document{} couldn't be reopened.", if count == 1 { "" } else { "s" });
		let list = "• ".to_string() + &self.failed_document_names.join("\n• ");
		let plural_s = if count == 1 { "" } else { "s" };
		let plural_it_them = if count == 1 { "it" } else { "them" };

		Layout(vec![
			LayoutGroup::row(vec![TextLabel::new(header).bold(true).multiline(true).widget_instance()]),
			LayoutGroup::row(vec![
				TextLabel::new(format!(
					"Sorry about that!\n\
					This shouldn't happen, and we'd like to help.\n\
					\n\
					Click \"Download\" to save a copy of the affected file{plural_s},\n\
					then please share {plural_it_them} with us so we can investigate:"
				))
				.multiline(true)
				.widget_instance(),
			]),
			LayoutGroup::row(vec![
				TextButton::new("Ask on Discord")
					.icon("Volunteer")
					.flush(true)
					.on_update(|_| {
						FrontendMessage::TriggerVisitLink {
							url: "https://discord.graphite.art".into(),
						}
						.into()
					})
					.widget_instance(),
			]),
			LayoutGroup::row(vec![
				TextButton::new("Report on GitHub")
					.icon("Bug")
					.flush(true)
					.on_update(|_| {
						FrontendMessage::TriggerVisitLink {
							url: "https://github.com/GraphiteEditor/Graphite/issues/new".into(),
						}
						.into()
					})
					.widget_instance(),
			]),
			LayoutGroup::row(vec![
				TextLabel::new(
					"In the meantime, you can keep working in the\n\
					previous version of Graphite:",
				)
				.multiline(true)
				.widget_instance(),
			]),
			LayoutGroup::row(vec![
				TextButton::new("Sept. 2025 Release")
					.icon("GraphiteLogo")
					.flush(true)
					.on_update(|_| {
						FrontendMessage::TriggerVisitLink {
							url: "https://57130155.graphite.pages.dev/".into(),
						}
						.into()
					})
					.widget_instance(),
			]),
			LayoutGroup::row(vec![TextLabel::new(format!("Affected document{plural_s}:\n{list}")).multiline(true).widget_instance()]),
		])
	}
}

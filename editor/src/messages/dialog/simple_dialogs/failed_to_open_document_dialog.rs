use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

/// A dialog shown when a manually opened document fails to deserialize. Mirrors the recovery affordances
/// of [`super::FailedToLoadDocumentsDialog`] (offering Discord/GitHub/previous-version links) but for
/// the single-file, user-initiated open case, where the file is already on the user's disk and there's
/// nothing to download or discard.
pub struct FailedToOpenDocumentDialog {
	/// Display name of the file the user tried to open; used in the dialog header. Falls back to a
	/// generic phrase when empty.
	pub document_name: String,
}

impl DialogLayoutHolder for FailedToOpenDocumentDialog {
	const ICON: &'static str = "Warning";
	const TITLE: &'static str = "Failed to Open Document";

	fn layout_buttons(&self) -> Layout {
		let widgets = vec![TextButton::new("OK").emphasized(true).on_update(|_| FrontendMessage::DialogClose.into()).widget_instance()];
		Layout(vec![LayoutGroup::row(widgets)])
	}
}

impl LayoutHolder for FailedToOpenDocumentDialog {
	fn layout(&self) -> Layout {
		let header = if self.document_name.trim().is_empty() {
			"The document couldn't be opened.".to_string()
		} else {
			format!("\"{}\" couldn't be opened.", self.document_name)
		};

		Layout(vec![
			LayoutGroup::row(vec![TextLabel::new(header).bold(true).multiline(true).widget_instance()]),
			LayoutGroup::row(vec![
				TextLabel::new(
					"Sorry about that!\n\
					This shouldn't happen, and we'd like to help.\n\
					\n\
					Please share the file with us so we can investigate:",
				)
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
		])
	}
}

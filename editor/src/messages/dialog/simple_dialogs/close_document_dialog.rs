use crate::messages::broadcast::broadcast_event::BroadcastEvent;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

/// A dialog for confirming the closing a document with unsaved changes.
pub struct CloseDocumentDialog {
	pub document_name: String,
	pub document_id: DocumentId,
}

impl DialogLayoutHolder for CloseDocumentDialog {
	const ICON: &'static str = "Warning";
	const TITLE: &'static str = "Closing Document";

	fn layout_buttons(&self) -> Layout {
		let document_id = self.document_id;
		let widgets = vec![
			TextButton::new("Save")
				.emphasized(true)
				.on_update(|_| {
					DialogMessage::CloseDialogAndThen {
						followups: vec![DocumentMessage::SaveDocument.into()],
					}
					.into()
				})
				.widget_holder(),
			TextButton::new("Discard")
				.on_update(move |_| {
					DialogMessage::CloseDialogAndThen {
						followups: vec![BroadcastEvent::ToolAbort.into(), PortfolioMessage::CloseDocument { document_id }.into()],
					}
					.into()
				})
				.widget_holder(),
			TextButton::new("Cancel").on_update(|_| FrontendMessage::DisplayDialogDismiss.into()).widget_holder(),
		];

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

impl LayoutHolder for CloseDocumentDialog {
	fn layout(&self) -> Layout {
		let max_length = 60;
		let max_one_line_length = 40;

		let mut name = self.document_name.clone();

		name.truncate(max_length);
		let ellipsis = if self.document_name.len() > max_length { "…" } else { "" };

		let break_lines = if self.document_name.len() > max_one_line_length { '\n' } else { ' ' };

		Layout::WidgetLayout(WidgetLayout::new(vec![
			LayoutGroup::Row {
				widgets: vec![TextLabel::new("Save document before closing it?").bold(true).widget_holder()],
			},
			LayoutGroup::Row {
				widgets: vec![TextLabel::new(format!("\"{name}{ellipsis}\"{break_lines}has unsaved changes")).multiline(true).widget_holder()],
			},
		]))
	}
}

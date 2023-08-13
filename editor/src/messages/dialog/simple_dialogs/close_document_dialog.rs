use crate::messages::broadcast::broadcast_event::BroadcastEvent;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

/// A dialog for confirming the closing a document with unsaved changes.
pub struct CloseDocumentDialog {
	pub document_name: String,
	pub document_id: u64,
}

impl LayoutHolder for CloseDocumentDialog {
	fn layout(&self) -> Layout {
		let document_id = self.document_id;

		let button_widgets = vec![
			TextButton::new("Save")
				.min_width(96)
				.emphasized(true)
				.on_update(|_| {
					DialogMessage::CloseDialogAndThen {
						followups: vec![DocumentMessage::SaveDocument.into()],
					}
					.into()
				})
				.widget_holder(),
			TextButton::new("Discard")
				.min_width(96)
				.on_update(move |_| {
					DialogMessage::CloseDialogAndThen {
						followups: vec![BroadcastEvent::ToolAbort.into(), PortfolioMessage::CloseDocument { document_id }.into()],
					}
					.into()
				})
				.widget_holder(),
			TextButton::new("Cancel").min_width(96).on_update(|_| FrontendMessage::DisplayDialogDismiss.into()).widget_holder(),
		];

		Layout::WidgetLayout(WidgetLayout::new(vec![
			LayoutGroup::Row {
				widgets: vec![TextLabel::new("Save changes before closing?").bold(true).widget_holder()],
			},
			LayoutGroup::Row {
				widgets: vec![TextLabel::new(&self.document_name).multiline(true).widget_holder()],
			},
			LayoutGroup::Row { widgets: button_widgets },
		]))
	}
}

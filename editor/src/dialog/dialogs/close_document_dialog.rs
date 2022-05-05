use crate::layout::widgets::*;
use crate::message_prelude::{DialogMessage, DocumentMessage, FrontendMessage, PortfolioMessage};

/// A dialog for confirming the closing a document with unsaved changes.
pub struct CloseDocument {
	pub document_name: String,
	pub document_id: u64,
}

impl PropertyHolder for CloseDocument {
	fn properties(&self) -> WidgetLayout {
		let document_id = self.document_id;

		let button_widgets = vec![
			WidgetHolder::new(Widget::TextButton(TextButton {
				label: "Save".to_string(),
				min_width: 96,
				emphasized: true,
				on_update: WidgetCallback::new(|_| {
					DialogMessage::CloseDialogAndThen {
						followup: Box::new(DocumentMessage::SaveDocument.into()),
					}
					.into()
				}),
				..Default::default()
			})),
			WidgetHolder::new(Widget::TextButton(TextButton {
				label: "Discard".to_string(),
				min_width: 96,
				on_update: WidgetCallback::new(move |_| {
					DialogMessage::CloseDialogAndThen {
						followup: Box::new(PortfolioMessage::CloseDocument { document_id }.into()),
					}
					.into()
				}),
				..Default::default()
			})),
			WidgetHolder::new(Widget::TextButton(TextButton {
				label: "Cancel".to_string(),
				min_width: 96,
				on_update: WidgetCallback::new(|_| FrontendMessage::DisplayDialogDismiss.into()),
				..Default::default()
			})),
		];

		WidgetLayout::new(vec![
			LayoutRow::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: self.document_name.clone(),
					preserve_whitespace: true,
					..Default::default()
				}))],
			},
			LayoutRow::Row {
				widgets: vec![WidgetHolder::new(Widget::Separator(Separator {
					direction: SeparatorDirection::Vertical,
					separator_type: SeparatorType::Unrelated,
				}))],
			},
			LayoutRow::Row { widgets: button_widgets },
		])
	}
}

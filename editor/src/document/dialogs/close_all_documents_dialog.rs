use crate::layout::widgets::*;
use crate::message_prelude::{FrontendMessage, PortfolioMessage};

pub struct CloseAllDocuments;

impl PropertyHolder for CloseAllDocuments {
	fn properties(&self) -> WidgetLayout {
		let button_widgets = vec![
			WidgetHolder::new(Widget::TextButton(TextButton {
				label: "Discard All".to_string(),
				min_width: 96,
				on_update: WidgetCallback::new(|_| {
					PortfolioMessage::CloseDialogAndThen {
						followup: Box::new(PortfolioMessage::CloseAllDocuments.into()),
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
					value: "Unsaved work will be lost!".to_string(),
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

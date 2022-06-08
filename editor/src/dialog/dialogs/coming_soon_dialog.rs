use crate::{layout::widgets::*, message_prelude::FrontendMessage};

use std::fmt::Write;

/// A dialog to notify users of an unfinished issue, optionally with an issue number.
pub struct ComingSoon {
	pub issue: Option<i32>,
}

impl PropertyHolder for ComingSoon {
	fn properties(&self) -> WidgetLayout {
		let mut details = "This feature is not implemented yet".to_string();
		let mut buttons = vec![WidgetHolder::new(Widget::TextButton(TextButton {
			label: "OK".to_string(),
			emphasized: true,
			min_width: 96,
			on_update: WidgetCallback::new(|_| FrontendMessage::DisplayDialogDismiss.into()),
			..Default::default()
		}))];
		if let Some(issue) = self.issue {
			let _ = write!(details, "— but you can help add it!\nSee issue #{issue} on GitHub.");
			buttons.push(WidgetHolder::new(Widget::TextButton(TextButton {
				label: format!("Issue #{issue}"),
				min_width: 96,
				on_update: WidgetCallback::new(move |_| {
					FrontendMessage::TriggerVisitLink {
						url: format!("https://github.com/GraphiteEditor/Graphite/issues/{issue}"),
					}
					.into()
				}),
				..Default::default()
			})));
		}
		WidgetLayout::new(vec![
			LayoutRow::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: "Coming soon".to_string(),
					bold: true,
					..Default::default()
				}))],
			},
			LayoutRow::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: details,
					multiline: true,
					..Default::default()
				}))],
			},
			LayoutRow::Row { widgets: buttons },
		])
	}
}

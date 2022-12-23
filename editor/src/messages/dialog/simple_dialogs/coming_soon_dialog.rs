use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, PropertyHolder, Widget, WidgetCallback, WidgetHolder, WidgetLayout};
use crate::messages::layout::utility_types::widgets::button_widgets::TextButton;
use crate::messages::layout::utility_types::widgets::label_widgets::TextLabel;
use crate::messages::prelude::*;

use std::fmt::Write;

/// A dialog to notify users of an unfinished issue, optionally with an issue number.
pub struct ComingSoonDialog {
	pub issue: Option<i32>,
}

impl PropertyHolder for ComingSoonDialog {
	fn properties(&self) -> Layout {
		let mut details = "This feature is not implemented yet".to_string();
		let mut buttons = vec![WidgetHolder::new(Widget::TextButton(TextButton {
			label: "OK".to_string(),
			emphasized: true,
			min_width: 96,
			on_update: widget_callback!(|_| FrontendMessage::DisplayDialogDismiss.into()),
			..Default::default()
		}))];
		if let Some(issue) = self.issue {
			let _ = write!(details, "— but you can help add it!\nSee issue #{issue} on GitHub.");
			buttons.push(WidgetHolder::new(Widget::TextButton(TextButton {
				label: format!("Issue #{issue}"),
				min_width: 96,
				on_update: widget_callback!(|_| {
					FrontendMessage::TriggerVisitLink {
						url: format!("https://github.com/GraphiteEditor/Graphite/issues/{issue}"),
					}
					.into()
				}),
				..Default::default()
			})));
		}
		Layout::WidgetLayout(WidgetLayout::new(vec![
			LayoutGroup::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: "Coming soon".to_string(),
					bold: true,
					..Default::default()
				}))],
			},
			LayoutGroup::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: details,
					multiline: true,
					..Default::default()
				}))],
			},
			LayoutGroup::Row { widgets: buttons },
		]))
	}
}

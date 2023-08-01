use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

use std::fmt::Write;

/// A dialog to notify users of an unfinished issue, optionally with an issue number.
pub struct ComingSoonDialog {
	pub issue: Option<i32>,
}

impl PropertyHolder for ComingSoonDialog {
	fn properties(&self) -> Layout {
		let mut details = "This feature is not implemented yet".to_string();

		let mut buttons = vec![TextButton::new("OK")
			.emphasized(true)
			.min_width(96)
			.on_update(|_| FrontendMessage::DisplayDialogDismiss.into())
			.widget_holder()];

		if let Some(issue) = self.issue {
			let _ = write!(details, "— but you can help add it!\nSee issue #{issue} on GitHub.");
			buttons.push(
				TextButton::new(format!("Issue #{issue}"))
					.min_width(96)
					.on_update(move |_| {
						FrontendMessage::TriggerVisitLink {
							url: format!("https://github.com/GraphiteEditor/Graphite/issues/{issue}"),
						}
						.into()
					})
					.widget_holder(),
			);
		}
		Layout::WidgetLayout(WidgetLayout::new(vec![
			LayoutGroup::Row {
				widgets: vec![TextLabel::new("Coming soon").bold(true).widget_holder()],
			},
			LayoutGroup::Row {
				widgets: vec![TextLabel::new(details).multiline(true).widget_holder()],
			},
			LayoutGroup::Row { widgets: buttons },
		]))
	}
}

use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

/// A dialog to notify users of an unfinished issue, optionally with an issue number.
pub struct ComingSoonDialog {
	pub issue: Option<i32>,
}

impl DialogLayoutHolder for ComingSoonDialog {
	const ICON: &'static str = "Delay";
	const TITLE: &'static str = "Coming Soon";

	fn layout_buttons(&self) -> Layout {
		let widgets = vec![TextButton::new("OK").emphasized(true).on_update(|_| FrontendMessage::DisplayDialogDismiss.into()).widget_holder()];

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

impl LayoutHolder for ComingSoonDialog {
	fn layout(&self) -> Layout {
		let header = vec![TextLabel::new("You've stumbled upon a placeholder").bold(true).widget_holder()];
		let row1 = vec![TextLabel::new("This feature is not implemented yet.").widget_holder()];

		let mut rows = vec![LayoutGroup::Row { widgets: header }, LayoutGroup::Row { widgets: row1 }];

		if let Some(issue) = self.issue {
			let row2 = vec![TextLabel::new("But you can help build it! Visit its issue:").widget_holder()];
			let row3 = vec![TextButton::new(format!("GitHub Issue #{issue}"))
				.icon(Some("Website".into()))
				.flush(true)
				.on_update(move |_| {
					FrontendMessage::TriggerVisitLink {
						url: format!("https://github.com/GraphiteEditor/Graphite/issues/{issue}"),
					}
					.into()
				})
				.widget_holder()];

			rows.push(LayoutGroup::Row { widgets: row2 });
			rows.push(LayoutGroup::Row { widgets: row3 });
		}

		Layout::WidgetLayout(WidgetLayout::new(rows))
	}
}

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
		if let Some(issue) = self.issue {
			let row1 = vec![TextLabel::new("This feature is not implemented yet— but you can help add it!\n\nOpen its issue to learn more:")
				.multiline(true)
				.widget_holder()];
			let row2 = vec![TextButton::new(format!("GitHub Issue #{issue}"))
				.icon(Some("Website".into()))
				.no_background(true)
				.on_update(move |_| {
					FrontendMessage::TriggerVisitLink {
						url: format!("https://github.com/GraphiteEditor/Graphite/issues/{issue}"),
					}
					.into()
				})
				.widget_holder()];

			Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets: row1 }, LayoutGroup::Row { widgets: row2 }]))
		} else {
			let widgets = vec![TextLabel::new("This feature is not implemented yet.").widget_holder()];

			Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
		}
	}
}

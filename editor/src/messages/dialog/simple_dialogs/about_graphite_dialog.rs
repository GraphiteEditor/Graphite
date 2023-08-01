use crate::application::{commit_info_localized, release_series};
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

/// A dialog for displaying information on [BuildMetadata] viewable via *Help* > *About Graphite* in the menu bar.
pub struct AboutGraphiteDialog {
	pub localized_commit_date: String,
}

impl LayoutHolder for AboutGraphiteDialog {
	fn layout(&self) -> Layout {
		let links = [
			("Website", "https://graphite.rs"),
			("Credits", "https://github.com/GraphiteEditor/Graphite/graphs/contributors"),
			("License", "https://raw.githubusercontent.com/GraphiteEditor/Graphite/master/LICENSE.txt"),
			("Third-Party Licenses", "/third-party-licenses.txt"),
		];
		let link_widgets = links
			.into_iter()
			.map(|(label, url)| TextButton::new(label).on_update(|_| FrontendMessage::TriggerVisitLink { url: url.to_string() }.into()).widget_holder())
			.collect();
		Layout::WidgetLayout(WidgetLayout::new(vec![
			LayoutGroup::Row {
				widgets: vec![TextLabel::new("Graphite".to_string()).bold(true).widget_holder()],
			},
			LayoutGroup::Row {
				widgets: vec![TextLabel::new(release_series()).widget_holder()],
			},
			LayoutGroup::Row {
				widgets: vec![TextLabel::new(commit_info_localized(&self.localized_commit_date)).multiline(true).widget_holder()],
			},
			LayoutGroup::Row { widgets: link_widgets },
		]))
	}
}

use crate::application::{commit_info_localized, release_series};
use crate::messages::layout::utility_types::layout_widget::{Layout, LayoutGroup, PropertyHolder, Widget, WidgetHolder, WidgetLayout};
use crate::messages::layout::utility_types::widgets::button_widgets::TextButton;
use crate::messages::layout::utility_types::widgets::label_widgets::TextLabel;
use crate::messages::prelude::*;

/// A dialog for displaying information on [BuildMetadata] viewable via *Help* > *About Graphite* in the menu bar.
pub struct AboutGraphiteDialog {
	pub localized_commit_date: String,
}

impl PropertyHolder for AboutGraphiteDialog {
	fn properties(&self) -> Layout {
		let links = [
			("Website", "https://graphite.rs"),
			("Credits", "https://github.com/GraphiteEditor/Graphite/graphs/contributors"),
			("License", "https://raw.githubusercontent.com/GraphiteEditor/Graphite/master/LICENSE.txt"),
			("Third-Party Licenses", "/third-party-licenses.txt"),
		];
		let link_widgets = links
			.into_iter()
			.map(|(label, url)| {
				WidgetHolder::new(Widget::TextButton(TextButton {
					label: label.to_string(),
					on_update: widget_callback!(|_| FrontendMessage::TriggerVisitLink { url: url.to_string() }.into()),
					..Default::default()
				}))
			})
			.collect();
		Layout::WidgetLayout(WidgetLayout::new(vec![
			LayoutGroup::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: "Graphite".to_string(),
					bold: true,
					..Default::default()
				}))],
			},
			LayoutGroup::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: release_series(),
					..Default::default()
				}))],
			},
			LayoutGroup::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: commit_info_localized(self.localized_commit_date.as_str()),
					multiline: true,
					..Default::default()
				}))],
			},
			LayoutGroup::Row { widgets: link_widgets },
		]))
	}
}

use crate::layout::widgets::*;
use crate::message_prelude::FrontendMessage;
use crate::misc::build_metadata::{commit_info_localized, release_series};

/// A dialog for displaying information on [`BuildMetadata`] viewable via *Help* > *About Graphite* in the menu bar.
pub struct AboutGraphite {
	pub localized_commit_date: String,
}

impl PropertyHolder for AboutGraphite {
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
					on_update: WidgetCallback::new(|_| FrontendMessage::TriggerVisitLink { url: url.to_string() }.into()),
					..Default::default()
				}))
			})
			.collect();
		Layout::WidgetLayout(WidgetLayout::new(vec![
			LayoutRow::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: "Graphite".to_string(),
					bold: true,
					..Default::default()
				}))],
			},
			LayoutRow::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: release_series(),
					..Default::default()
				}))],
			},
			LayoutRow::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: commit_info_localized(self.localized_commit_date.as_str()),
					multiline: true,
					..Default::default()
				}))],
			},
			LayoutRow::Row { widgets: link_widgets },
		]))
	}
}

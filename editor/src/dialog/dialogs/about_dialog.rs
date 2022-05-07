use crate::{communication::BuildMetadata, layout::widgets::*, message_prelude::FrontendMessage};

/// A dialog for displaying information on [`BuildMetadata`] viewable via `help -> about graphite` in the menu bar.
pub struct AboutGraphite {
	pub build_metadata: BuildMetadata,
}

impl PropertyHolder for AboutGraphite {
	fn properties(&self) -> WidgetLayout {
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
		WidgetLayout::new(vec![
			LayoutRow::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: "Graphite".to_string(),
					bold: true,
					..Default::default()
				}))],
			},
			LayoutRow::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: self.build_metadata.release_series(),
					..Default::default()
				}))],
			},
			LayoutRow::Row {
				widgets: vec![WidgetHolder::new(Widget::TextLabel(TextLabel {
					value: self.build_metadata.commit_info(),
					multiline: true,
					..Default::default()
				}))],
			},
			LayoutRow::Row { widgets: link_widgets },
		])
	}
}

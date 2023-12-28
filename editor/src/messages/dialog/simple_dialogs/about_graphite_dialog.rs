use crate::application::commit_info_localized;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::prelude::*;

/// A dialog for displaying information on [BuildMetadata] viewable via *Help* > *About Graphite* in the menu bar.
pub struct AboutGraphiteDialog {
	pub localized_commit_date: String,
	pub localized_commit_year: String,
}

impl DialogLayoutHolder for AboutGraphiteDialog {
	const ICON: &'static str = "GraphiteLogo";
	const TITLE: &'static str = "About Graphite";

	fn layout_buttons(&self) -> Layout {
		let widgets = vec![TextButton::new("OK").emphasized(true).on_update(|_| FrontendMessage::DisplayDialogDismiss.into()).widget_holder()];

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}

	fn layout_column_2(&self) -> Layout {
		let links = [
			("Website", "Website", "https://graphite.rs"),
			("Volunteer", "Volunteer", "https://graphite.rs/volunteer/"),
			("Credits", "Credits", "https://github.com/GraphiteEditor/Graphite/graphs/contributors"),
		];
		let mut widgets = links
			.into_iter()
			.map(|(icon, label, url)| {
				TextButton::new(label)
					.icon(Some(icon.into()))
					.flush(true)
					.on_update(|_| FrontendMessage::TriggerVisitLink { url: url.into() }.into())
					.widget_holder()
			})
			.collect::<Vec<_>>();

		// Cloning here and below seems to be necessary to appease the borrow checker, as far as I can tell.
		let localized_commit_year = self.localized_commit_year.clone();
		widgets.push(
			TextButton::new("Licenses")
				.icon(Some("License".into()))
				.flush(true)
				.on_update(move |_| {
					DialogMessage::RequestLicensesDialogWithLocalizedCommitDate {
						localized_commit_year: localized_commit_year.clone(),
					}
					.into()
				})
				.widget_holder(),
		);

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Column { widgets }]))
	}
}

impl LayoutHolder for AboutGraphiteDialog {
	fn layout(&self) -> Layout {
		Layout::WidgetLayout(WidgetLayout::new(vec![
			LayoutGroup::Row {
				widgets: vec![TextLabel::new("About this release").bold(true).widget_holder()],
			},
			LayoutGroup::Row {
				widgets: vec![TextLabel::new(commit_info_localized(&self.localized_commit_date)).multiline(true).widget_holder()],
			},
			LayoutGroup::Row {
				widgets: vec![TextLabel::new(format!("Copyright © {} Graphite contributors", self.localized_commit_year)).widget_holder()],
			},
		]))
	}
}
